-- 001_initial.sql · Teamo 本地 SQLite 初始 schema
-- 参照 TEAMO_ARCHITECTURE.md 附录 A

-- ============================================================
-- clipboard_local · 本地剪切板存储
-- ============================================================
CREATE TABLE clipboard_local (
    id              TEXT PRIMARY KEY,         -- 端侧 UUIDv7
    content_hash    TEXT NOT NULL,            -- SHA256(canonicalize(content))，归一化后的精确去重指纹
    content         TEXT,                     -- 全部明文
    content_type    TEXT NOT NULL,            -- text / image / file / html / rtf
    size_bytes      INTEGER,
    image_path      TEXT,                     -- 图片本地路径
    file_path       TEXT,                     -- 文件路径
    source_app      TEXT,
    source_url      TEXT,
    source_title    TEXT,
    captured_at     INTEGER NOT NULL,         -- Unix ms
    sensitive_type  TEXT,                     -- null / password / token / credit_card / id_card / phone / email
    blocked_reason  TEXT,                     -- null / app_blacklist / domain_blacklist / sensitive / short / dedup
    state           TEXT NOT NULL DEFAULT 'captured',
    server_id       TEXT,                     -- 上云后服务端回填的 memo_id
    occurrence_count INTEGER DEFAULT 1,
    last_seen_at    INTEGER,
    upload_attempts INTEGER DEFAULT 0,
    upload_error    TEXT,
    created_at      INTEGER DEFAULT (strftime('%s','now')*1000),
    updated_at      INTEGER DEFAULT (strftime('%s','now')*1000)
);

CREATE INDEX idx_local_state ON clipboard_local(state, captured_at);
CREATE INDEX idx_local_captured ON clipboard_local(captured_at DESC);
CREATE INDEX idx_local_hash ON clipboard_local(content_hash);
CREATE INDEX idx_local_app ON clipboard_local(source_app, captured_at DESC);

-- ============================================================
-- 全文搜索（FTS5 影子表）
-- ============================================================
CREATE VIRTUAL TABLE clipboard_fts USING fts5(
    content,
    source_title,
    source_url,
    source_app,
    content='clipboard_local',
    content_rowid='rowid',
    tokenize='unicode61'
);

-- 同步触发器
CREATE TRIGGER clipboard_local_ai AFTER INSERT ON clipboard_local BEGIN
  INSERT INTO clipboard_fts(rowid, content, source_title, source_url, source_app)
  VALUES (new.rowid, new.content, new.source_title, new.source_url, new.source_app);
END;

CREATE TRIGGER clipboard_local_ad AFTER DELETE ON clipboard_local BEGIN
  INSERT INTO clipboard_fts(clipboard_fts, rowid, content, source_title, source_url, source_app)
  VALUES ('delete', old.rowid, old.content, old.source_title, old.source_url, old.source_app);
END;

CREATE TRIGGER clipboard_local_au AFTER UPDATE ON clipboard_local BEGIN
  INSERT INTO clipboard_fts(clipboard_fts, rowid, content, source_title, source_url, source_app)
  VALUES ('delete', old.rowid, old.content, old.source_title, old.source_url, old.source_app);
  INSERT INTO clipboard_fts(rowid, content, source_title, source_url, source_app)
  VALUES (new.rowid, new.content, new.source_title, new.source_url, new.source_app);
END;

-- ============================================================
-- app_rules · App 黑白名单
-- ============================================================
CREATE TABLE app_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    app_identifier TEXT NOT NULL UNIQUE,    -- bundle_id / exe_name
    rule_type TEXT NOT NULL,                -- 'blacklist' / 'whitelist'
    created_at INTEGER DEFAULT (strftime('%s','now')*1000)
);

-- ============================================================
-- domain_rules · 域名规则
-- ============================================================
CREATE TABLE domain_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern TEXT NOT NULL,                  -- douyin.com/video/*
    rule_type TEXT NOT NULL,                -- 'parse' / 'skip_parse' / 'skip_upload'
    priority INTEGER DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'builtin', -- 'builtin' / 'cloud' / 'user'
    created_at INTEGER DEFAULT (strftime('%s','now')*1000),
    updated_at INTEGER DEFAULT (strftime('%s','now')*1000)
);

CREATE INDEX idx_domain_rules_priority ON domain_rules(rule_type, priority DESC);

-- ============================================================
-- settings · 桌面端配置
-- ============================================================
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT,
    updated_at INTEGER DEFAULT (strftime('%s','now')*1000)
);

-- 初始配置
INSERT INTO settings (key, value) VALUES
    ('autostart', '1'),
    ('hotkey_panel', 'cmd+shift+v'),
    ('theme', 'system'),
    ('min_text_length', '8'),
    ('retention_days', '0'),
    ('logged_in_user_id', NULL),
    ('cloud_sync_enabled', '0');

-- ============================================================
-- schema_migrations · 版本追踪
-- ============================================================
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at INTEGER DEFAULT (strftime('%s','now')*1000)
);

INSERT INTO schema_migrations (version) VALUES (1);
