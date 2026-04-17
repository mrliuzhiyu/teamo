//! 数据保留清理 —— 按 `data.retention` 设置删除过期 clipboard_local 行 + 关联图片。
//!
//! 触发时机：
//! - 启动时跑一次（lib.rs setup）
//! - 未来 Phase 2 加 tokio interval 每 6 小时跑一次
//!
//! 保留策略（`data.retention` 枚举值）：
//! - `forever`（默认）→ 不清理
//! - `1y` / `6m` / `1m` → 对应天数前的记录删掉
//!
//! 设计原则：
//! - 清理 clipboard_local 行靠 DELETE（FTS5 触发器自动同步）
//! - 关联 image_path 指向的 PNG 文件同步删
//! - 失败不阻塞启动（log warn 即可）
//! - 用户改动保留时长实时生效（下次启动生效，不重启后台清理 Phase 2 做）

use rusqlite::{params, Connection};
use std::path::Path;

use crate::settings_keys;

use super::{repository, StorageError};

/// 把 retention 枚举值翻译成毫秒阈值。`forever` 返回 None（不清理）。
fn retention_to_ms(retention: &str) -> Option<i64> {
    let days: i64 = match retention {
        "1y" => 365,
        "6m" => 180,
        "1m" => 30,
        _ => return None, // "forever" 或未知值都不清理
    };
    Some(days * 24 * 3600 * 1000)
}

/// 根据 `data.retention` 设置清理过期数据。返回删除的行数（图片也一并清）。
pub fn prune_expired(conn: &Connection, images_dir: &Path) -> Result<usize, StorageError> {
    let retention = repository::get_setting(conn, settings_keys::DATA_RETENTION)?
        .unwrap_or_else(|| settings_keys::DATA_RETENTION_DEFAULT.to_string());

    let Some(retention_ms) = retention_to_ms(&retention) else {
        tracing::debug!("retention={retention} → no pruning");
        return Ok(0);
    };

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let cutoff = now_ms - retention_ms;

    // 1. 收集即将删除的 image_path（要清关联文件）
    let mut stmt = conn.prepare(
        "SELECT image_path FROM clipboard_local
         WHERE captured_at < ?1 AND image_path IS NOT NULL",
    )?;
    let expired_images: Vec<String> = stmt
        .query_map(params![cutoff], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    // 2. DELETE 行（FTS5 触发器自动同步）
    let deleted = conn.execute(
        "DELETE FROM clipboard_local WHERE captured_at < ?1",
        params![cutoff],
    )?;

    // 3. 清关联图片文件
    for img in expired_images {
        let path = images_dir.join(&img);
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("failed to remove expired image {img}: {e}");
            }
        }
    }

    if deleted > 0 {
        tracing::info!(
            "retention[{retention}] pruned {deleted} clipboard_local rows older than {cutoff} ms"
        );
    }

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{repository, schema};
    use tempfile::TempDir;

    fn setup() -> (Connection, TempDir) {
        let tmp = TempDir::new().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        schema::run_migrations(&conn).unwrap();
        (conn, tmp)
    }

    fn insert_row(conn: &Connection, id: &str, captured_at_ms: i64) {
        conn.execute(
            "INSERT INTO clipboard_local
             (id, content_hash, content, content_type, captured_at, state)
             VALUES (?1, ?2, ?3, 'text', ?4, 'captured')",
            params![id, format!("hash-{id}"), format!("content-{id}"), captured_at_ms],
        )
        .unwrap();
    }

    #[test]
    fn test_forever_no_pruning() {
        let (conn, tmp) = setup();
        insert_row(&conn, "a", 0); // 很久以前
        insert_row(&conn, "b", i64::MAX / 2);

        // 默认 "forever" 不清
        let pruned = prune_expired(&conn, tmp.path()).unwrap();
        assert_eq!(pruned, 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_local", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_1m_prunes_old() {
        let (conn, tmp) = setup();
        repository::set_setting(&conn, settings_keys::DATA_RETENTION, Some("1m")).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let two_months_ago = now - 60 * 24 * 3600 * 1000;
        let last_week = now - 7 * 24 * 3600 * 1000;

        insert_row(&conn, "old", two_months_ago);
        insert_row(&conn, "new", last_week);

        let pruned = prune_expired(&conn, tmp.path()).unwrap();
        assert_eq!(pruned, 1);

        let remaining: Vec<String> = conn
            .prepare("SELECT id FROM clipboard_local")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(remaining, vec!["new".to_string()]);
    }

    #[test]
    fn test_prune_removes_image_files() {
        let (conn, tmp) = setup();
        repository::set_setting(&conn, settings_keys::DATA_RETENTION, Some("1m")).unwrap();

        let img_file = "old.png";
        let img_path = tmp.path().join(img_file);
        std::fs::write(&img_path, b"fake image").unwrap();

        let two_months_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            - 60 * 24 * 3600 * 1000;

        conn.execute(
            "INSERT INTO clipboard_local
             (id, content_hash, content, content_type, image_path, captured_at, state)
             VALUES ('img-old', 'h1', 'fp', 'image', ?1, ?2, 'captured')",
            params![img_file, two_months_ago],
        )
        .unwrap();

        let pruned = prune_expired(&conn, tmp.path()).unwrap();
        assert_eq!(pruned, 1);
        assert!(!img_path.exists(), "image file should be removed");
    }

    #[test]
    fn test_unknown_retention_no_pruning() {
        let (conn, tmp) = setup();
        repository::set_setting(
            &conn,
            settings_keys::DATA_RETENTION,
            Some("bogus_value"),
        )
        .unwrap();
        insert_row(&conn, "a", 0);

        let pruned = prune_expired(&conn, tmp.path()).unwrap();
        assert_eq!(pruned, 0, "unknown retention value should be safe no-op");
    }
}
