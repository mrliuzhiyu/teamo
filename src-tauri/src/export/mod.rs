//! 数据导出 — JSON / Markdown + 图片副本
//!
//! Phase 1（当前）：同步导出，console 打 log
//! Phase 2 留：tokio 后台任务 + 进度 event + 取消按钮
//!
//! 设计原则：
//! - 格式公开（JSON schema + Markdown frontmatter），任何工具都能消费
//! - 图片独立目录，避免单文件 50MB 图片串堆
//! - 敏感内容在 Markdown 里打码（JSON 保留原内容，用户知情）

pub mod json;
pub mod markdown;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::storage::AppDatabase;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Json,
    Markdown,
}

/// 导出结果 — 返回给前端显示统计
#[derive(Debug, Serialize)]
pub struct ExportResult {
    pub exported_count: usize,
    pub image_count: usize,
    pub missing_images: usize,
    pub target_dir: String,
}

/// 单条导出 row — JSON 序列化后直接写盘
///
/// 同时 derive Deserialize 便于单测 round-trip（序列化 → 解析回来字段一致）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRow {
    pub id: String,
    pub content: Option<String>,
    pub content_type: String,
    pub image_path: Option<String>,
    pub file_path: Option<String>,
    pub source_app: Option<String>,
    pub source_url: Option<String>,
    pub source_title: Option<String>,
    /// ISO 8601 UTC 格式（SQLite 层生成）
    pub captured_at: String,
    /// 原始 Unix ms 时间戳，便于按时间排序或重建
    pub captured_at_ms: i64,
    pub state: String,
    pub sensitive_type: Option<String>,
    pub blocked_reason: Option<String>,
    pub occurrence_count: i64,
    /// 图片源文件丢失时 true（JSON 里暴露，不阻塞导出）
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub image_missing: bool,
}

/// metadata.json 结构
#[derive(Debug, Serialize)]
struct Metadata {
    /// ISO 8601 UTC
    exported_at: String,
    teamo_version: &'static str,
    total_count: usize,
    format: ExportFormat,
    /// v1 为当前 schema。未来升级时用于向后兼容判断。
    schema_version: &'static str,
}

/// 主入口 — 按格式导出所有数据到 target_parent/teamo-export-YYYYMMDD-HHMMSS/
pub fn export_data(
    db: &AppDatabase,
    format: ExportFormat,
    target_parent: &Path,
) -> Result<ExportResult, String> {
    let conn = db.conn();

    // 1. 时间戳子目录
    let stamp = now_compact(&conn)?;
    let target_dir = target_parent.join(format!("teamo-export-{stamp}"));
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("create export dir: {e}"))?;
    let images_target = target_dir.join("images");
    std::fs::create_dir_all(&images_target)
        .map_err(|e| format!("create images dir: {e}"))?;

    // 2. 查所有数据
    let mut rows = fetch_all(&conn)?;

    // 3. 拷贝图片（同时在 rows 里标记 missing）
    let (image_count, missing_images) =
        copy_images(&mut rows, &db.images_dir(), &images_target)?;

    // 4. 主文件
    match format {
        ExportFormat::Json => json::write(&rows, &target_dir)?,
        ExportFormat::Markdown => markdown::write(&rows, &target_dir)?,
    }

    // 5. metadata
    write_metadata(&conn, &target_dir, rows.len(), format)?;

    // 6. README（AC-6：预留导入说明）
    write_readme(&target_dir)?;

    tracing::info!(
        "Export done: {} rows, {} images copied ({} missing), dir={}",
        rows.len(),
        image_count,
        missing_images,
        target_dir.display()
    );

    Ok(ExportResult {
        exported_count: rows.len(),
        image_count,
        missing_images,
        target_dir: target_dir.to_string_lossy().to_string(),
    })
}

fn fetch_all(conn: &Connection) -> Result<Vec<ExportRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, content, content_type, image_path, file_path, source_app,
                    source_url, source_title,
                    strftime('%Y-%m-%dT%H:%M:%SZ', captured_at/1000, 'unixepoch'),
                    captured_at,
                    state, sensitive_type, blocked_reason, occurrence_count
             FROM clipboard_local
             ORDER BY captured_at DESC",
        )
        .map_err(|e| format!("prepare export query: {e}"))?;

    let rows: Vec<ExportRow> = stmt
        .query_map([], |r| {
            Ok(ExportRow {
                id: r.get(0)?,
                content: r.get(1)?,
                content_type: r.get(2)?,
                image_path: r.get(3)?,
                file_path: r.get(4)?,
                source_app: r.get(5)?,
                source_url: r.get(6)?,
                source_title: r.get(7)?,
                captured_at: r.get(8)?,
                captured_at_ms: r.get(9)?,
                state: r.get(10)?,
                sensitive_type: r.get(11)?,
                blocked_reason: r.get(12)?,
                occurrence_count: r.get(13)?,
                image_missing: false,
            })
        })
        .map_err(|e| format!("execute export query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

fn copy_images(
    rows: &mut [ExportRow],
    source_dir: &Path,
    target_dir: &Path,
) -> Result<(usize, usize), String> {
    let mut copied = 0usize;
    let mut missing = 0usize;
    for row in rows.iter_mut() {
        if let Some(ref img) = row.image_path {
            let src = source_dir.join(img);
            let dst = target_dir.join(img);
            if !src.exists() {
                missing += 1;
                row.image_missing = true;
                continue;
            }
            std::fs::copy(&src, &dst)
                .map_err(|e| format!("copy image {img}: {e}"))?;
            copied += 1;
        }
    }
    Ok((copied, missing))
}

fn write_metadata(
    conn: &Connection,
    target: &Path,
    count: usize,
    format: ExportFormat,
) -> Result<(), String> {
    let md = Metadata {
        exported_at: now_iso(conn)?,
        teamo_version: env!("CARGO_PKG_VERSION"),
        total_count: count,
        format,
        schema_version: "v1",
    };
    let json = serde_json::to_string_pretty(&md).map_err(|e| e.to_string())?;
    std::fs::write(target.join("metadata.json"), json)
        .map_err(|e| format!("write metadata: {e}"))?;
    Ok(())
}

fn write_readme(target: &Path) -> Result<(), String> {
    let text = "\
Teamo 数据导出 · README
========================

目录结构：
- clipboard.json 或 clipboard.md  主数据文件
- images/                          图片副本（若有）
- metadata.json                    导出元信息（时间 / 版本 / 总数 / schema）
- README.txt                       本文件

数据说明：
- 本导出包含您 Teamo 应用中的所有本地数据（clipboard_local 表）
- 图片以原 PNG 格式存放于 images/ 下，文件名为原始 id.png
- 敏感数据（密码 / Token 等）在 Markdown 里会打码，JSON 保留原内容

导入回 Teamo：
- 当前版本（v0.1）暂不支持导入回 Teamo
- JSON 文件已包含 schema_version: v1，未来版本会提供导入工具
";
    std::fs::write(target.join("README.txt"), text)
        .map_err(|e| format!("write README: {e}"))?;
    Ok(())
}

// ── 时间工具：完全走 SQLite strftime，避免引入 chrono/time crate ──

fn now_iso(conn: &Connection) -> Result<String, String> {
    conn.query_row(
        "SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        [],
        |r| r.get::<_, String>(0),
    )
    .map_err(|e| format!("query now_iso: {e}"))
}

fn now_compact(conn: &Connection) -> Result<String, String> {
    conn.query_row(
        "SELECT strftime('%Y%m%d-%H%M%S', 'now')",
        [],
        |r| r.get::<_, String>(0),
    )
    .map_err(|e| format!("query now_compact: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{repository, AppDatabase};
    use tempfile::TempDir;

    fn setup_db() -> (AppDatabase, TempDir) {
        let tmp = TempDir::new().unwrap();
        let db = AppDatabase::init(tmp.path().to_path_buf()).unwrap();
        (db, tmp)
    }

    fn insert_text(db: &AppDatabase, id: &str, content: &str) {
        let conn = db.conn();
        let req = repository::InsertRequest {
            id: id.to_string(),
            content: Some(content.to_string()),
            content_type: "text".to_string(),
            source_app: Some("VS Code".to_string()),
            ..Default::default()
        };
        repository::insert_clipboard(&conn, req).unwrap();
    }

    #[test]
    fn test_export_json_roundtrip() {
        let (db, _tmp_db) = setup_db();
        for i in 0..10 {
            insert_text(&db, &format!("id-{i}"), &format!("content {i}"));
        }

        let export_parent = TempDir::new().unwrap();
        let result = export_data(&db, ExportFormat::Json, export_parent.path()).unwrap();
        assert_eq!(result.exported_count, 10);
        assert_eq!(result.image_count, 0);
        assert_eq!(result.missing_images, 0);

        // 解析回来字段一致
        let json_path = std::path::PathBuf::from(&result.target_dir).join("clipboard.json");
        let text = std::fs::read_to_string(&json_path).unwrap();
        let parsed: Vec<ExportRow> = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.len(), 10);
        assert_eq!(parsed[0].source_app.as_deref(), Some("VS Code"));
        // captured_at 是 ISO 字符串
        assert!(parsed[0].captured_at.starts_with("20"));
        assert!(parsed[0].captured_at.ends_with("Z"));
    }

    #[test]
    fn test_export_markdown() {
        let (db, _tmp_db) = setup_db();
        insert_text(&db, "uuid-1", "hello world");

        let export_parent = TempDir::new().unwrap();
        let result = export_data(&db, ExportFormat::Markdown, export_parent.path()).unwrap();

        let md_path = std::path::PathBuf::from(&result.target_dir).join("clipboard.md");
        let md = std::fs::read_to_string(&md_path).unwrap();
        assert!(md.contains("hello world"));
        assert!(md.contains("id: uuid-1"));
        assert!(md.contains("source_app: VS Code"));
    }

    #[test]
    fn test_export_metadata_and_readme() {
        let (db, _tmp_db) = setup_db();
        insert_text(&db, "x", "y");

        let export_parent = TempDir::new().unwrap();
        let result = export_data(&db, ExportFormat::Json, export_parent.path()).unwrap();

        let target = std::path::PathBuf::from(&result.target_dir);
        assert!(target.join("metadata.json").exists());
        assert!(target.join("README.txt").exists());
        assert!(target.join("images").is_dir());

        let metadata_text = std::fs::read_to_string(target.join("metadata.json")).unwrap();
        assert!(metadata_text.contains("\"schema_version\": \"v1\""));
        assert!(metadata_text.contains("\"total_count\": 1"));
    }

    #[test]
    fn test_export_image_missing_marked() {
        let (db, _tmp_db) = setup_db();
        // 必须 block scope 让 conn 的 MutexGuard 在 export_data 之前 drop——
        // export_data 内部会再 db.conn() 拿锁，std Mutex 不可重入会死锁
        {
            let conn = db.conn();
            let req = repository::InsertRequest {
                id: "img-1".to_string(),
                content: Some("fingerprint".to_string()),
                content_type: "image".to_string(),
                image_path: Some("ghost.png".to_string()),
                ..Default::default()
            };
            repository::insert_clipboard(&conn, req).unwrap();
        }

        let export_parent = TempDir::new().unwrap();
        let result = export_data(&db, ExportFormat::Json, export_parent.path()).unwrap();
        assert_eq!(result.image_count, 0);
        assert_eq!(result.missing_images, 1);

        let json_path = std::path::PathBuf::from(&result.target_dir).join("clipboard.json");
        let text = std::fs::read_to_string(&json_path).unwrap();
        let parsed: Vec<ExportRow> = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].image_missing);
    }

    #[test]
    fn test_export_image_bytes_exact() {
        let (db, _tmp_db) = setup_db();
        let img_bytes: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        let img_file = "real.png";
        std::fs::write(db.images_dir().join(img_file), &img_bytes).unwrap();

        // block scope 让 conn 的 MutexGuard 在 export_data 之前 drop（std Mutex 不可重入）
        {
            let conn = db.conn();
            let req = repository::InsertRequest {
                id: "img-real".to_string(),
                content: Some("fp".to_string()),
                content_type: "image".to_string(),
                image_path: Some(img_file.to_string()),
                ..Default::default()
            };
            repository::insert_clipboard(&conn, req).unwrap();
        }

        let export_parent = TempDir::new().unwrap();
        let result = export_data(&db, ExportFormat::Json, export_parent.path()).unwrap();
        assert_eq!(result.image_count, 1);

        let copied = std::path::PathBuf::from(&result.target_dir)
            .join("images")
            .join(img_file);
        let copied_bytes = std::fs::read(&copied).unwrap();
        assert_eq!(copied_bytes, img_bytes, "copied image should be byte-exact");
    }

    #[test]
    fn test_export_empty_db() {
        let (db, _tmp_db) = setup_db();
        let export_parent = TempDir::new().unwrap();
        let result = export_data(&db, ExportFormat::Json, export_parent.path()).unwrap();
        assert_eq!(result.exported_count, 0);

        let json_path = std::path::PathBuf::from(&result.target_dir).join("clipboard.json");
        let text = std::fs::read_to_string(&json_path).unwrap();
        assert_eq!(text, "[]");
    }
}
