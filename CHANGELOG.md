# Changelog

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 规范，版本号遵循 [SemVer](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### 进行中（M1 收尾）
- v0.1 发布前检查：实机验证、Windows/macOS 安装包构建、签名策略确认
- 宣传渠道稿件：V2EX、即刻、HN、Twitter、朋友圈

### Phase 2-3 规划
- macOS NSPanel（无 Dock + 不抢焦点）+ macOS 系统粘贴（CGEvent）+ NSWorkspace source_app 抓取
- Tray 状态色动态切换（4 色图标）+ 动态 tooltip 统计
- 主题切换（深色模式 Tailwind dark class + CSS 变量）
- 快捷键可配置（Settings 改 hotkey_panel 键）
- 导出进度条 + 取消按钮（tokio 后台任务 + event）
- 检查更新 endpoint 配置
- Tray "Capture: Dead" UI 显示（后端心跳已就位）

## [0.1.0-alpha] - 2026-MM-DD（待发布）

### 新增

#### 剪切板捕获与存储
- 后台剪切板监听（arboard 500ms 轮询）
- 文本 / 图片 / 文件三种内容类型，图片真 PNG 编码持久化宽高
- 归一化精确去重（末尾标点 / 空白 / 零宽字符归一），30 秒窗口内同内容只 bump `occurrence_count` 不新建行
- 图片按像素 SHA256 指纹去重（跨进程缓存）
- SQLite + FTS5 全文索引，中文/英文搜索毫秒级
- FTS5 查询短语转义，用户输入任意特殊字符（括号/AND/NOT/引号）不抛语法错
- 暂停记录（5 分钟 / 1 小时 / 手动恢复），重启恢复暂停状态

#### 快速面板（Cmd/Ctrl+Shift+V）
- 全局快捷键唤起浮窗（多入口 Vite + 独立 panel Window + alwaysOnTop + skipTaskbar）
- 搜索框 300ms debounce + 高亮匹配 + 清除按钮
- 最近 20 条卡片列表（相对时间 / 来源 App / 状态徽标 / 敏感打码）
- 键盘全覆盖：↑↓ 选择 / Enter 复制并关闭 / Delete 忘记 / Esc 关闭
- 忘记 + 5 秒撤销浮条（失焦时自动 flush 防跨会话泄漏）
- 卡片选中时右侧浮出 [复制] [忘记] 按钮
- 底部操作栏：暂停下拉 / 云端连接 / 打开设置
- 顶部今日统计：已记 / 拦截 / 上云（SQLite localtime 归一）
- **Windows 原生系统粘贴**：`Enter` 触发 writeText → hide panel → SetForegroundWindow 记前景 App + enigo 模拟 Ctrl+V，用户感知"按下即粘贴"
- 图片卡片粘贴：PNG decode → `arboard::set_image` → 系统 Ctrl+V

#### Tray 图标与菜单
- 系统托盘常驻图标（Windows 任务栏 / macOS 菜单栏）
- 菜单项：快速搜索 / 暂停子菜单（5 分钟/1 小时/手动） / 继续记录 / 设置 / 退出
- 关闭主窗口不退出应用（Slack 风格）
- `IS_QUITTING` 原子信号量区分用户点 X 与 tray 退出路径

#### 端侧敏感检测（filter-engine）
- 6 类检测：密码 / Token / 银行卡 / 身份证 / 手机号 / 邮箱
- Token 识别：OpenAI `sk-*` / Stripe `pk-*` / GitHub `gh[pousrb]_*` / Slack `xox[abpr]-*` / HTTP `Bearer *` / JWT 三段式
- 银行卡：13-19 位候选 + Luhn 算法（Visa/MC/Amex/UnionPay）
- 中国身份证：GB 11643 加权校验码（支持末位 X/x）
- 手机：`1[3-9]\d{9}` 中国大陆
- 邮箱：RFC 5322 简化正则
- 密码启发式：无空格 + 不含 `://` + 8-64 字符 + 至少 3 种字符类型（小写/大写/数字/符号）
- 闸门集成 capture loop，命中内容 `state=local_only` + `blocked_reason` + `sensitive_type` 写入 DB
- 40+ 单测覆盖正则 / Luhn / 身份证 / 熵各工具层

#### 数据导出（data-export）
- JSON 格式：完整 schema + 图片独立 `images/` 目录 + `metadata.json`（含 `schema_version: v1`）
- Markdown 格式：按时间倒序 + frontmatter + 敏感打码 `••••••• [拦截：password]`
- 图片字节级拷贝（不 decode/encode 避免色彩转换误差）
- 图片丢失标记：JSON `image_missing: true`，不阻塞导出
- 时间格式化零依赖走 SQLite `strftime`，不引 chrono/time crate
- 6 个单测（含 roundtrip + 字节级图片比对）

#### 设置页（settings-page）
- 5 区纵向滚动：通用 / 隐私 / 云端 / 数据 / 关于
- **通用**：开机自启动开关（`plugin-autostart`）+ 分平台快捷键展示（⌘⇧V vs Ctrl+Shift+V）
- **隐私**：6 个敏感类型开关持久化（真生效）；App 黑白名单 CRUD（输入框 + 抓当前 App + 黑/白分组列表 + × 删除）
- **云端**：未登录引导卡片 + 「连接 TextView 云端（即将支持）」+ 了解链接
- **数据**：路径显示 + 打开文件管理器 + DB/图片字节统计 + JSON/MD 导出 + 保留时长下拉 + 清空（二次确认）
- **关于**：版本号 + AGPL-3.0 / GitHub / Issues 外链

#### App 黑白名单生效（filter-engine Phase 2A）
- Windows source_app 抓取：`GetForegroundWindow` → `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` → `GetModuleFileNameExW`，动态 buffer doubling 到 32768 支持 long-path
- Teamo 自身进程过滤（pid == GetCurrentProcessId）
- **`<elevated>` 哨兵**：OpenProcess 被拒（管理员进程 / 受保护进程）返哨兵而非 None，filter 对哨兵采保守策略（用户已配 blacklist 则视同黑名单，否则透明）。修复"KeePass 以管理员运行时黑名单 bypass"的 silent-failure
- filter::apply_filters L1.2 White/Blacklist → captured / blocked_app
- 文本 + 图片分支走共用 `filter::check_app_rules` 函数保持对称

#### URL 域名规则生效（filter-engine Phase 2B）
- 依赖 `url = "2.5"` + `serde_yaml = "0.9"`
- `filter/url_match.rs`：URL 提取 + 5 种 wildcard pattern 匹配（`domain.com` / `domain.com/path/*` / `*.domain.com/*` / `*/login` / `localhost*`），regex 编译缓存
- `storage/seed_rules.rs`：`include_str!` 嵌入 domain_rules.yaml，YAML 编译期严格验证（rule_type 白名单 + priority 范围 + 空 pattern 三重 test）
- **版本化升级**：`filter.builtin_rules_version` settings key + YAML 顶部 `version: N`，版本 bump 时清空 builtin 重 seed（保留 user/cloud 规则）
- `filter::apply_filters` L1.4 分支：skip_upload → local_only；`parse_as_content` / `skip_parse` 写入 `clipboard_local.matched_domain_rule`（migration 003）供 M3 云端 parse_worker 消费
- 白名单优先级 > 域名 skip_upload（用户信任 App 可覆盖 URL 规则）

#### 保留时长真清理
- `storage/retention.rs`：forever / 1y / 6m / 1m 枚举映射毫秒 → DELETE clipboard_local WHERE captured_at < cutoff（FTS5 触发器自动同步）→ 清对应 PNG 文件
- 启动时跑一次（lib.rs setup 第 2c 步）；Phase 2 加 tokio interval 每 6h 定时清
- Data.tsx UI 下拉接真实 settings 读写

#### 架构加固（eng review 14 修 + 4 test gap）
- **settings key 单源**：`src-tauri/src/settings_keys.rs` + `src/lib/settings-keys.ts` 跨语言常量对齐；migration 002 值迁移（仅非默认值迁移新 key，fresh migration 表为空）
- **filter 读 settings 真生效**：`apply_filters(conn, content, source_app)` 扩参；`filter::cache::FilterSnapshot` RwLock 单例缓存 sens.* 开关 + min_text_len + domain_rules，capture loop 0 次 DB 查询（原 9 次 / capture）；set_setting / seed 改动后 invalidate
- **UX 共用组件**：`src/lib/platform.ts`（isMac / shortcutLabel / enterHintLabel）+ `src/lib/CloudCtaButton.tsx`
- **Capture loop panic 自愈**：外层 supervisor `catch_unwind` + 1s 重启；`CaptureState::last_heartbeat_ms` AtomicI64 + `get_capture_health` command（Tray UI 显示 Phase 2）
- **图片 dedup 全量 sha256**：消灭原首 4KB DefaultHasher 误判导致的"截图 silent 丢失"
- **storage 层 thiserror StorageError**：Sqlite / Yaml / Io / Message 四 variants，seed_rules + retention 切新类型
- **首次启动引导**：`APP_FIRST_RUN_COMPLETED` settings 标记；tauri.conf.json main window visible=false，首次 show 引导后续静默到 tray
- **clear_all_data event 通知**：emit `data:cleared` → panel 监听秒刷新

### 代码质量与测试

- **132 个 lib 单测**（filter 50+ / repository 20+ / settings_keys 7 / canonicalize 6 / schema 4 / seed_rules 5 / retention 4 / export 6 / platform::basename 6 / filter::cache 2）
- **3 轮代码 review 修 31 bug + 补 4 test gap**：
  - quick-panel 2 轮共 14（严重：Teamo 自身窗口粘回、UndoToast race、FTS5 特殊字符、focus 覆盖搜索）
  - tray 2（严重：`app.exit` 被 CloseRequested 拦；tray 搜索忘抓前景）
  - export 1（test 死锁：`db.conn()` MutexGuard 跨 export_data）
  - **post-hoc eng review 14**（outside voice 捕到 5 silent-failure：elevated bypass / 图片 4KB 误判 / capture panic 静默死亡 / YAML typo 无 CI 拦 / migration 静默抹用户值；架构 5 + 质量 2：cache / thiserror / buffer doubling / DRY / 版本化 / matched_domain_rule / app 哨兵）

### 决策与限制

- **未签名发布**：v0.1 不购买 EV Cert（Windows ¥3000-5000/年）/ Apple Developer ID（$99/年）；用户首次运行需手动点「仍要运行」
- **暂不支持 Linux**：Tauri 2.x Linux 构建可行但未测试
- **macOS Phase 1 未做 NSPanel + 系统粘贴 + source_app 抓取**：Phase 4 一起做（CGEvent 需辅助功能权限引导 + NSWorkspace API）；Phase 1 macOS 能用但 Enter 不模拟系统粘贴 + App 黑白名单规则不生效
- **macOS 图片 decode**：依赖 arboard 的 Windows CF_DIBV5 premultiplied alpha 处理；macOS 待测
- **不支持导入**：JSON 含 `schema_version` 为未来导入预留，当前版本仅导出不导入
- **source_url 抓取缺失**：只对 content 本身是 URL 的场景走 domain_rules；"银行页面复制正常文本自动拦"需要浏览器扩展 / macOS AppleScript / Windows UIA（Phase 2+）

---

## [0.0.1] - 2026-04-15

### 项目启动
- 仓库初始化
- 产品定位与架构文档
- 决策：AGPL-3.0 协议、Tauri 2.x 自建、不登录可用、TextView 云端为可选增值
- 初始域名规则库 70+ 条（[domain_rules.yaml](domain_rules.yaml)）
