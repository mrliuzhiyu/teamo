# Teamo Roadmap

Teamo 是一个正在快速迭代的开源项目。本文档公开当前开发状态与未来规划，帮助用户和贡献者了解项目方向。

> 内部的详细工程看板在主仓库（非公开），本文档是公开精简版。
> 最后更新：2026-04-17

---

## 当前状态：pre-alpha（M1 收尾中）

- **v0.1 目标**：不登录可用的纯本地剪切板工具，Windows 为主发布平台
- **进度**：7 个核心卷全部启动至 Phase 1 以上，v0.1 正式发布前还要完成 Windows/macOS 实机验证、安装包构建、宣传渠道

---

## 里程碑

### M1 · v0.1 桌面端独立发布（进行中）

**目标**：Windows 不登录可用的纯本地剪切板工具。AGPL 开源，单机一人即可用。

- ✅ 桌面端 Tauri 2.x 脚手架
- ✅ 剪切板捕获（text / image / file）+ 归一化精确去重 + FTS5 全文索引
- 🔧 快速面板 UI（Windows 全闭环，Cmd+Shift+V + 键盘导航 + Enter 粘贴 + 忘记撤销）
- 🔧 Tray 图标 + 菜单（最小可用 + 关闭不退出）
- 🔧 端侧敏感检测（密码 / Token / 银行卡 / 身份证 / 手机 / 邮箱，含 Luhn + GB 11643 校验）
- 🔧 数据导出（JSON / Markdown + 图片副本 + metadata）
- 🔧 设置页（5 区：通用 / 隐私 / 云端 / 数据 / 关于）
- 📋 v0.1 发布前准备：实机验证、安装包、Release 页、宣传

### M2 · 后端登录链路就绪（计划中）

**目标**：为 Teamo 登录云端同步铺路的后端能力建设。

- OAuth 2.0 + PKCE 授权流
- `POST /api/memos/batch` 批量上传 API + 幂等中间件 + 跨设备去重
- `device_registry` 表 + 注册 / 撤销路由
- `GET /api/rules` 同步全局规则

### M3 · v0.2 桌面端登录上云（计划中）

**目标**：桌面端可选登录 → 自动上云 → Web 端 `/journal` 可见。

- OAuth 流程 + Keychain JWT 存储 + deep link callback
- 批量上传调度器 + 重试 + 幂等 + 错误处理
- 设置页云端连接状态 + 快速面板"已上云"标记
- 从云端拉规则更新合并到本地规则库

### M4 · 日卡片 + 来源筛选（计划中）

**目标**：AI 帮你整理昨天，第二天看到"整理好的昨天"。

- `diary_pipeline` 扩展输出 daily_cards JSON
- `/journal` 顶部 SourceFilter tab
- 列表 / 卡片 / 日记 三视图切换
- DailyCards + ClusterCard 组件

---

## 近期 Phase 2 增量

M1 的各卷都有 Phase 2 增量，不阻塞 v0.1 发布，但提升体验：

- **filter-engine Phase 2**：App 黑白名单（Windows `GetForegroundWindow` + `GetModuleFileNameEx` / macOS `NSWorkspace frontmostApplication.bundleIdentifier`）+ 域名规则 YAML seed 加载
- **quick-panel Phase 4**：macOS NSPanel（无 Dock + 不抢焦点）+ macOS 系统粘贴（CGEvent，需辅助功能权限引导）
- **tray-menu Phase 2**：4 色状态图标（绿/黄/红/灰）+ 动态 tooltip/菜单状态行
- **settings-page Phase 2-3**：主题切换（深色模式）+ 短文本最小长度 + 保留时长 + 检查更新
- **data-export Phase 2**：进度条 UI + tokio 后台任务 + 取消按钮

---

## 长期方向（M5+）

这些是路线图上的远方目标，时间表不定：

- **浏览器扩展**：同协议接入 TextView 云端，让你在浏览器里也能记录
- **移动端**（iOS / Android）：离线优先 + 云端同步，同一套数据跨平台
- **输入法集成**：Mac / Windows 候选词来源集成，写作时直接选历史
- **本地 AI 整理**：离线小模型做日卡片生成，彻底不依赖云端
- **国际化**：目前仅简体中文
- **应用商店上架**：Microsoft Store / Mac App Store（需签名成本）

---

## 如何影响 Roadmap

- **功能建议**：[GitHub Issues](https://github.com/mrliuzhiyu/teamo/issues) 使用「Feature Request」模板
- **发起讨论**：[GitHub Discussions](https://github.com/mrliuzhiyu/teamo/discussions) Ideas 分类
- **贡献代码**：参考 [CONTRIBUTING.md](CONTRIBUTING.md)
- **我们关心的高价值贡献**：
  - macOS / Windows 平台 bug 修复
  - 域名规则库扩充（[domain_rules.yaml](domain_rules.yaml)）
  - i18n（目前仅中文）
  - 性能优化

---

## 为什么这样排序

**短期**（M1 → M2 → M3）：先做桌面端独立可用，再做云端对接，最后做桌面登录体验。每一步都完整可用，不跳步。

**中期**（Phase 2 增量）：不阻塞发布的打磨，提升长期体验。

**长期**（M5+）：只有当 M1-M4 稳定后再展开。不在地基没打好前铺屋顶。

---

## 版本号策略

- **0.x.y-alpha**：pre-alpha 阶段，可能有不兼容变更
- **0.x.y**：alpha 稳定，接口相对稳定但仍可能调整
- **1.0.0**：正式发布，API / 数据格式稳定保证向后兼容

当前处于 0.0.x → 0.1.0-alpha 过渡。1.0 可能在 M3 完成后 6-12 个月内发布。

---

**下次审阅**：v0.1.0 正式发布时同步更新本文档。
