# Teamo · 隐私政策

> 最后更新：2026-04-15
> 适用版本：v0.1+

Teamo 的隐私承诺非常简单：**你的数据，你做主**。

## 不登录使用 Teamo（默认）

不登录使用 Teamo 时：

- ✅ Teamo **不收集**任何使用数据、遥测、错误报告、设备信息
- ✅ Teamo **不发起**任何对外网络请求（除了用户手动触发"检查更新"）
- ✅ 所有剪切板内容**完全保留在你自己电脑上**

数据存储位置：

| 平台 | 路径 |
|---|---|
| Windows | `%APPDATA%\Teamo\` |
| macOS | `~/Library/Application Support/Teamo/` |
| Linux | `~/.local/share/Teamo/` |

主要文件：
- `clipboard.db` — 本地 SQLite 数据库（剪切板历史）
- `settings.json` — 设置项
- `app_rules.json` / `domain_rules.yaml` — 规则库副本

## 端侧敏感数据拦截（不可关闭的承诺）

无论你是否登录，Teamo 都会在端侧识别以下类型的内容并**永远不上云**：

- 密码（高熵短串）
- API Token（`sk-*`、`ghp_*`、`Bearer xxx`、JWT 三段式）
- 银行卡号（Luhn 校验通过）
- 中国身份证号（18 位 + 校验码）
- 手机号（中国 11 位）
- 邮箱

这些内容**保留在你本地**可以搜索和粘贴使用，但绝不会通过任何上云路径离开你的设备。

代码层：
- 端侧检测命中后状态机标记 `local_only`
- 上行调度器的 `where state == 'pending_upload'` 查询天然过滤
- 单元测试守护这个不变量

可见证据：
- 快速面板顶部"今日已记 N · 拦截 M"
- 点击展开看具体拦截类型分布

## 登录使用 Teamo（连接 TextView 云端）

登录后启用云端同步时，**只有以下数据**会上传到 [TextView](https://textview.cn) 云端：

| 数据 | 上传 | 备注 |
|---|---|---|
| 端侧闸门通过的剪切板内容 | ✅ | 约捕获总量的 10-15%（敏感 / 黑名单 / 重复 / 短文本全过滤） |
| `device_id`（随机 UUID）| ✅ | 用于跨设备去重，不关联硬件指纹 |
| `device_name`（你设的设备名） | ✅ | 默认 hostname，可在设置改 |
| `source_app` / `source_url` / `source_title`（剪切来源） | ✅ | 帮助云端 enrich 链接卡片 |
| `client_version`（Teamo 版本） | ✅ | 兼容性判断 |
| **敏感正则命中项** | ❌ | **永远不上传** |
| **App 黑名单内容** | ❌ | **永远不上传** |
| **域名 skip_upload 规则命中** | ❌ | **永远不上传** |
| 用户名 / 密码 / 浏览历史 / 截屏 / 键盘记录 | ❌ | **Teamo 根本不读这些** |

云端的进一步隐私政策见 [TextView 隐私政策](https://textview.cn/privacy)。

## 第三方依赖

Teamo 不嵌入任何第三方分析 / 广告 / 错误追踪 SDK（Sentry / Bugsnag / Google Analytics / Firebase 等都没有）。

唯一的网络通信对象：
- 不登录时：仅 GitHub Releases（"检查更新"功能）
- 登录后：仅你授权的 TextView 云端

## 数据导出与删除

- **导出**：Settings → 数据 → 导出（JSON / Markdown）—— 全量本地数据一键下载
- **删除单条**：快速面板每条记录 → "忘记这条"
- **清空本地**：Settings → 数据 → 清空本地数据
- **删除云端**：登录用户在 TextView Web 端操作

## 协议变更

任何隐私政策的实质性变更会：
1. 在 GitHub Releases 的 release notes 显著标注
2. 桌面端启动时弹窗提示
3. 这份文件 `git log` 可追溯每次变更

## 联系

数据处理疑问或申诉：[GitHub Issues](https://github.com/mrliuzhiyu/teamo/issues) 或邮件 `54liuzhiyu@gmail.com`。
