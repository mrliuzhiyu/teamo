-- 004_pin_support.sql · clipboard_local 加 pinned_at 列（支持置顶到面板顶部）
--
-- 背景：竞品对比发现 Maccy / CopyQ / Ditto 都有 pin/收藏机制——高频条目（常用邮箱/
-- 地址/代码片段）置顶后不用搜索即可直达。Teamo 这个基线缺失导致键盘党用户
-- 每次都要搜才能粘到高频条目。
--
-- 设计选择：用 INTEGER 时间戳而非 BOOLEAN：
--   - NULL = 未置顶
--   - 数字 = 置顶时间戳（Unix ms）
--   多个 pin 项之间按 pinned_at DESC 排序（最新 pin 的在最上），保留"折叠收藏时间"信息。
--
-- 排序规则变化：list_recent / search_clipboard 都要在现有 ORDER BY 前加
--   (pinned_at IS NULL) ASC, pinned_at DESC
-- 这样 pin 项聚集在顶部，未 pin 按原规则（captured_at DESC / FTS rank）。

ALTER TABLE clipboard_local ADD COLUMN pinned_at INTEGER;

CREATE INDEX idx_local_pinned ON clipboard_local(pinned_at DESC) WHERE pinned_at IS NOT NULL;

INSERT INTO schema_migrations (version) VALUES (4);
