-- 006_image_dims.sql · clipboard_local 加 image_width / image_height 列
--
-- 背景：CardItem 列表显示图片尺寸（"1920×1080 · 来自 Chrome"）原本走"前端额外
-- invoke 原图 base64 → new Image() 读 naturalWidth"路径，20 条图片列表同时发
-- 40 个并发 invoke + ~200MB IPC 浪费。真相是 capture 时 arboard 已经返回
-- image.width / image.height，不存下来纯浪费。
--
-- 两列都 NULL-able：非图片记录不填；老数据（migration 前的图片）也保持 NULL
-- 前端对 NULL 回退显示 "尺寸未知" 或跳过尺寸展示

ALTER TABLE clipboard_local ADD COLUMN image_width INTEGER;
ALTER TABLE clipboard_local ADD COLUMN image_height INTEGER;

INSERT INTO schema_migrations (version) VALUES (6);
