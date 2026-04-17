-- 002_settings_keys_cleanup.sql · 统一 settings key 命名空间
--
-- 背景：migration 001 的 INSERT 预置了 `autostart` / `hotkey_panel` / `theme` /
-- `min_text_length` / `retention_days` / `logged_in_user_id` / `cloud_sync_enabled`
-- 等简单命名键。前端代码却用 `ui.theme` / `filter.min_text_len` / `sens.*` 等点分
-- 命名键。两套并行谁都读不到谁，业务层实质上从未消费过 migration 001 的默认值
-- （autostart 走 plugin-autostart、theme 用前端新键、其余三个完全没接入）。
--
-- 本 migration 清除老键，业务层转由 `settings_keys.rs` 常量 + `unwrap_or(DEFAULT)` 模式
-- 拿默认值，设置表只存用户**改过**的值。架构更干净：
--   1. 单一 source of truth（settings_keys.rs + 前端 settings-keys.ts 镜像）
--   2. 迁移新版本时不会出现 migration INSERT 的默认值和业务代码 DEFAULT 常量漂移
--   3. 新增设置项只改常量模块，不需要改 migration

DELETE FROM settings
WHERE key IN (
    'autostart',
    'hotkey_panel',
    'theme',
    'min_text_length',
    'retention_days',
    'logged_in_user_id',
    'cloud_sync_enabled'
);

INSERT INTO schema_migrations (version) VALUES (2);
