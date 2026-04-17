//! domain_rules.yaml seed —— 启动时把仓库根的 domain_rules.yaml 70+ 条
//! 内置规则导入 DB（首次启动或显式重 seed 时）。
//!
//! 架构：
//! - YAML 通过 `include_str!` 编译时嵌入二进制，运行时不读文件
//! - 规则 `source='builtin'` 标记，和用户/云端规则分开管理
//! - 首次启动 count('builtin')=0 时自动 seed；一旦 seeded 之后不重复
//!   （未来规则库升级策略：加 `filter.builtin_rules_version` settings，
//!    版本号升级时清空 builtin 重 seed，保留 user/cloud）

use serde::Deserialize;

use super::repository;

/// YAML 顶层结构
#[derive(Debug, Deserialize)]
struct RuleFile {
    #[allow(dead_code)]
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

/// 如果 DB 中 builtin 规则为 0 条，就从 YAML seed 一次。
/// 幂等：已有 builtin 规则时 no-op。
pub fn seed_if_empty(conn: &rusqlite::Connection) -> Result<(), String> {
    let existing = repository::count_domain_rules_by_source(conn, "builtin")
        .map_err(|e| format!("count builtin rules: {e}"))?;
    if existing > 0 {
        tracing::debug!("domain_rules builtin already seeded ({existing} rules), skip");
        return Ok(());
    }

    let parsed: RuleFile = serde_yaml::from_str(EMBEDDED_YAML)
        .map_err(|e| format!("parse domain_rules.yaml: {e}"))?;

    let rules: Vec<(String, String, i64)> = parsed
        .rules
        .into_iter()
        .filter(|r| {
            // 过滤无效 rule_type（YAML 里允许 parse_as_content / skip_parse / skip_upload）
            matches!(
                r.rule_type.as_str(),
                "parse_as_content" | "skip_parse" | "skip_upload"
            )
        })
        .map(|r| (r.pattern, r.rule_type, r.priority.unwrap_or(0)))
        .collect();

    let count = repository::bulk_insert_domain_rules(conn, &rules, "builtin")
        .map_err(|e| format!("insert domain_rules: {e}"))?;
    tracing::info!("domain_rules builtin seeded: {count} rules");
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

    #[test]
    fn test_seed_from_embedded_yaml() {
        let conn = setup_db();
        seed_if_empty(&conn).unwrap();
        let count =
            repository::count_domain_rules_by_source(&conn, "builtin").unwrap();
        // 仓库 YAML 有 70+ 条规则
        assert!(count >= 50, "expected 50+ builtin rules, got {count}");
    }

    #[test]
    fn test_seed_idempotent() {
        let conn = setup_db();
        seed_if_empty(&conn).unwrap();
        let c1 = repository::count_domain_rules_by_source(&conn, "builtin").unwrap();
        // 第二次调用不应该重复插入
        seed_if_empty(&conn).unwrap();
        let c2 = repository::count_domain_rules_by_source(&conn, "builtin").unwrap();
        assert_eq!(c1, c2, "seed should be idempotent");
    }
}
