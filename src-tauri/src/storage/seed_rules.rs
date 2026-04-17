//! domain_rules.yaml seed —— 启动时把仓库根的 domain_rules.yaml 70+ 条
//! 内置规则导入 DB（首次启动或显式重 seed 时）。
//!
//! 架构：
//! - YAML 通过 `include_str!` 编译时嵌入二进制，运行时不读文件
//! - 规则 `source='builtin'` 标记，和用户/云端规则分开管理
//! - **版本化升级**：YAML 顶层 `version: N` 字段 + DB 里 `filter.builtin_rules_version` settings。
//!   启动时比对：YAML 版本 > DB 版本 → 清空 builtin 重 seed（保留 user/cloud），
//!   写入新版本。这样 Teamo 发版升级 YAML 时老用户的 builtin 规则自动同步。

use serde::Deserialize;

use crate::settings_keys;

use super::{repository, StorageError};

/// YAML 顶层结构
#[derive(Debug, Deserialize)]
struct RuleFile {
    version: Option<u32>,
    rules: Vec<RuleEntry>,
}

#[derive(Debug, Deserialize)]
struct RuleEntry {
    pattern: String,
    #[serde(rename = "type")]
    rule_type: String,
    #[serde(default)]
    priority: Option<i64>,
    // note 字段存在但不导入 DB
    #[allow(dead_code)]
    #[serde(default)]
    note: Option<String>,
}

/// 嵌入的 YAML 源文件（仓库根的 domain_rules.yaml）。
const EMBEDDED_YAML: &str = include_str!("../../../domain_rules.yaml");

/// 按版本号比对决定是否 (重新) seed 内置 domain_rules。
///
/// 策略：
/// - YAML 顶部 version 字段（`version: 1`）作为内置规则库的版本源
/// - DB 里 `filter.builtin_rules_version` settings 记录当前已 seeded 的版本
/// - YAML 版本 > DB 版本（或 DB 无记录）→ 清空 source='builtin' 重 seed，更新版本号
/// - YAML 版本 ≤ DB 版本 → no-op
///
/// 这样 Teamo 发版时每次更新 domain_rules.yaml 都 bump version，老用户升级启动
/// 自动拿到新的 builtin 规则。user/cloud 来源规则不受影响（独立 source 标记）。
///
/// 向后兼容：旧版 Teamo 已经 seed 的用户 DB 里没有 `filter.builtin_rules_version` 设置，
/// 首次升级当作 version=0 → 触发重 seed → 正常。
pub fn seed_if_outdated(conn: &rusqlite::Connection) -> Result<(), StorageError> {
    let parsed: RuleFile = serde_yaml::from_str(EMBEDDED_YAML)?; // ? 走 StorageError::Yaml

    let yaml_version = parsed.version.unwrap_or(0);

    let db_version: u32 = repository::get_setting(conn, settings_keys::FILTER_BUILTIN_RULES_VERSION)?
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if yaml_version <= db_version {
        tracing::debug!(
            "domain_rules up to date (yaml v{yaml_version} ≤ db v{db_version}), skip seed"
        );
        return Ok(());
    }

    tracing::info!("domain_rules yaml v{yaml_version} > db v{db_version} — reseeding builtin");

    // 先清空 source='builtin' 的规则（保留 user/cloud）
    let removed = repository::delete_domain_rules_by_source(conn, "builtin")?;
    if removed > 0 {
        tracing::info!("removed {removed} stale builtin rules");
    }

    let rules: Vec<(String, String, i64)> = parsed
        .rules
        .into_iter()
        .filter(|r| {
            matches!(
                r.rule_type.as_str(),
                "parse_as_content" | "skip_parse" | "skip_upload"
            )
        })
        .map(|r| (r.pattern, r.rule_type, r.priority.unwrap_or(0)))
        .collect();

    let count = repository::bulk_insert_domain_rules(conn, &rules, "builtin")?;

    repository::set_setting(
        conn,
        settings_keys::FILTER_BUILTIN_RULES_VERSION,
        Some(&yaml_version.to_string()),
    )?;

    tracing::info!("domain_rules builtin seeded to v{yaml_version}: {count} rules");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::schema;

    fn setup_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        schema::run_migrations(&conn).unwrap();
        conn
    }

    /// **编译/CI 防线**：嵌入的 domain_rules.yaml 必须能被 serde_yaml 解析。
    /// 开发者在 YAML 里手滑（tab/空格混用、缺字段、rule_type 拼错）时，
    /// 这个测试立即在 CI 红灯，而不是等 release 到用户机器上 seed 0 条规则
    /// 导致所有银行/登录页 skip_upload 静默失效。
    #[test]
    fn test_embedded_yaml_parses() {
        let parsed: RuleFile = serde_yaml::from_str(EMBEDDED_YAML)
            .expect("domain_rules.yaml failed to parse — fix syntax in the YAML file");
        assert!(
            !parsed.rules.is_empty(),
            "domain_rules.yaml has no rules entries"
        );
    }

    /// **编译/CI 防线**：每条 YAML 规则的 rule_type 必须在白名单里。
    /// 历史教训：typo "skip_uploads"（多了 s）会被 seed_if_empty 的 filter 静默丢弃，
    /// 开发者只有在手动查 DB 时才发现规则少了一条。
    #[test]
    fn test_embedded_yaml_rule_types_valid() {
        let parsed: RuleFile = serde_yaml::from_str(EMBEDDED_YAML).unwrap();
        const VALID: &[&str] = &["parse_as_content", "skip_parse", "skip_upload"];
        for (i, rule) in parsed.rules.iter().enumerate() {
            assert!(
                VALID.contains(&rule.rule_type.as_str()),
                "rule #{i} pattern={:?} has invalid rule_type {:?} (must be one of {:?})",
                rule.pattern,
                rule.rule_type,
                VALID,
            );
        }
    }

    /// **编译/CI 防线**：pattern 非空、priority 合理（如果提供）。
    #[test]
    fn test_embedded_yaml_fields_sane() {
        let parsed: RuleFile = serde_yaml::from_str(EMBEDDED_YAML).unwrap();
        for (i, rule) in parsed.rules.iter().enumerate() {
            assert!(
                !rule.pattern.trim().is_empty(),
                "rule #{i} has empty pattern"
            );
            if let Some(p) = rule.priority {
                assert!(
                    p >= 0 && p <= 10_000,
                    "rule #{i} pattern={:?} priority={p} out of sane range [0, 10000]",
                    rule.pattern
                );
            }
        }
    }

    #[test]
    fn test_seed_from_embedded_yaml() {
        let conn = setup_db();
        seed_if_outdated(&conn).unwrap();
        let count = repository::count_domain_rules_by_source(&conn, "builtin").unwrap();
        // 仓库 YAML 有 70+ 条规则
        assert!(count >= 50, "expected 50+ builtin rules, got {count}");
    }

    #[test]
    fn test_seed_idempotent() {
        let conn = setup_db();
        seed_if_outdated(&conn).unwrap();
        let c1 = repository::count_domain_rules_by_source(&conn, "builtin").unwrap();
        // 版本号没 bump，第二次应 no-op
        seed_if_outdated(&conn).unwrap();
        let c2 = repository::count_domain_rules_by_source(&conn, "builtin").unwrap();
        assert_eq!(c1, c2, "seed should be no-op when version unchanged");
    }

    #[test]
    fn test_seed_version_bump_reseeds() {
        let conn = setup_db();
        seed_if_outdated(&conn).unwrap();
        let c1 = repository::count_domain_rules_by_source(&conn, "builtin").unwrap();

        // 模拟 YAML 版本 bump：把 DB 里记录的版本回退到 0
        repository::set_setting(&conn, settings_keys::FILTER_BUILTIN_RULES_VERSION, Some("0"))
            .unwrap();

        // 再跑 seed — YAML version=1 > db version=0 → 应该清空重 seed
        seed_if_outdated(&conn).unwrap();
        let c2 = repository::count_domain_rules_by_source(&conn, "builtin").unwrap();
        assert_eq!(c1, c2, "reseed should yield same builtin count from same YAML");
    }

    #[test]
    fn test_seed_version_bump_preserves_user_rules() {
        let conn = setup_db();
        seed_if_outdated(&conn).unwrap();

        // 用户添加自定义规则（source='user'）
        repository::bulk_insert_domain_rules(
            &conn,
            &[("my-private.com/*".to_string(), "skip_upload".to_string(), 500)],
            "user",
        )
        .unwrap();
        let user_count_before = repository::count_domain_rules_by_source(&conn, "user").unwrap();
        assert_eq!(user_count_before, 1);

        // 模拟版本 bump 触发重 seed
        repository::set_setting(&conn, settings_keys::FILTER_BUILTIN_RULES_VERSION, Some("0"))
            .unwrap();
        seed_if_outdated(&conn).unwrap();

        // builtin 被重建，但 user 规则必须保留
        let user_count_after = repository::count_domain_rules_by_source(&conn, "user").unwrap();
        assert_eq!(
            user_count_after, 1,
            "user rules must survive builtin reseed (delete_domain_rules_by_source 不该误删 user)"
        );
    }
}
