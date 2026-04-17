//! JSON 格式化 — pretty-printed（缩进 2）、UTF-8 无 BOM

use std::path::Path;

use super::ExportRow;

pub fn write(rows: &[ExportRow], target: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(rows).map_err(|e| format!("serialize JSON: {e}"))?;
    std::fs::write(target.join("clipboard.json"), json)
        .map_err(|e| format!("write clipboard.json: {e}"))?;
    Ok(())
}
