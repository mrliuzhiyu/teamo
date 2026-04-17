<div align="center">

<img src="docs/logo.svg" width="80" height="80" alt="Teamo logo">

# Teamo

**你的人生记录 Agent** · Your life-recording agent

一款好用的本地智能剪切板工具。无感记录、智能过滤、可选同步。

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-lightgrey)]()
[![Status](https://img.shields.io/badge/status-pre--alpha-orange)]()

[特性](#特性) · [安装](#安装) · [隐私承诺](#隐私承诺) · [可选 · 云端同步](#可选--云端同步) · [开发](#开发) · [License](#license)

</div>

---

## 简介

Teamo 是一款专注于「无感记录人生」的本地剪切板工具。它默默运行在系统后台，每次你 `Ctrl+C / Cmd+C` 时自动捕获内容，本地端侧智能过滤敏感信息（密码、Token、银行卡号），把"有价值的"留下来。

**它对你**：是一款比 Paste / Maccy 多了点智能的本地剪切板工具，免费、开源、不需要登录。

**它实际承载的**：是「人生记录 Agent」的本地引擎——记录、打标、去重、聚合。如果你愿意登录 [TextView](https://textview.cn) 账号，捕获的精选内容会同步到云端，AI 帮你整理成可回看的日卡片，第二天打开网页一眼看到昨天。

> **关键不变量**：登录从来不是必选项。任何时候你可以登出，本地数据保留不变。
> 敏感数据（密码 / Token / 银行卡 / 身份证）**永远只在你自己电脑里**，登录与否都不上云。

---

## 特性

### 本地（不登录即可用）

- ⌨️ **自动捕获**：剪切板变化即记录（文本 / 图片 / 文件路径），零输入成本
- 🔍 **本地全文搜索**：SQLite FTS5，中英文毫秒级查找历史
- ⚡ **快速粘贴面板**：`Cmd+Shift+V`（macOS）/ `Ctrl+Shift+V`（Windows）唤起最近记录，Windows 下 Enter 直接粘贴到目标 App
- 🛡️ **端侧敏感拦截**：密码（多字符类型启发式）/ Token（sk-/ghp-/JWT）/ 银行卡（Luhn）/ 身份证（GB 11643）/ 手机 / 邮箱 共 6 类端侧检测，绝不上云
- 📋 **App 黑白名单**：1Password / Bitwarden / 银行客户端等敏感来源默认不记录；含 `<elevated>` 哨兵守护"以管理员运行的密码管理器不 bypass 规则"
- 🌐 **域名规则库**：70+ 条内置规则（抖音 / B 站 / 知乎 / 公众号 / 小红书 / YouTube / X / Reddit / Medium / 国内外各大网银 / 登录支付通配）随安装自动生效，启动时按版本号自动升级
- 📦 **数据导出**：JSON / Markdown 一键导出，图片字节级副本，数据永远归你
- ⏳ **保留时长自动清理**：永久 / 1 年 / 6 月 / 1 月 四档可选，启动时按策略清过期记录
- 🔕 **可暂停可关闭**：5 分钟 / 1 小时 / 手动恢复；关闭主窗口只隐藏不退出（Slack 风格）
- 🩺 **后台自愈**：capture 线程 panic 自动重启 + 心跳监控，避免"剪切板突然不记录了"而用户无感

### 登录后增值（连接 TextView 云端，可选）

- ☁️ **云端同步**：精选内容（敏感项除外）自动上云
- 🤖 **AI 整理**：链接卡片解析（视频元数据 / 文章摘要）、主题聚类、日卡片视图
- 🔄 **跨设备**：家里电脑 / 公司电脑 / 笔记本，记录跟着走
- 📰 **日卡片回看**：第二天打开 TextView Web `/journal` 看 AI 整理后的昨天

---

## 安装

> ⚠️ **当前处于 pre-alpha 阶段**，v0.1 尚未发布。GitHub Releases 一旦放出会更新此处。

### Windows

1. 从 [Releases](https://github.com/mrliuzhiyu/teamo/releases) 下载 `Teamo-Setup-x.y.z.exe`
2. 首次运行 Windows SmartScreen 可能警告 "未知发布者"——这是因为我们尚未购买 EV 代码签名证书。点 **更多信息 → 仍要运行**
3. 安装完成后 Teamo 会在系统托盘出现绿色图标

> 关于代码签名：v0.1 阶段使用 unsigned 发布以降低成本（EV Cert 约 ¥3000-5000/年）。等下载量积累后会评估申请。开源代码可审计，安全担忧可以阅读源码或自行编译。

### macOS

1. 从 [Releases](https://github.com/mrliuzhiyu/teamo/releases) 下载 `Teamo-x.y.z.dmg`
2. 首次运行 macOS Gatekeeper 警告："无法打开，因为它来自身份不明的开发者"
3. 系统设置 → 隐私与安全性 → 找到 Teamo → 仍要打开

> ⚠️ macOS 平台支持当前为 Phase 1（基础功能可用）。完整体验（无 Dock 图标的 NSPanel + 系统粘贴模拟 + App 黑白名单 source_app 抓取）留 Phase 4 与 CGEvent + 辅助功能权限引导一同上线。

### 自行编译

```bash
git clone https://github.com/mrliuzhiyu/teamo.git
cd teamo
pnpm install
pnpm tauri dev    # 开发模式
pnpm tauri build  # 生产构建
```

依赖：
- Rust 1.75+
- Node.js 20+
- pnpm 8+
- Tauri 2.x 平台依赖（[官方文档](https://tauri.app/start/prerequisites/)）

---

## 隐私承诺

Teamo 的核心承诺是：**敏感数据永远不离开你的电脑**。

我们如何保证这一点：

1. **代码层**：端侧敏感检测命中的内容，本地状态机标记 `local_only`，上行调度器**永远过滤**这个状态。这是单元测试守护的不变量。
2. **可见证据**：快速面板顶部"今日已记 N · 拦截 M"——每次打开都看见拦截的具体类型（密码 / Token / 银行卡号等）。
3. **开源审计**：本仓库 AGPL-3.0 许可，代码完全公开。任何人可审计验证承诺。
4. **用户可控**：每条本地记录可手动标记上云 / 不上云。

不登录使用 Teamo 时，**没有任何数据离开你的电脑**——也不收集任何遥测、使用统计、错误报告。

登录使用云端同步时，只有：
- 端侧闸门通过的精选内容（约 10-15% 的捕获率）
- 不含敏感正则命中项
- 不含黑名单 App 来源
- 不含 App 黑名单来源

会上传到 [TextView 云端](https://textview.cn)。云端隐私政策见 [textview.cn/privacy](https://textview.cn/privacy)。

---

## 可选 · 云端同步

云端同步是 Teamo 的可选增值能力，依托 [TextView](https://textview.cn) 商业服务。

### 如何启用

1. 安装 Teamo
2. 设置 → 云端连接 → 「连接 TextView 云端」
3. 浏览器跳转 OAuth 授权（OTP 邮箱或微信扫码）
4. 授权完成后桌面端自动开启上云

### 如何停用

设置 → 云端连接 → 「断开」即可。本地数据**全部保留**。

### 协议

云端对接协议（OAuth 2.0 + PKCE / `POST /api/memos/batch`）完全公开，详见 [docs/CLOUD_PROTOCOL.md](docs/CLOUD_PROTOCOL.md)（即将公开）。理论上任何人可以基于此协议实现自己的客户端接入 TextView 云端。

---

## 开发

### 项目结构

```
teamo/
├── src-tauri/        # Rust 后端
│   ├── src/
│   │   ├── clipboard/   # 剪切板监听
│   │   ├── storage/     # SQLite + FTS5
│   │   ├── filter/      # 端侧闸门（敏感正则 / 黑白名单 / SimHash / 域名规则）
│   │   ├── tray/        # Tray 菜单 + 全局快捷键
│   │   ├── auth/        # OAuth + Keychain
│   │   └── sync/        # 上行调度器
│   └── Cargo.toml
├── src/              # 前端（React + TypeScript + Tailwind）
│   ├── panel/        # 快速面板 UI
│   ├── settings/     # 设置页
│   └── main.tsx
├── docs/
│   ├── ARCHITECTURE.md
│   ├── CLOUD_PROTOCOL.md
│   └── PRIVACY.md
├── .github/workflows/
│   ├── ci.yml        # macOS + Windows + Linux 矩阵测试
│   └── release.yml   # 跨平台打包 + Release
├── LICENSE
├── README.md
└── CHANGELOG.md
```

### 贡献

参考 [CONTRIBUTING.md](CONTRIBUTING.md)（即将完善）。

我们关心的贡献方向：
- macOS / Windows 平台 bug 修复
- 域名规则库扩充（[scripts/domain_rules.yaml](scripts/domain_rules.yaml)）
- 国际化（i18n，目前仅中文）
- 性能优化

---

## License

Teamo 采用 **GNU Affero General Public License v3.0 (AGPL-3.0)**。

简单理解：
- ✅ 你可以自由使用、修改、分发本软件
- ✅ 你可以商用
- ⚠️ 任何基于 Teamo 修改后**对外提供网络服务**的项目，**必须同样开源** AGPL-3.0
- ⚠️ 任何基于 Teamo 修改后分发的版本，必须同样开源 AGPL-3.0

详见 [LICENSE](LICENSE)。

> 我们选 AGPL 而不是 MIT，是为了防止有人 fork Teamo 改改包装做闭源商业云端服务。

---

## 相关项目

- [TextView](https://textview.cn) — Teamo 的云端家园，负责整理与展示
- 未来：浏览器扩展、移动端、输入法集成（同协议接入云端）

---

<div align="center">

**Teamo** · 你的人生记录 Agent

</div>
