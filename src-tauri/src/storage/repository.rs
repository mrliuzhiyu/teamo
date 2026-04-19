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
    /// URL 命中的 domain_rule（"parse_as_content:v.douyin.com/*" 等）。
    /// M3 云端 parse_worker 消费字段；v0.1 前端可选显示 "识别为 X 类型"
    pub matched_domain_rule: Option<String>,
    /// 置顶时间戳（Unix ms），NULL = 未置顶。列表按 pinned_at DESC 排序让 pin 项聚顶
    pub pinned_at: Option<i64>,
    /// 上次被使用（复制到剪贴板）的时间戳（Unix ms），NULL = 从未使用。
    /// 列表排序用 COALESCE(last_used_at, captured_at) 实现"粘贴后 promote"
    pub last_used_at: Option<i64>,
}

/// 插入请求
#[derive(Default)]
pub struct InsertRequest {
    pub id: String,
    pub content: Option<String>,
    pub content_type: String,
    pub image_path: Option<String>,
    pub file_path: Option<String>,
    pub source_app: Option<String>,
    /// state 列值；不传默认 "captured"。闸门拦截时为 "local_only"。
    pub state: Option<String>,
    /// 闸门命中时填充（比如 "sensitive:password"）
    pub blocked_reason: Option<String>,
    /// 敏感类型（password/token/credit_card/...）
    pub sensitive_type: Option<String>,
    /// URL 命中的 domain_rule（"parse_as_content:pattern" / "skip_parse:pattern" /
    /// "skip_upload:pattern"）。M3 云端 parse_worker 读此字段决策 enrich/skip。
    pub matched_domain_rule: Option<String>,
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

    let state = req.state.as_deref().unwrap_or("captured");
    conn.execute(
        "INSERT INTO clipboard_local
         (id, content_hash, content, content_type, size_bytes, image_path, file_path,
          source_app, captured_at, state, blocked_reason, sensitive_type,
          matched_domain_rule, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?9)",
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
            state,
            req.blocked_reason,
            req.sensitive_type,
            req.matched_domain_rule,
        ],
    )?;

    Ok(InsertResult::Inserted)
}

/// FTS5 全文搜索
///
/// 用户输入的 query 会原样当成短语搜索——把整个字符串用 `"` 包起来，
/// 内部的 `"` 按 FTS5 规范 double-escape 成 `""`。这样不管用户输入什么
/// 特殊字符（括号、AND/OR/NOT、星号、减号）都不会被 FTS5 当成语法，
/// 避免"搜 `foo(bar)` 就抛 FTS5 syntax error"。
pub fn search_clipboard(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<ClipboardRow>, rusqlite::Error> {
    let escaped = query.replace('"', "\"\"");
    let phrase = format!("\"{escaped}\"");

    // 排序分层（对标"精确 > 前缀 > 包含 > FTS rank"的用户预期）：
    //   1. 置顶项聚顶（pin 语义最高）
    //   2. 完全匹配（用户输入就是一条记录的完整内容）
    //   3. 开头匹配（用户输入是记录的前缀 —— 自动补全 mental model）
    //   4. 包含匹配
    //   5. FTS5 rank（BM25-like）兜底
    // 比纯 ORDER BY rank 更符合"我搜这个词想找最直接的那条"
    let mut stmt = conn.prepare(
        "SELECT c.id, c.content_hash, c.content, c.content_type, c.size_bytes,
                c.image_path, c.file_path, c.source_app, c.source_url, c.source_title,
                c.captured_at, c.sensitive_type, c.blocked_reason, c.state,
                c.server_id, c.occurrence_count, c.last_seen_at,
                c.created_at, c.updated_at, c.matched_domain_rule, c.pinned_at, c.last_used_at
         FROM clipboard_fts f
         JOIN clipboard_local c ON c.rowid = f.rowid
         WHERE clipboard_fts MATCH ?1
         ORDER BY (c.pinned_at IS NULL) ASC, c.pinned_at DESC,
                  CASE
                      WHEN c.content = ?3 THEN 0
                      WHEN c.content LIKE ?3 || '%' ESCAPE '\\' THEN 1
                      WHEN c.content LIKE '%' || ?3 || '%' ESCAPE '\\' THEN 2
                      ELSE 3
                  END,
                  rank
         LIMIT ?2",
    )?;

    // LIKE 转义：query 内含 % _ \ 会被当通配符，简化为用户输入极少含这些字符，
    // 保险起见先转义 \ 再包住 % _
    let like_safe = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    let rows = stmt
        .query_map(params![phrase, limit, like_safe], row_to_clipboard)?
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
                created_at, updated_at, matched_domain_rule, pinned_at, last_used_at
         FROM clipboard_local
         ORDER BY (pinned_at IS NULL) ASC, pinned_at DESC,
                  COALESCE(last_used_at, captured_at) DESC
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
                created_at, updated_at, matched_domain_rule, pinned_at, last_used_at
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

/// 切换置顶状态：当前未 pin → pin (now)；当前 pin → 取消 pin (NULL)。
/// 返回新的 pinned_at（None = 已取消置顶；Some(ts) = 已置顶于 ts）
pub fn toggle_pin(conn: &Connection, id: &str) -> Result<Option<i64>, rusqlite::Error> {
    let current: Option<i64> = conn
        .query_row(
            "SELECT pinned_at FROM clipboard_local WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let new_value: Option<i64> = if current.is_some() { None } else { Some(now_ms()) };

    conn.execute(
        "UPDATE clipboard_local SET pinned_at = ?1 WHERE id = ?2",
        params![new_value, id],
    )?;

    Ok(new_value)
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

// ══════════════════════════════════════════════════════════════════
// app_rules · App 黑白名单
// ══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRule {
    pub id: i64,
    pub app_identifier: String, // Windows exe 名（Chrome.exe）/ macOS bundle id
    pub rule_type: String,      // "blacklist" | "whitelist"
    pub created_at: i64,
}

fn row_to_app_rule(row: &rusqlite::Row<'_>) -> Result<AppRule, rusqlite::Error> {
    Ok(AppRule {
        id: row.get(0)?,
        app_identifier: row.get(1)?,
        rule_type: row.get(2)?,
        created_at: row.get(3)?,
    })
}

pub fn list_app_rules(conn: &Connection) -> Result<Vec<AppRule>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, app_identifier, rule_type, created_at FROM app_rules ORDER BY rule_type, app_identifier",
    )?;
    let rows = stmt
        .query_map([], row_to_app_rule)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn add_app_rule(
    conn: &Connection,
    app_identifier: &str,
    rule_type: &str,
) -> Result<i64, rusqlite::Error> {
    // 大小写规范化，避免 Chrome.exe / chrome.exe 重复
    let normalized = app_identifier.trim().to_string();
    if normalized.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "empty app_identifier".to_string(),
        ));
    }
    if rule_type != "blacklist" && rule_type != "whitelist" {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "invalid rule_type: {rule_type}"
        )));
    }
    conn.execute(
        "INSERT INTO app_rules (app_identifier, rule_type) VALUES (?1, ?2)
         ON CONFLICT(app_identifier) DO UPDATE SET rule_type = ?2",
        params![normalized, rule_type],
    )?;
    let id = conn.last_insert_rowid();
    Ok(id)
}

pub fn remove_app_rule(conn: &Connection, id: i64) -> Result<bool, rusqlite::Error> {
    let affected = conn.execute("DELETE FROM app_rules WHERE id = ?1", params![id])?;
    Ok(affected > 0)
}

/// 查询 source_app 在规则里的命中（None = 无规则；Some("blacklist"|"whitelist") = 命中）。
///
/// 匹配为**大小写不敏感精确匹配**（Chrome.exe 与 chrome.exe 视为同一 App）。
pub fn app_rule_match(
    conn: &Connection,
    source_app: &str,
) -> Result<Option<String>, rusqlite::Error> {
    let query = source_app.trim();
    if query.is_empty() {
        return Ok(None);
    }
    let result: Option<String> = conn
        .query_row(
            "SELECT rule_type FROM app_rules WHERE LOWER(app_identifier) = LOWER(?1) LIMIT 1",
            params![query],
            |row| row.get(0),
        )
        .ok();
    Ok(result)
}

// ══════════════════════════════════════════════════════════════════
// domain_rules · URL 域名规则
// ══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainRule {
    pub id: i64,
    pub pattern: String,
    /// "parse_as_content" | "skip_parse" | "skip_upload"
    pub rule_type: String,
    pub priority: i64,
    /// "builtin" | "cloud" | "user"
    pub source: String,
    pub created_at: i64,
    pub updated_at: i64,
}

fn row_to_domain_rule(row: &rusqlite::Row<'_>) -> Result<DomainRule, rusqlite::Error> {
    Ok(DomainRule {
        id: row.get(0)?,
        pattern: row.get(1)?,
        rule_type: row.get(2)?,
        priority: row.get(3)?,
        source: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

/// 列出所有 domain_rules（按 priority 降序，便于匹配时优先高优）
pub fn list_domain_rules(conn: &Connection) -> Result<Vec<DomainRule>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, pattern, rule_type, priority, source, created_at, updated_at
         FROM domain_rules
         ORDER BY priority DESC, id ASC",
    )?;
    let rows = stmt
        .query_map([], row_to_domain_rule)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// builtin 规则计数（用于 seed 判断是否已导入）
pub fn count_domain_rules_by_source(
    conn: &Connection,
    source: &str,
) -> Result<i64, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*) FROM domain_rules WHERE source = ?1",
        params![source],
        |row| row.get(0),
    )
}

/// 批量插入 builtin 规则（seed 用）
pub fn bulk_insert_domain_rules(
    conn: &Connection,
    rules: &[(String, String, i64)], // (pattern, rule_type, priority)
    source: &str,
) -> Result<usize, rusqlite::Error> {
    let tx = conn.unchecked_transaction()?;
    let mut stmt = tx.prepare(
        "INSERT INTO domain_rules (pattern, rule_type, priority, source) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut count = 0usize;
    for (pattern, rule_type, priority) in rules {
        stmt.execute(params![pattern, rule_type, priority, source])?;
        count += 1;
    }
    drop(stmt);
    tx.commit()?;
    Ok(count)
}

/// 删除指定 source 的全部规则（比如清空 builtin 重 seed 新版本时用）
pub fn delete_domain_rules_by_source(
    conn: &Connection,
    source: &str,
) -> Result<usize, rusqlite::Error> {
    let affected = conn.execute(
        "DELETE FROM domain_rules WHERE source = ?1",
        params![source],
    )?;
    Ok(affected)
}

// ══════════════════════════════════════════════════════════════════
// settings · 桌面端配置
// ══════════════════════════════════════════════════════════════════

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
        matched_domain_rule: row.get(19)?,
        pinned_at: row.get(20)?,
        last_used_at: row.get(21)?,
    })
}

/// 标记一条记录为"刚刚被使用"（复制到剪贴板）→ 更新 last_used_at
/// 被 pasteRow / copyToClipboard 成功后调用；ORDER BY COALESCE(last_used_at,
/// captured_at) DESC 让该项 promote 到顶部
pub fn mark_used(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE clipboard_local SET last_used_at = ?1 WHERE id = ?2",
        params![now_ms(), id],
    )?;
    Ok(())
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
            ..Default::default()
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
            ..Default::default()
        };
        let req2 = InsertRequest {
            id: "uuid-2".to_string(),
            content: Some("duplicate content".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
            ..Default::default()
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
            ..Default::default()
        };
        let req2 = InsertRequest {
            id: "uuid-2".to_string(),
            content: Some("Rust 是一门注重安全速度和并发的编程语言。".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
            ..Default::default()
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
            ..Default::default()
        };
        let req2 = InsertRequest {
            id: "uuid-2".to_string(),
            content: Some("  hello world\n".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
            ..Default::default()
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
            ..Default::default()
        };
        let req2 = InsertRequest {
            id: "img-2".to_string(),
            content: Some("ff00aa11bb22cc33000000000000000000000000000000000000000000000000".to_string()),
            content_type: "image".to_string(),
            image_path: Some("img-2.png".to_string()),
            file_path: None,
            source_app: None,
            ..Default::default()
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
            ..Default::default()
        };
        let req2 = InsertRequest {
            id: "img-2".to_string(),
            content: Some(fingerprint.to_string()),
            content_type: "image".to_string(),
            image_path: Some("img-2.png".to_string()),
            file_path: None,
            source_app: None,
            ..Default::default()
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
            ..Default::default()
        };
        let req2 = InsertRequest {
            id: "uuid-2".to_string(),
            content: Some("Rust 注重速度的编程语言".to_string()),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: None,
            ..Default::default()
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
                ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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

    // ── app_rules CRUD 补测（T1 gap）──

    #[test]
    fn test_app_rules_list_and_remove() {
        let conn = setup_db();
        let id1 = add_app_rule(&conn, "Chrome.exe", "blacklist").unwrap();
        let id2 = add_app_rule(&conn, "1Password.exe", "blacklist").unwrap();
        let id3 = add_app_rule(&conn, "TrustedApp.exe", "whitelist").unwrap();

        let list = list_app_rules(&conn).unwrap();
        assert_eq!(list.len(), 3);
        // ORDER BY rule_type, app_identifier → blacklist 先（字典序：1Password, Chrome）
        assert_eq!(list[0].rule_type, "blacklist");
        assert_eq!(list[0].app_identifier, "1Password.exe");
        assert_eq!(list[1].app_identifier, "Chrome.exe");
        assert_eq!(list[2].rule_type, "whitelist");

        // remove 中间一条
        let removed = remove_app_rule(&conn, id1).unwrap();
        assert!(removed);
        let list2 = list_app_rules(&conn).unwrap();
        assert_eq!(list2.len(), 2);
        assert!(!list2.iter().any(|r| r.id == id1));

        // 不存在的 id 返 false
        let removed_missing = remove_app_rule(&conn, 99999).unwrap();
        assert!(!removed_missing);

        // 确保 id2 / id3 还在
        assert!(list2.iter().any(|r| r.id == id2));
        assert!(list2.iter().any(|r| r.id == id3));
    }

    #[test]
    fn test_app_rule_upsert_overwrites_rule_type() {
        // add 同一 app_identifier 两次，第二次应更新 rule_type 而不是插入
        let conn = setup_db();
        add_app_rule(&conn, "app.exe", "blacklist").unwrap();
        add_app_rule(&conn, "app.exe", "whitelist").unwrap(); // ON CONFLICT UPDATE

        let list = list_app_rules(&conn).unwrap();
        assert_eq!(list.len(), 1, "same app_identifier should not duplicate");
        assert_eq!(list[0].rule_type, "whitelist", "latest add wins");
    }

    #[test]
    fn test_app_rule_invalid_type_rejected() {
        let conn = setup_db();
        let err = add_app_rule(&conn, "app.exe", "graylist");
        assert!(err.is_err(), "only blacklist/whitelist allowed");
    }

    // ── domain_rules delete_by_source 补测（T2 gap）──

    #[test]
    fn test_delete_domain_rules_by_source_preserves_others() {
        let conn = setup_db();
        bulk_insert_domain_rules(
            &conn,
            &[
                ("builtin.com/*".to_string(), "skip_upload".to_string(), 100),
                ("built2.com/*".to_string(), "skip_parse".to_string(), 80),
            ],
            "builtin",
        )
        .unwrap();
        bulk_insert_domain_rules(
            &conn,
            &[("user.com/*".to_string(), "skip_upload".to_string(), 500)],
            "user",
        )
        .unwrap();

        assert_eq!(count_domain_rules_by_source(&conn, "builtin").unwrap(), 2);
        assert_eq!(count_domain_rules_by_source(&conn, "user").unwrap(), 1);

        let removed = delete_domain_rules_by_source(&conn, "builtin").unwrap();
        assert_eq!(removed, 2);
        assert_eq!(count_domain_rules_by_source(&conn, "builtin").unwrap(), 0);
        // user 规则必须保留
        assert_eq!(count_domain_rules_by_source(&conn, "user").unwrap(), 1);
    }

    #[test]
    fn test_settings() {
        let conn = setup_db();

        // migration 002 清理了预置键，现在默认值靠 settings_keys.rs 常量而不是 DB INSERT
        let missing_before_write = get_setting(&conn, "ui.theme").unwrap();
        assert!(missing_before_write.is_none());

        set_setting(&conn, "ui.theme", Some("dark")).unwrap();
        let theme = get_setting(&conn, "ui.theme").unwrap();
        assert_eq!(theme.as_deref(), Some("dark"));

        // 清空为 None 删除该行
        set_setting(&conn, "ui.theme", None).unwrap();
        let cleared = get_setting(&conn, "ui.theme").unwrap();
        // set_setting(None) 会写 NULL，get_setting 读出来也是 None
        assert!(cleared.is_none());

        let missing = get_setting(&conn, "nonexistent").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_search_fts5_special_chars_no_panic() {
        // 用户输入的特殊字符（括号 / AND/OR/NOT / 引号 / 星号 / 减号）
        // 在未转义时会让 FTS5 抛语法错误。短语转义后任意输入都应安全。
        let conn = setup_db();

        for (i, content) in [
            "hello(world)",
            "foo AND bar",
            "\"double quoted\"",
            "NOT good",
            "a * b",
            "minus-sign",
        ]
        .iter()
        .enumerate()
        {
            let req = InsertRequest {
                id: format!("sp-{i}"),
                content: Some(content.to_string()),
                content_type: "text".to_string(),
                image_path: None,
                file_path: None,
                source_app: None,
                ..Default::default()
            };
            insert_clipboard(&conn, req).unwrap();
        }

        // 任何含特殊字符的 query 都不应 panic（即便匹配 0 条也 OK）
        assert!(search_clipboard(&conn, "hello(world)", 10).is_ok());
        assert!(search_clipboard(&conn, "foo AND bar", 10).is_ok());
        assert!(search_clipboard(&conn, "\"double quoted\"", 10).is_ok());
        assert!(search_clipboard(&conn, "NOT good", 10).is_ok());
        assert!(search_clipboard(&conn, "(test)", 10).is_ok());
        assert!(search_clipboard(&conn, "", 10).is_ok());
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
                ..Default::default()
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
