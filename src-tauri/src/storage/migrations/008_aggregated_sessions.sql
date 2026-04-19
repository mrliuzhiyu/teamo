-- 008_aggregated_sessions.sql · R3.4 session 上云状态持久化
--
-- 之前 session 是 clipboard_local 里的 rows 按 session_id GROUP BY 聚合出来的
-- 动态概念，没有实体表。R3.2/R3.3 的 "✓ 已上云" 标记在 SessionCard 里只是
-- React state，Teamo 重启后丢失 → 用户会重复点 "上云"，后端虽幂等（按
-- local_id 去重）但用户看不到"已上云"标。本 migration 给 session 一个持久
-- 化的元表。
--
-- 字段设计：
--   session_id:      PRIMARY KEY，对应 clipboard_local.session_id
--   server_memo_id:  上云后 TextView 返回的 memo id（未来 M4 Web 端跳转用）
--   uploaded_at:     Unix ms 时间戳；NULL = 未上云（或上云失败）
--   upload_error:    最近一次上云失败原因（若有），toast 提示用
--   created_at / updated_at: 标准元数据
--
-- 未来 R3+ 扩展字段（不破坏现有）：
--   ai_title:        L3 云端 LLM 生成的标题
--   ai_summary:      LLM 生成摘要
--   topic_id:        L2 主题聚类归属
--   embedding:       BLOB 缓存

CREATE TABLE aggregated_sessions (
    session_id       TEXT PRIMARY KEY,
    server_memo_id   TEXT,
    uploaded_at      INTEGER,
    upload_error     TEXT,
    created_at       INTEGER NOT NULL DEFAULT (strftime('%s','now')*1000),
    updated_at       INTEGER NOT NULL DEFAULT (strftime('%s','now')*1000)
);

CREATE INDEX idx_agg_uploaded ON aggregated_sessions(uploaded_at DESC)
    WHERE uploaded_at IS NOT NULL;

INSERT INTO schema_migrations (version) VALUES (8);
