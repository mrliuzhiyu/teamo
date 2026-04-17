-- 003_matched_domain_rule.sql · clipboard_local 加 matched_domain_rule 列
--
-- 背景：filter::apply_filters 里 URL 命中 domain_rules 的 `parse_as_content` 或
-- `skip_parse` 时，Phase 1 **不写 row** ——仅 continue 循环。结果：M3 上云后，
-- 云端 parse_worker 看到一条 captured 的 row 不知道它是"应该 link_cards 解析"
-- 还是"应该跳过"。云端需要再跑一次 domain_rules 匹配（额外 cloud-side 计算 +
-- 规则库同步复杂度）。
--
-- 本次增列：clipboard_local.matched_domain_rule 格式为 "rule_type:pattern"，比如：
--   - "parse_as_content:v.douyin.com/*"
--   - "skip_parse:baidu.com/s"
--   - NULL（非 URL 或未命中任何 domain_rule）
-- skip_upload 不写这里（走 state='local_only' + blocked_reason='domain_skip_upload:pattern'）。
--
-- M3 云端侧规则：
--   - 上云时把 matched_domain_rule 跟 content 一起传
--   - parse_worker 读到 matched_domain_rule='parse_as_content:*' → 走 link_cards 解析
--   - parse_worker 读到 'skip_parse:*' → 不解析，仅索引

ALTER TABLE clipboard_local ADD COLUMN matched_domain_rule TEXT;

INSERT INTO schema_migrations (version) VALUES (3);
