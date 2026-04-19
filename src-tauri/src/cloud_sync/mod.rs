//! R3.2 session → memo 上云管道。
//!
//! 架构：
//! - upload_session command 接 session_id
//! - sync 部分：查 items + 三道闸门过滤 + 组装 memo（含 markdown 拼接）
//! - async 部分：调 /api/memos/batch（带 access_token）
//!
//! 三道闸门保护（客户端先拦）：
//! 1. sensitive_type 命中 → 跳
//! 2. state == 'local_only' / blocked_reason 非空 → 跳
//! 3. content_type != 'text'/'url' → 跳（图片 R3.3 单独走 upload 端点）
//!
//! 敏感内容永不上云 = 架构性保证（不是云端判断后拒，是本地根本不发）

use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

use crate::storage::repository::{self, ClipboardRow};

/// 图片上云单图上限（后端 MAX_IMAGE_SIZE 10MB，客户端压缩到这以内）
const IMAGE_UPLOAD_MAX_BYTES: usize = 10 * 1024 * 1024;

const SETTING_DEVICE_UUID: &str = "device.uuid";

#[derive(Debug, Serialize)]
pub struct UploadSessionResult {
    /// 本次上云的 memo 数（当前固定 1 — 一个 session → 一条 memo）
    pub uploaded_count: usize,
    /// 被过滤掉未上云的 items 数（sensitive / local_only / image / 空内容）
    pub skipped_items: usize,
    /// 参与上云的 items 数
    pub included_items: usize,
}

/// 获取或生成 device_uuid（跨 Teamo 会话稳定，用于 memo.clientId）
pub fn get_or_create_device_id(conn: &Connection) -> Result<String, String> {
    if let Ok(Some(existing)) = repository::get_setting(conn, SETTING_DEVICE_UUID) {
        if !existing.is_empty() {
            return Ok(existing);
        }
    }
    // 复用 clipboard::generate_id 的 UUID 风格（无需新依赖）
    let new_id = crate::clipboard::generate_id();
    repository::set_setting(conn, SETTING_DEVICE_UUID, Some(&new_id))
        .map_err(|e| format!("save device uuid: {e}"))?;
    Ok(new_id)
}

/// 三道闸门：筛出可以上云的 items（非敏感 / 非 blocked / 支持类型）。
/// R3.3 起放开图片：image 类型 + 有 image_path + 非敏感 → 参与上云
pub fn filter_cloud_safe<'a>(items: &'a [ClipboardRow]) -> Vec<&'a ClipboardRow> {
    items
        .iter()
        .filter(|r| {
            // 闸门 1：敏感
            if r.sensitive_type.is_some() {
                return false;
            }
            // 闸门 2：state 或 blocked_reason
            if r.state == "local_only" {
                return false;
            }
            if r.blocked_reason.is_some() {
                return false;
            }
            // 闸门 3：类型必须是 text / url / image，且有可用内容
            match r.content_type.as_str() {
                "text" | "url" => match r.content.as_deref() {
                    Some(c) if !c.trim().is_empty() => true,
                    _ => false,
                },
                "image" => r.image_path.is_some(),
                _ => false, // file 类型 Phase 4+ 支持
            }
        })
        .collect()
}

/// 组装 markdown content：主文（父节点）为主体，子节点作引用列表，孤儿独立段落。
fn build_markdown_content(
    source_app: &str,
    started_at: i64,
    ended_at: i64,
    items: &[&ClipboardRow],
) -> String {
    // parent_id=None 的作为顶层（主文或独立片段）
    // parent_id=Some 的按 parent 分组
    let mut children_by_parent: HashMap<&str, Vec<&ClipboardRow>> = HashMap::new();
    let mut top_level: Vec<&&ClipboardRow> = Vec::new();
    for item in items {
        match &item.parent_id {
            Some(pid) => children_by_parent
                .entry(pid.as_str())
                .or_default()
                .push(item),
            None => top_level.push(item),
        }
    }
    // 顶层按 captured_at DESC（最新主文在前）
    top_level.sort_by_key(|i| std::cmp::Reverse(i.captured_at));
    // 子项按 captured_at ASC（原始复制顺序）
    for kids in children_by_parent.values_mut() {
        kids.sort_by_key(|i| i.captured_at);
    }

    let mut md = String::new();
    md.push_str(&format!(
        "# {} · {}\n\n",
        source_app,
        format_time_range(started_at, ended_at)
    ));
    md.push_str(&format!("共 {} 个片段\n\n---\n\n", items.len()));

    for parent in &top_level {
        if let Some(c) = parent.content.as_deref() {
            md.push_str(c);
            md.push_str("\n\n");
        }
        if let Some(kids) = children_by_parent.get(parent.id.as_str()) {
            md.push_str("## 引用片段\n\n");
            for kid in kids {
                if let Some(c) = kid.content.as_deref() {
                    // 子片段限 200 字避免 markdown 噪音
                    let snippet: String = c.chars().take(200).collect();
                    md.push_str(&format!("> {}\n\n", snippet.replace('\n', " ")));
                }
            }
        }
    }

    md
}

/// 上传单张图片到 /api/images/upload，返回公开 URL。
/// 超过 10MB 或读失败返 Err；调用方决定是否 fallback（跳过 or 压缩重试）
pub async fn upload_image_to_cloud(
    local_path: &Path,
) -> Result<String, String> {
    let bytes = std::fs::read(local_path)
        .map_err(|e| format!("读取图片失败 {}: {e}", local_path.display()))?;
    if bytes.len() > IMAGE_UPLOAD_MAX_BYTES {
        return Err(format!(
            "图片 {} KB 超过云端 10MB 上限",
            bytes.len() / 1024
        ));
    }
    let filename = local_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image.png")
        .to_string();

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename)
        .mime_str("image/png")
        .map_err(|e| format!("mime: {e}"))?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("document_id", ""); // 后端允许空

    let resp = crate::auth::http::authed_multipart("/api/images/upload", form).await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("图片上传失败（{status}）：{text}"));
    }

    #[derive(serde::Deserialize)]
    struct UploadResp {
        url: String,
    }
    let body: UploadResp = resp
        .json()
        .await
        .map_err(|e| format!("上传响应解析失败：{e}"))?;
    Ok(body.url)
}

fn format_time_range(start_ms: i64, end_ms: i64) -> String {
    use chrono::{DateTime, TimeZone, Utc};
    let start: DateTime<Utc> = Utc
        .timestamp_millis_opt(start_ms)
        .single()
        .unwrap_or_else(Utc::now);
    let end: DateTime<Utc> = Utc
        .timestamp_millis_opt(end_ms)
        .single()
        .unwrap_or_else(Utc::now);
    let start_local = start.with_timezone(&chrono::Local);
    let end_local = end.with_timezone(&chrono::Local);
    if start_local.date_naive() == end_local.date_naive() {
        format!(
            "{} {}–{}",
            start_local.format("%Y-%m-%d"),
            start_local.format("%H:%M"),
            end_local.format("%H:%M")
        )
    } else {
        format!(
            "{} – {}",
            start_local.format("%Y-%m-%d %H:%M"),
            end_local.format("%m-%d %H:%M")
        )
    }
}

/// 组装完整 memo payload（memos.batch 入参格式）
///
/// `image_urls` 是调用方预先上传后拿到的 `row.id -> 云端 URL` 映射。
/// 图片 item 在 raw_items 里带 image_url 字段（Web 端未来渲染）；文本 items 照原样。
pub fn build_session_memo(
    session_id: &str,
    items: &[&ClipboardRow],
    device_id: &str,
    image_urls: &HashMap<String, String>,
) -> Value {
    let source_app = items
        .iter()
        .find_map(|i| i.source_app.as_deref())
        .unwrap_or("Unknown");
    let started = items.iter().map(|i| i.captured_at).min().unwrap_or(0);
    let ended = items.iter().map(|i| i.captured_at).max().unwrap_or(0);
    let content = build_markdown_content(source_app, started, ended, items);

    let raw_items: Vec<Value> = items
        .iter()
        .map(|i| {
            let mut obj = json!({
                "id": i.id,
                "content_type": i.content_type,
                "captured_at": i.captured_at,
                "parent_id": i.parent_id,
            });
            if i.content_type == "image" {
                obj["image_width"] = json!(i.image_width);
                obj["image_height"] = json!(i.image_height);
                if let Some(url) = image_urls.get(&i.id) {
                    obj["image_url"] = json!(url);
                }
            } else {
                obj["content"] = json!(i.content);
            }
            obj
        })
        .collect();

    json!({
        "id": format!("teamo-session-{}", session_id),
        "content": content,
        "source": "teamo_desktop",
        "clientId": format!("teamo_desktop_{}", device_id),
        "createdAt": started,
        "updatedAt": ended,
        "version": 1,
        "attachments": {
            "teamo_source_type": "session",
            "session_id": session_id,
            "source_app": source_app,
            "started_at": started,
            "ended_at": ended,
            "item_count": items.len(),
            "raw_items": raw_items
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, content: &str, sensitive: Option<&str>, state: &str) -> ClipboardRow {
        ClipboardRow {
            id: id.to_string(),
            content_hash: String::new(),
            content: Some(content.to_string()),
            content_type: "text".to_string(),
            size_bytes: None,
            image_path: None,
            file_path: None,
            source_app: Some("Chrome.exe".to_string()),
            source_url: None,
            source_title: None,
            captured_at: 0,
            sensitive_type: sensitive.map(String::from),
            blocked_reason: None,
            state: state.to_string(),
            server_id: None,
            occurrence_count: 1,
            last_seen_at: None,
            created_at: 0,
            updated_at: 0,
            matched_domain_rule: None,
            pinned_at: None,
            last_used_at: None,
            image_width: None,
            image_height: None,
            session_id: Some("s1".to_string()),
            parent_id: None,
        }
    }

    #[test]
    fn test_filter_excludes_sensitive() {
        let items = vec![
            row("a", "normal content", None, "captured"),
            row("b", "password here", Some("password"), "local_only"),
        ];
        let filtered = filter_cloud_safe(&items);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "a");
    }

    #[test]
    fn test_filter_excludes_local_only() {
        let items = vec![
            row("a", "normal", None, "captured"),
            row("b", "private", None, "local_only"),
        ];
        let filtered = filter_cloud_safe(&items);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_filter_excludes_image() {
        let mut img = row("img", "hash", None, "captured");
        img.content_type = "image".to_string();
        let items = vec![row("a", "text", None, "captured"), img];
        let filtered = filter_cloud_safe(&items);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "a");
    }

    #[test]
    fn test_build_memo_has_required_fields() {
        let items = vec![row("a", "hello world", None, "captured")];
        let refs: Vec<&ClipboardRow> = items.iter().collect();
        let memo = build_session_memo("s1", &refs, "dev-uuid", &HashMap::new());
        assert_eq!(memo["id"], "teamo-session-s1");
        assert_eq!(memo["source"], "teamo_desktop");
        assert_eq!(memo["clientId"], "teamo_desktop_dev-uuid");
        assert_eq!(memo["attachments"]["session_id"], "s1");
        assert_eq!(memo["attachments"]["item_count"], 1);
        assert!(memo["content"].as_str().unwrap().contains("hello world"));
    }

    #[test]
    fn test_markdown_has_parent_child_structure() {
        let mut parent = row("p", "long parent text content", None, "captured");
        parent.captured_at = 100;
        let mut child = row("c", "child quote", None, "captured");
        child.captured_at = 200;
        child.parent_id = Some("p".to_string());
        let items = vec![parent, child];
        let refs: Vec<&ClipboardRow> = items.iter().collect();
        let memo = build_session_memo("s1", &refs, "uuid", &HashMap::new());
        let content = memo["content"].as_str().unwrap();
        assert!(content.contains("long parent text content"));
        assert!(content.contains("## 引用片段"));
        assert!(content.contains("child quote"));
    }

    #[test]
    fn test_image_item_carries_url_in_raw_items() {
        let mut img = row("img1", "hash-fingerprint", None, "captured");
        img.content_type = "image".to_string();
        img.image_path = Some("uuid.png".to_string());
        img.image_width = Some(1920);
        img.image_height = Some(1080);
        let items = vec![row("a", "text", None, "captured"), img];
        let refs: Vec<&ClipboardRow> = items.iter().collect();
        let mut urls = HashMap::new();
        urls.insert("img1".to_string(), "https://cdn.example.com/img1.png".to_string());
        let memo = build_session_memo("s1", &refs, "uuid", &urls);
        let raw_items = memo["attachments"]["raw_items"].as_array().unwrap();
        let img_item = raw_items.iter().find(|r| r["id"] == "img1").unwrap();
        assert_eq!(img_item["content_type"], "image");
        assert_eq!(img_item["image_url"], "https://cdn.example.com/img1.png");
        assert_eq!(img_item["image_width"], 1920);
        // 文本 item 有 content，图片 item 没有 content 字段（content = hash 指纹无意义给前端）
        assert!(img_item.get("content").is_none());
    }

    #[test]
    fn test_filter_includes_image_when_has_path() {
        let mut img = row("img", "hash", None, "captured");
        img.content_type = "image".to_string();
        img.image_path = Some("foo.png".to_string());
        let items = vec![img];
        let filtered = filter_cloud_safe(&items);
        assert_eq!(filtered.len(), 1, "image with path should pass filter");
    }

    #[test]
    fn test_filter_excludes_image_without_path() {
        let mut img = row("img", "hash", None, "captured");
        img.content_type = "image".to_string();
        img.image_path = None;
        let items = vec![img];
        let filtered = filter_cloud_safe(&items);
        assert_eq!(filtered.len(), 0);
    }
}
