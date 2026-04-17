//! Markdown 格式化 — 每条 row 带 frontmatter + 内容
//!
//! - 敏感内容打码（JSON 里保留原文，Markdown 不保留）
//! - 图片用 `![](images/xxx.png)` 引用
//! - frontmatter 字段按 AC-2 示例格式

use std::fmt::Write;
use std::path::Path;

use super::ExportRow;

pub fn write(rows: &[ExportRow], target: &Path) -> Result<(), String> {
    let mut out = String::new();
    out.push_str("# Teamo Export\n\n");
    out.push_str(&format!("共 {} 条记录，按时间倒序。\n\n", rows.len()));

    for row in rows {
        render_row(&mut out, row);
    }

    std::fs::write(target.join("clipboard.md"), out)
        .map_err(|e| format!("write clipboard.md: {e}"))?;
    Ok(())
}

fn render_row(out: &mut String, row: &ExportRow) {
    out.push_str("---\n");
    let _ = writeln!(out, "id: {}", row.id);
    let _ = writeln!(out, "captured_at: {}", row.captured_at);
    let _ = writeln!(
        out,
        "source_app: {}",
        row.source_app.as_deref().unwrap_or("null")
    );
    let _ = writeln!(
        out,
        "source_url: {}",
        row.source_url.as_deref().unwrap_or("null")
    );
    let _ = writeln!(out, "state: {}", row.state);
    let _ = writeln!(
        out,
        "sensitive_type: {}",
        row.sensitive_type.as_deref().unwrap_or("null")
    );
    out.push_str("---\n\n");

    if let Some(ref stype) = row.sensitive_type {
        // 敏感：打码 + 类型提示
        let _ = writeln!(out, "••••••• [拦截：{}]\n", stype);
    } else if row.content_type == "image" {
        if let Some(ref img) = row.image_path {
            if row.image_missing {
                let _ = writeln!(out, "_[图片丢失：{}]_\n", img);
            } else {
                let _ = writeln!(out, "![](images/{})\n", img);
            }
        }
    } else if let Some(ref content) = row.content {
        out.push_str(content);
        out.push_str("\n\n");
    }

    out.push_str("---\n\n");
}
