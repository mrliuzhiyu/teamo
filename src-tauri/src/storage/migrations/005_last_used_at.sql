-- 005_last_used_at.sql · clipboard_local 加 last_used_at 列（支持粘贴后 promote）
--
-- 背景：Ditto 的"粘贴后置首位"是高频流"复制 A → 粘 B → 想再粘 A"的关键贴心。
-- Teamo 当前 ORDER BY captured_at DESC，一旦有新复制 push，老条目即使刚用过也
-- 沉到列表下面，用户要重搜。
--
-- 设计：新加 last_used_at INTEGER NULL，代表"上次被使用（复制到剪贴板）的时间"。
--   - NULL = 从未使用 → 排序退化到 captured_at
--   - 值 = 上次粘贴/复制时间戳（Unix ms）
--
-- 排序规则：
--   (pinned_at IS NULL) ASC,           -- pin 项聚顶
--   pinned_at DESC,                    -- 多个 pin 项按 pin 时间
--   COALESCE(last_used_at, captured_at) DESC   -- 用过的用时间；没用过的按捕获时间
--
-- 谁触发更新：前端 copyToClipboard 成功后 invoke('mark_used', {id})。
-- 包含的用户动作：Enter / 双击 / 右键「粘贴」/「仅复制」—— 任何把该条写到系统
-- 剪贴板的行为都算一次"使用"。

ALTER TABLE clipboard_local ADD COLUMN last_used_at INTEGER;

CREATE INDEX idx_local_last_used ON clipboard_local(last_used_at DESC) WHERE last_used_at IS NOT NULL;

INSERT INTO schema_migrations (version) VALUES (5);
