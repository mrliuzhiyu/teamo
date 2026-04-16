// repository.rs · 业务读写函数
//
// clipboard_local 表的 CRUD + FTS5 搜索 + 归一化精确去重

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::canonicalize::canonicalize;

/// 剪切板记录行（前端可见）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardRow {
    pub id: String,
    pub content_hash: String,
    pub content: Option<String>,
    pub content_type: String,
    pub size_bytes: Option<i64>,
    pub image_path: Option<String>,
    pub file_path: Option<String>,
    pub source_app: Option<String>,
    pub source_url: Option<String>,
    pub source_title: Option<String>,
    pub captured_at: i64,
    pub sensitive_type: Option<String>,
    pub blocked_reason: Option<String>,
    pub state: String,
    pub server_id: Option<String>,
    pub occurrence_count: i64,
    pub last_seen_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 插入请求
pub struct InsertRequest {
    pub id: String,
    pub content: Option<String>,
    pub content_type: String,
    pub image_path: Option<String>,
    pub file_path: Option<String>,
    pub source_app: Option<String>,
}

/// 插入结果
pub enum InsertResult {
    /// 新记录已插入
    Inserted,
    /// 与已有记录归一化后等价，已更新 occurrence_count
    Deduplicated { existing_id: String },
}

/// 当前时间戳（Unix ms）
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// SHA256 哈希（pub 以便 clipboard 层对图片像素做指纹）
pub fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write;
    // 简单实现：用 rusqlite 不带 sha256，我们手动算
    // 为了不引新依赖，用一个最小的 SHA256
    let digest = sha256_digest(data);
    let mut hex = String::with_capacity(64);
    for byte in &digest {
        write!(hex, "{byte:02x}").unwrap();
    }
    hex
}

/// 最小化 SHA256 实现（纯 Rust，无外部依赖）
fn sha256_digest(data: &[u8]) -> [u8; 32] {
    // 内部函数，不对外暴露；外部调用 sha256_hex

    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // Padding
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    // Process blocks
    for chunk in padded.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut result = [0u8; 32];
    for (i, val) in h.iter().enumerate() {
        result[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
    }
    result
}

/// 插入剪切板记录（归一化精确去重）
///
/// content_hash = SHA256(canonicalize(content))
/// 30 秒窗口内同 hash → bump occurrence_count 而不新建行
pub fn insert_clipboard(conn: &Connection, req: InsertRequest) -> Result<InsertResult, rusqlite::Error> {
    let now = now_ms();
    let dedup_window = now - 30_000;

    let raw_content = req.content.as_deref().unwrap_or("");
    let canon = canonicalize(raw_content);
    let content_hash = sha256_hex(canon.as_bytes());
    let size_bytes = raw_content.len() as i64;

    let exact_dup: Option<String> = conn
        .query_row(
            "SELECT id FROM clipboard_local
             WHERE content_hash = ?1 AND captured_at > ?2
             ORDER BY captured_at DESC LIMIT 1",
            params![content_hash, dedup_window],
            |row| row.get(0),
        )
        .ok();

    if let Some(existing_id) = exact_dup {
        conn.execute(
            "UPDATE clipboard_local
             SET occurrence_count = occurrence_count + 1, last_seen_at = ?1, updated_at = ?1
             WHERE id = ?2",
            params![now, existing_id],
        )?;
        return Ok(InsertResult::Deduplicated { existing_id });
    }

    conn.execute(
        "INSERT INTO clipboard_local
         (id, content_hash, content, content_type, size_bytes, image_path, file_path,
          source_app, captured_at, state, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'captured', ?9)",
        params![
            req.id,
            content_hash,
            req.content,
            req.content_type,
            size_bytes,
            req.image_path,
            req.file_path,
            req.source_app,
            now,
        ],
    )?;

    Ok(InsertResult::Inserted)
}

/// FTS5 全文搜索
pub fn search_clipboard(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<ClipboardRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.content_hash, c.content, c.content_type, c.size_bytes,
                c.image_path, c.file_path, c.source_app, c.source_url, c.source_title,
                c.captured_at, c.sensitive_type, c.blocked_reason, c.state,
                c.server_id, c.occurrence_count, c.last_seen_at,
                c.created_at, c.updated_at
         FROM clipboard_fts f
         JOIN clipboard_local c ON c.rowid = f.rowid
         WHERE clipboard_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;

    let rows = stmt
        .query_map(params![query, limit], row_to_clipboard)?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// 列出最近记录
pub fn list_recent(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<ClipboardRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, content_hash, content, content_type, size_bytes,
                image_path, file_path, source_app, source_url, source_title,
                captured_at, sensitive_type, blocked_reason, state,
                server_id, occurrence_count, last_seen_at,
                created_at, updated_at
         FROM clipboard_local
         ORDER BY captured_at DESC
         LIMIT ?1 OFFSET ?2",
    )?;

    let rows = stmt
        .query_map(params![limit, offset], row_to_clipboard)?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// 获取单条记录详情
pub fn get_detail(conn: &Connection, id: &str) -> Result<Option<ClipboardRow>, rusqlite::Error> {
    let result = conn.query_row(
        "SELECT id, content_hash, content, content_type, size_bytes,
                image_path, file_path, source_app, source_url, source_title,
                captured_at, sensitive_type, blocked_reason, state,
                server_id, occurrence_count, last_seen_at,
                created_at, updated_at
         FROM clipboard_local
         WHERE id = ?1",
        params![id],
        row_to_clipboard,
    );

    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// 忘记一条记录（删除行 + 关联图片文件）
pub fn forget(conn: &Connection, id: &str, images_dir: &std::path::Path) -> Result<bool, rusqlite::Error> {
    // 先查关联图片路径
    let image_path: Option<String> = conn
        .query_row(
            "SELECT image_path FROM clipboard_local WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let affected = conn.execute("DELETE FROM clipboard_local WHERE id = ?1", params![id])?;

    // 删除关联图片文件
    if let Some(img) = image_path {
        let full_path = images_dir.join(&img);
        if full_path.exists() {
            let _ = std::fs::remove_file(full_path);
        }
    }

    Ok(affected > 0)
}

/// 今日统计（按本地时区）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodayStats {
    pub captured: i64,
    pub blocked: i64,
    pub uploaded: i64,
}

/// 查询今日统计：已记 / 拦截 / 上云
///
/// 用 SQLite 的 `DATE(..., 'unixepoch', 'localtime')` 对 captured_at（Unix ms）
/// 做本地时区的日期归一化——避免引入 chrono 依赖。
pub fn get_today_stats(conn: &Connection) -> Result<TodayStats, rusqlite::Error> {
    conn.query_row(
        "SELECT
           COUNT(*),
           COALESCE(SUM(CASE WHEN blocked_reason IS NOT NULL THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN state = 'uploaded' THEN 1 ELSE 0 END), 0)
         FROM clipboard_local
         WHERE DATE(captured_at/1000, 'unixepoch', 'localtime') = DATE('now', 'localtime')",
        [],
        |row| {
            Ok(TodayStats {
                captured: row.get(0)?,
                blocked: row.get(1)?,
                uploaded: row.get(2)?,
            })
        },
    )
}

/// 获取设置值
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    let result = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    );
    match result {
        Ok(val) => Ok(val),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// 设置值
pub fn set_setting(conn: &Connection, key: &str, value: Option<&str>) -> Result<(), rusqlite::Error> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
        params![key, value, now],
    )?;
    Ok(())
}

/// 从 rusqlite Row 解析 ClipboardRow
fn row_to_clipboard(row: &rusqlite::Row<'_>) -> Result<ClipboardRow, rusqlite::Error> {
    Ok(ClipboardRow {
        id: row.get(0)?,
        content_hash: row.get(1)?,
        content: row.get(2)?,
        content_type: row.get(3)?,
        size_bytes: row.get(4)?,
        image_path: row.get(5)?,
        file_path: row.get(6)?,
        source_app: row.get(7)?,
        source_url: row.get(8)?,
        source_title: row.get(9)?,
        captured_at: row.get(10)?,
        sensitive_type: row.get(11)?,
        blocked_reason: row.get(12)?,
        state: row.get(13)?,
        server_id: row.get(14)?,
        occurrence_count: row.get(15)?,
        last_seen_at: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::schema;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        schema::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_insert_and_list() {
        let conn = setup_db();

        let req = InsertRequest {
            id: "uuid-1".to_string(),
            content: Some("hello world 你好世界".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: Some("VS Code".to_string()),
        };

        let result = insert_clipboard(&conn, req).unwrap();
        assert!(matches!(result, InsertResult::Inserted));

        let items = list_recent(&conn, 10, 0).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "uuid-1");
        assert_eq!(items[0].content.as_deref(), Some("hello world 你好世界"));
        assert_eq!(items[0].source_app.as_deref(), Some("VS Code"));
    }

    #[test]
    fn test_exact_dedup() {
        let conn = setup_db();

        let req1 = InsertRequest {
            id: "uuid-1".to_string(),
            content: Some("duplicate content".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
        };
        let req2 = InsertRequest {
            id: "uuid-2".to_string(),
            content: Some("duplicate content".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
        };

        insert_clipboard(&conn, req1).unwrap();
        let result = insert_clipboard(&conn, req2).unwrap();

        assert!(matches!(result, InsertResult::Deduplicated { .. }));

        let items = list_recent(&conn, 10, 0).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].occurrence_count, 2);
    }

    #[test]
    fn test_canonical_dedup_trailing_punct() {
        // 末尾多一个句号 → 归一化后等价 → 去重
        let conn = setup_db();

        let req1 = InsertRequest {
            id: "uuid-1".to_string(),
            content: Some("Rust 是一门注重安全速度和并发的编程语言".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
        };
        let req2 = InsertRequest {
            id: "uuid-2".to_string(),
            content: Some("Rust 是一门注重安全速度和并发的编程语言。".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
        };

        insert_clipboard(&conn, req1).unwrap();
        let result = insert_clipboard(&conn, req2).unwrap();

        assert!(matches!(result, InsertResult::Deduplicated { .. }));
    }

    #[test]
    fn test_canonical_dedup_trailing_whitespace() {
        // 末尾多空白 / 首尾空白 → 归一化后等价 → 去重
        let conn = setup_db();

        let req1 = InsertRequest {
            id: "uuid-1".to_string(),
            content: Some("hello world".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
        };
        let req2 = InsertRequest {
            id: "uuid-2".to_string(),
            content: Some("  hello world\n".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
        };

        insert_clipboard(&conn, req1).unwrap();
        let result = insert_clipboard(&conn, req2).unwrap();

        assert!(matches!(result, InsertResult::Deduplicated { .. }));
    }

    #[test]
    fn test_image_dedup_by_pixel_hash() {
        // 两张不同图片：content 各自是 pixel SHA256 hex → 不应误判重复
        let conn = setup_db();

        let req1 = InsertRequest {
            id: "img-1".to_string(),
            content: Some("a3b5c7d9e1f20000000000000000000000000000000000000000000000000000".to_string()),
            content_type: "image".to_string(),
            image_path: Some("img-1.png".to_string()),
            file_path: None,
            source_app: None,
        };
        let req2 = InsertRequest {
            id: "img-2".to_string(),
            content: Some("ff00aa11bb22cc33000000000000000000000000000000000000000000000000".to_string()),
            content_type: "image".to_string(),
            image_path: Some("img-2.png".to_string()),
            file_path: None,
            source_app: None,
        };

        assert!(matches!(insert_clipboard(&conn, req1).unwrap(), InsertResult::Inserted));
        assert!(matches!(insert_clipboard(&conn, req2).unwrap(), InsertResult::Inserted));

        let items = list_recent(&conn, 10, 0).unwrap();
        assert_eq!(items.len(), 2, "两张不同图片应各存一条");
    }

    #[test]
    fn test_image_dedup_same_pixel_hash() {
        // 同一张图片（相同 pixel SHA256）→ 应去重
        let conn = setup_db();

        let fingerprint = "deadbeef00000000000000000000000000000000000000000000000000000000";
        let req1 = InsertRequest {
            id: "img-1".to_string(),
            content: Some(fingerprint.to_string()),
            content_type: "image".to_string(),
            image_path: Some("img-1.png".to_string()),
            file_path: None,
            source_app: None,
        };
        let req2 = InsertRequest {
            id: "img-2".to_string(),
            content: Some(fingerprint.to_string()),
            content_type: "image".to_string(),
            image_path: Some("img-2.png".to_string()),
            file_path: None,
            source_app: None,
        };

        insert_clipboard(&conn, req1).unwrap();
        let result = insert_clipboard(&conn, req2).unwrap();

        assert!(matches!(result, InsertResult::Deduplicated { .. }));
        let items = list_recent(&conn, 10, 0).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].occurrence_count, 2);
    }

    #[test]
    fn test_word_change_not_dedup() {
        // 词级修改是两次真实的不同复制，不该去重
        let conn = setup_db();

        let req1 = InsertRequest {
            id: "uuid-1".to_string(),
            content: Some("Rust 注重安全的编程语言".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
        };
        let req2 = InsertRequest {
            id: "uuid-2".to_string(),
            content: Some("Rust 注重速度的编程语言".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
        };

        insert_clipboard(&conn, req1).unwrap();
        let result = insert_clipboard(&conn, req2).unwrap();

        assert!(matches!(result, InsertResult::Inserted));
        let items = list_recent(&conn, 10, 0).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_search() {
        let conn = setup_db();

        for i in 0..5 {
            let req = InsertRequest {
                id: format!("uuid-{i}"),
                content: Some(format!("item {i}: Rust 编程语言 sample {i}")),
                content_type: "text".to_string(),
                image_path: None,
                file_path: None,
                source_app: None,
            };
            insert_clipboard(&conn, req).unwrap();
        }

        let results = search_clipboard(&conn, "Rust", 10).unwrap();
        assert_eq!(results.len(), 5);

        let results = search_clipboard(&conn, "sample 3", 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_get_detail() {
        let conn = setup_db();

        let req = InsertRequest {
            id: "uuid-detail".to_string(),
            content: Some("detail test".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: Some("Chrome".to_string()),
        };
        insert_clipboard(&conn, req).unwrap();

        let detail = get_detail(&conn, "uuid-detail").unwrap();
        assert!(detail.is_some());
        assert_eq!(detail.unwrap().source_app.as_deref(), Some("Chrome"));

        let missing = get_detail(&conn, "nonexistent").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_forget() {
        let conn = setup_db();
        let tmp = std::env::temp_dir();

        let req = InsertRequest {
            id: "uuid-forget".to_string(),
            content: Some("to be forgotten".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
        };
        insert_clipboard(&conn, req).unwrap();

        let deleted = forget(&conn, "uuid-forget", &tmp).unwrap();
        assert!(deleted);

        let items = list_recent(&conn, 10, 0).unwrap();
        assert!(items.is_empty());

        // 搜索也找不到了
        let results = search_clipboard(&conn, "forgotten", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_settings() {
        let conn = setup_db();

        let autostart = get_setting(&conn, "autostart").unwrap();
        assert_eq!(autostart.as_deref(), Some("1"));

        set_setting(&conn, "theme", Some("dark")).unwrap();
        let theme = get_setting(&conn, "theme").unwrap();
        assert_eq!(theme.as_deref(), Some("dark"));

        let missing = get_setting(&conn, "nonexistent").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_insert_100_and_search_chinese() {
        let conn = setup_db();

        let samples = vec![
            "今天学了 Rust 的所有权系统",
            "React 组件生命周期详解",
            "深度学习模型训练技巧",
            "如何写好技术文档",
            "PostgreSQL 性能优化指南",
        ];

        for i in 0..100 {
            let content = format!("{} — 扩展内容第 {i} 条", samples[i % samples.len()]);
            let req = InsertRequest {
                id: format!("batch-{i}"),
                content: Some(content),
                content_type: "text".to_string(),
                image_path: None,
                file_path: None,
                source_app: None,
            };
            insert_clipboard(&conn, req).unwrap();
        }

        let total = list_recent(&conn, 200, 0).unwrap();
        assert_eq!(total.len(), 100);

        // 搜索中文
        let results = search_clipboard(&conn, "Rust", 10).unwrap();
        assert!(!results.is_empty(), "should find Rust entries");

        let results = search_clipboard(&conn, "PostgreSQL", 10).unwrap();
        assert!(!results.is_empty(), "should find PostgreSQL entries");
    }
}
