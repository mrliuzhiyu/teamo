-- 007_session_grouping.sql · L1 规则分组（session_id + parent_id）
--
-- 背景：Teamo 定位是"记录工具"不是"剪贴板工具"。用户在一个网页 / 文章里反复
-- 复制（Ctrl+A 全文 + 多段 Ctrl+C）产生 N 条 raw 记录，剪贴板 tab 全量保留，
-- 但聚合 tab 需要把"同一次阅读会话的碎片"归成一组展示。
--
-- 架构：
-- - L0：capture loop 全量写入 clipboard_local（不压缩，保真度绝对优先）
-- - L1（本 migration 支持）：capture 时按规则分配 session_id；idle debounce 后
--   补算 parent_id（字符串子集 B⊂A → B.parent_id = A.id）
-- - L2：未来 embedding + topic cluster（不在此 migration）
-- - L3：云端 memo 整理（不在此 migration）
--
-- Session 分配规则（同步，capture 前算，无 AI）：
-- - 查最近一条 WHERE source_app = :current_app ORDER BY captured_at DESC LIMIT 1
-- - 如果存在且 (now - captured_at) < 5 分钟 → 复用其 session_id
-- - 否则生成新 UUID
-- - 无 source_app（elevated 哨兵 / 非 Windows）→ 独立 session
--
-- Parent 分配规则（异步，idle 5s debounce 后批处理）：
-- - 同 session 内两两比较，短内容是长内容的子串 → 短 row.parent_id = 长 row.id
-- - R1 暂不实现（观察 session 聚合效果后再决定是否加），留 schema 预留

ALTER TABLE clipboard_local ADD COLUMN session_id TEXT;
ALTER TABLE clipboard_local ADD COLUMN parent_id TEXT;

CREATE INDEX idx_local_session_captured ON clipboard_local(session_id, captured_at DESC);

INSERT INTO schema_migrations (version) VALUES (7);
