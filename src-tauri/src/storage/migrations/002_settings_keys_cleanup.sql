-- 002_settings_keys_cleanup.sql · 统一 settings key 命名空间
--
-- 背景：migration 001 的 INSERT 预置了 `autostart` / `hotkey_panel` / `theme` /
-- `min_text_length` / `retention_days` / `logged_in_user_id` / `cloud_sync_enabled`
-- 等简单命名键。前端代码却用 `ui.theme` / `filter.min_text_len` / `sens.*` 等点分
-- 命名键。两套并行谁都读不到谁，业务层实质上从未消费过 migration 001 的默认值。
--
-- 本 migration 做两件事：
--   1. **值迁移（仅非默认值）**：如果用户自定义改过某个老 key，把值复制到对应新点分键。
--      "非默认值" = 与 migration 001 INSERT 的默认字面量不同（比如 theme != 'system'）。
--      这样 fresh 安装走完 migration 表是空的（默认值由 settings_keys 常量提供），
--      user 改过的值才进新表。Outside voice review 的"升级不丢用户设置"要求 + 架构
--      "单一 source of truth" 要求两者兼得。
--   2. DELETE 全部老键
--
-- retention_days（天数）→ data.retention 枚举值的映射：
--   30 → '1m'
--   180 → '6m'
--   365 → '1y'
--   其他非默认值 → 'forever'（保守：保留全部）

-- 1. 值迁移（仅非默认值）

-- theme: 默认 'system'；用户改成 'light'/'dark' 才迁
INSERT INTO settings (key, value, updated_at)
SELECT 'ui.theme', value, strftime('%s','now')*1000
FROM settings
WHERE key = 'theme' AND value IS NOT NULL AND value != 'system'
ON CONFLICT(key) DO NOTHING;

-- min_text_length: 默认 '8'；其他值才迁
INSERT INTO settings (key, value, updated_at)
SELECT 'filter.min_text_len', value, strftime('%s','now')*1000
FROM settings
WHERE key = 'min_text_length' AND value IS NOT NULL AND value != '8'
ON CONFLICT(key) DO NOTHING;

-- retention_days: 默认 '0'；非默认值做枚举映射
INSERT INTO settings (key, value, updated_at)
SELECT
    'data.retention',
    CASE
        WHEN CAST(value AS INTEGER) = 30 THEN '1m'
        WHEN CAST(value AS INTEGER) = 180 THEN '6m'
        WHEN CAST(value AS INTEGER) = 365 THEN '1y'
        ELSE 'forever'
    END,
    strftime('%s','now')*1000
FROM settings
WHERE key = 'retention_days' AND value IS NOT NULL AND value != '0'
ON CONFLICT(key) DO NOTHING;

-- cloud_sync_enabled: 默认 '0'；改成 '1' 才迁（代表用户开过云端同步）
INSERT INTO settings (key, value, updated_at)
SELECT 'cloud.sync_enabled', value, strftime('%s','now')*1000
FROM settings
WHERE key = 'cloud_sync_enabled' AND value IS NOT NULL AND value != '0'
ON CONFLICT(key) DO NOTHING;

-- logged_in_user_id: migration 001 默认 NULL（WHERE value IS NOT NULL 已过滤掉默认）。
-- 有值即意味着用户登录过 —— 迁。
INSERT INTO settings (key, value, updated_at)
SELECT 'cloud.logged_in_user_id', value, strftime('%s','now')*1000
FROM settings
WHERE key = 'logged_in_user_id' AND value IS NOT NULL
ON CONFLICT(key) DO NOTHING;

-- autostart / hotkey_panel 不迁移：
--   - autostart 走 tauri-plugin-autostart 管理 OS 层注册表，不靠 settings 表
--   - hotkey_panel 当前写死（Phase 3 做快捷键可配置时再考虑迁移）

-- 2. DELETE 老键

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
