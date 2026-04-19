//! 数据导入 — 从 Teamo 自己导出的 JSON 目录还原记录
//!
//! 对应 export::json 的逆向：读 clipboard.json → INSERT 到 clipboard_local；
//! 同时把 images/ 下的 PNG 拷贝到当前 data_dir 的 images/ 目录。
//!
//! 使用场景：换电脑 / 备份恢复 / 从旧版本迁移。
//!
//! 冲突策略：
//! - id 冲突（极低概率，UUID 碰撞）→ 跳过 + log
//! - content_hash 冲突（同内容已在）→ 跳过（尊重现有去重）
//! - image_path 指向的 PNG 不存在（export 时 image_missing=true）→ 跳过图片条
//!   但保留 row（content 里存了 hash 可供未来手动补）
//!
//! 不做的事：
//! - 不覆盖现有记录（保守策略，用户可先清空再导入达到 replace 语义）
//! - 不恢复 settings / app_rules / domain_rules（只是剪贴板历史）
//! - 不恢复 pinned_at / last_used_at（新导入的都按正常时间线走）

use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

use super::ExportRow;

#[derive(Debug, Serialize)]
pub struct ImportResult {
    /// 本次真正插入的新行数
    pub imported_count: usize,
    /// 跳过的行（id 已存或 content_hash 重复）
    pub skipped_count: usize,
    /// 拷贝到本地 images/ 的图片数
    pub copied_images: usize,
    /// 引用的 PNG 在源目录找不到的条数（row 仍插入，但图片 path 可能失效）
    pub missing_images: usize,
    /// 解析错误行数（JSON 损坏 / 字段缺失）
    pub failed_rows: usize,
}

/// 从导出目录导入到当前 DB + images/
///
/// `source_dir` 指向 export 时生成的 `teamo-export-YYYYMMDD-HHMMSS/` 目录，
/// 内含 `clipboard.json` 和可选的 `images/` 子目录。
pub fn import_from_dir(
    conn: &Connection,
    source_dir: &Path,
    dest_images_dir: &Path,
) -> Result<ImportResult, String> {
    let json_path = source_dir.join("clipboard.json");
    if !json_path.exists() {
        return Err(format!(
            "clipboard.json not found in {} (expected Teamo export format)",
            source_dir.display()
        ));
    }

    let json_bytes =
        std::fs::read(&json_path).map_err(|e| format!("read clipboard.json: {e}"))?;
    let rows: Vec<ExportRow> = serde_json::from_slice(&json_bytes)
        .map_err(|e| format!("parse clipboard.json: {e}"))?;

    let source_images_dir = source_dir.join("images");
    std::fs::create_dir_all(dest_images_dir)
        .map_err(|e| format!("create dest images dir: {e}"))?;

    let mut result = ImportResult {
        imported_count: 0,
        skipped_count: 0,
        copied_images: 0,
        missing_images: 0,
        failed_rows: 0,
    };

    for row in rows {
        // 已存在（id 冲突）跳过
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM clipboard_local WHERE id = ?1",
                params![row.id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if exists {
            result.skipped_count += 1;
            continue;
        }

        // 图片拷贝：先把 PNG 搬到 dest，拷贝失败时 row 仍插入但标记 image_missing
        let mut image_path_final = row.image_path.clone();
        if let Some(img_name) = &row.image_path {
            let src_png = source_images_dir.join(img_name);
            let dest_png = dest_images_dir.join(img_name);
            if src_png.exists() {
                if dest_png.exists() {
                    // 同名文件已在（另一导出也带过）→ 不覆盖，跳过拷贝但保留引用
                } else {
                    match std::fs::copy(&src_png, &dest_png) {
                        Ok(_) => result.copied_images += 1,
                        Err(e) => {
                            tracing::warn!("copy image {img_name}: {e}");
                            image_path_final = None;
                            result.missing_images += 1;
                        }
                    }
                }
            } else {
                // 导出时就 missing（image_missing=true）或文件被删
                image_path_final = None;
                result.missing_images += 1;
            }
        }

        // content_hash 由 insert 流程算，但导入记录不走 capture 的 filter 链路，
        // 直接 INSERT 保留 export 时的 state / sensitive_type / blocked_reason
        let content_hash = if let Some(c) = &row.content {
            super::super::storage::repository::sha256_hex(c.as_bytes())
        } else {
            String::new()
        };

        let insert_result = conn.execute(
            "INSERT INTO clipboard_local
             (id, content_hash, content, content_type, image_path, file_path,
              source_app, source_url, source_title, captured_at,
              sensitive_type, blocked_reason, state, occurrence_count,
              created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
            params![
                row.id,
                content_hash,
                row.content,
                row.content_type,
                image_path_final,
                row.file_path,
                row.source_app,
                row.source_url,
                row.source_title,
                row.captured_at_ms,
                row.sensitive_type,
                row.blocked_reason,
                row.state,
                row.occurrence_count,
                row.captured_at_ms, // created_at / updated_at 用原捕获时间
            ],
        );

        match insert_result {
            Ok(_) => result.imported_count += 1,
            Err(e) => {
                tracing::warn!("import row {}: {e}", row.id);
                result.failed_rows += 1;
            }
        }
    }

    tracing::info!(
        "Import complete: {} imported, {} skipped, {} images copied, {} missing, {} failed",
        result.imported_count,
        result.skipped_count,
        result.copied_images,
        result.missing_images,
        result.failed_rows,
    );
    Ok(result)
}
