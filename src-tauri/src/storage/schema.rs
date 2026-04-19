// schema.rs · 嵌入式 migration 管理
//
// 每个 migration 文件编译时嵌入二进制，启动时按版本号顺序执行未应用的。

use rusqlite::Connection;

/// 嵌入 migration SQL 文件
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("migrations/001_initial.sql")),
    (2, include_str!("migrations/002_settings_keys_cleanup.sql")),
    (3, include_str!("migrations/003_matched_domain_rule.sql")),
    (4, include_str!("migrations/004_pin_support.sql")),
    (5, include_str!("migrations/005_last_used_at.sql")),
    (6, include_str!("migrations/006_image_dims.sql")),
    (7, include_str!("migrations/007_session_grouping.sql")),
    (8, include_str!("migrations/008_aggregated_sessions.sql")),
];

/// 获取当前 schema 版本（schema_migrations 表不存在返回 0）
fn current_version(conn: &Connection) -> i64 {
    // 检查 schema_migrations 表是否存在
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !table_exists {
        return 0;
    }

    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// 执行所有未应用的 migration
pub fn run_migrations(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let current = current_version(conn);
    tracing::info!("Current schema version: {current}");

    for (version, sql) in MIGRATIONS {
        if *version > current {
            tracing::info!("Applying migration {version}...");
            conn.execute_batch(sql)?;
            tracing::info!("Migration {version} applied successfully");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_fresh_migration() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();

        run_migrations(&conn).unwrap();

        // 验证表存在
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"clipboard_local".to_string()));
        assert!(tables.contains(&"app_rules".to_string()));
        assert!(tables.contains(&"domain_rules".to_string()));
        assert!(tables.contains(&"settings".to_string()));
        assert!(tables.contains(&"schema_migrations".to_string()));

        // 版本 = MIGRATIONS 数组最大版本号
        assert_eq!(current_version(&conn), MIGRATIONS.iter().map(|(v, _)| *v).max().unwrap());
    }

    #[test]
    fn test_idempotent_migration() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();

        run_migrations(&conn).unwrap();
        let v1 = current_version(&conn);
        // 第二次执行不报错
        run_migrations(&conn).unwrap();
        assert_eq!(current_version(&conn), v1);
    }

    #[test]
    fn test_fts5_trigger() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        run_migrations(&conn).unwrap();

        // 插入一条记录
        conn.execute(
            "INSERT INTO clipboard_local (id, content_hash, content, content_type, captured_at, state)
             VALUES ('test-1', 'hash1', 'hello world 你好世界', 'text', 1000, 'captured')",
            [],
        )
        .unwrap();

        // FTS5 搜索
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE clipboard_fts MATCH 'hello'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // 删除后 FTS 也同步
        conn.execute("DELETE FROM clipboard_local WHERE id = 'test-1'", [])
            .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE clipboard_fts MATCH 'hello'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_settings_table_empty_after_migrations() {
        // migration 001 预置了几个老键，migration 002 清理它们。
        // 新架构里 settings 表在 fresh migration 后应为空 —— 业务层默认值全走
        // settings_keys.rs 常量的 *_DEFAULT，不靠 DB INSERT。
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        run_migrations(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM settings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            count, 0,
            "settings 表 fresh migration 后应为空，业务默认值走常量而非 DB INSERT"
        );
    }
}
