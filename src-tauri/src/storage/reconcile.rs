//! 孤儿 PNG 清理 —— 启动时对账 images/ 目录与 DB 引用。
//!
//! 背景：capture loop 流程是"先写 PNG 文件 → INSERT clipboard_local 行"，两步非
//! 原子。崩溃 / 磁盘满 / 进程被 kill 在中间 → PNG 在但 DB 行不在，产生孤儿。
//! 长期积累 `images/` 会越来越大。
//!
//! Ditto 用 SQLite 事务包裹多步写入避免这种孤儿；Teamo 的 image 文件是文件系统
//! 级的，不在 SQL 事务内，必然有这种 skew。启动期一次性 reconcile 是成本最低的
//! 修复（对比：每次 capture 都 fsync + 事务 = 性能退化）。
//!
//! 对标方向：清理规则不是"DB 没引用就删"，而是"DB 没引用且文件 mtime > 60s"
//! —— 避免清掉正在 in-flight 被 capture_loop 写入但 INSERT 还没完成的 PNG。

use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;
use std::time::SystemTime;

/// 启动时清理孤儿 PNG。返回清理数量。
///
/// 规则：文件名不在 `clipboard_local.image_path` 集合中 **且** 文件 mtime 超过
/// 60 秒前（给 in-flight 的写操作留安全边界）。
pub fn cleanup_orphan_images(
    conn: &Connection,
    images_dir: &Path,
) -> Result<usize, Box<dyn std::error::Error>> {
    if !images_dir.exists() {
        return Ok(0);
    }

    // 收集 DB 里所有被引用的 image_path 文件名
    let referenced: HashSet<String> = conn
        .prepare("SELECT image_path FROM clipboard_local WHERE image_path IS NOT NULL")?
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    let safety_cutoff = SystemTime::now() - std::time::Duration::from_secs(60);

    let mut removed = 0;
    for entry in std::fs::read_dir(images_dir)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();

        // 跳过非 PNG（未来可能有其他扩展名 / thumbnail cache）
        if !name.to_ascii_lowercase().ends_with(".png") {
            continue;
        }

        if referenced.contains(&name) {
            continue;
        }

        // mtime 安全边界：in-flight 写入的 PNG 可能 INSERT 还在事务中，别误删
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(mtime) = meta.modified() else { continue };
        if mtime > safety_cutoff {
            continue;
        }

        let path = entry.path();
        match std::fs::remove_file(&path) {
            Ok(_) => {
                removed += 1;
                tracing::info!("Removed orphan image: {name}");
            }
            Err(e) => {
                tracing::warn!("Failed to remove orphan image {name}: {e}");
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::schema;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Connection) {
        let dir = TempDir::new().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        schema::run_migrations(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn test_recent_file_kept() {
        // in-flight 场景：文件刚创建 (< 60s) 不应被清（即使 DB 无引用）
        // 这是最关键的安全边界：防止误删 capture_loop 正在写但 INSERT 未完成的 PNG
        let (dir, conn) = setup();
        fs::write(dir.path().join("inflight.png"), b"fake").unwrap();
        let removed = cleanup_orphan_images(&conn, dir.path()).unwrap();
        assert_eq!(removed, 0);
        assert!(dir.path().join("inflight.png").exists());
    }

    #[test]
    fn test_nonexistent_dir() {
        let (_dir, conn) = setup();
        let removed = cleanup_orphan_images(&conn, Path::new("/nonexistent/path")).unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_non_png_skipped() {
        // 非 .png 文件跳过（未来可能有 thumbnail cache 等）
        let (dir, conn) = setup();
        fs::write(dir.path().join("readme.txt"), b"fake").unwrap();
        let removed = cleanup_orphan_images(&conn, dir.path()).unwrap();
        assert_eq!(removed, 0);
        assert!(dir.path().join("readme.txt").exists());
    }

    // 完整 orphan-removal 测试需要 set_file_mtime 能力（std 无此 API），
    // 在 CI 集成测试里用 `filetime` crate 覆盖；这里单测只验证保守边界。
}
