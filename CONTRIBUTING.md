# 贡献指南

感谢你对 Teamo 的兴趣！

## 本地开发

### 依赖

- **Node.js** 20+
- **pnpm** 9+（`npm install -g pnpm`）
- **Rust** 1.75+（[rustup.rs](https://rustup.rs)）
- **Tauri 2.x 系统依赖**（按平台见 [Tauri 官方文档](https://tauri.app/start/prerequisites/)）

### 启动

```bash
git clone https://github.com/mrliuzhiyu/teamo.git
cd teamo
pnpm install
pnpm icons        # 生成图标（首次需要）
pnpm tauri dev    # 启动开发模式
```

### 常用命令

| 命令 | 作用 |
|---|---|
| `pnpm dev` | 仅启动前端 Vite dev server（不带 Tauri） |
| `pnpm build` | 仅前端构建 |
| `pnpm tauri dev` | 完整桌面端开发模式 |
| `pnpm tauri build` | 生产构建（生成 .exe / .dmg / .deb） |
| `pnpm type-check` | TypeScript 类型检查 |
| `pnpm icons` | 从 `docs/logo.svg` 生成多分辨率图标 |
| `cd src-tauri && cargo check` | Rust 静态检查 |
| `cd src-tauri && cargo test` | Rust 单元测试 |

## 提交 PR

1. Fork 本仓库
2. 从 `main` 切出 feature 分支：`git checkout -b feat/your-feature`
3. 改完 commit（中文 message，参考现有格式：`新增/优化/修复/重构: xxx`）
4. push 到你的 fork
5. 在 GitHub 提 PR，关联相关 issue
6. CI 必须全绿才能合并

## Code Style

- **Rust**：`cargo fmt` 格式化；`cargo clippy` 通过
- **TypeScript / React**：保持与现有代码一致
- **CSS**：Tailwind 优先，避免自定义 CSS

## 报告 Bug

请使用 [Bug Report Issue 模板](.github/ISSUE_TEMPLATE/bug_report.md)，附上：
- 操作系统 + 版本
- Teamo 版本（`Settings → 关于`）
- 复现步骤
- 期望 vs 实际行为
- 相关日志或截图

## 提议新功能

请使用 [Feature Request Issue 模板](.github/ISSUE_TEMPLATE/feature_request.md)，描述使用场景。

## 关于规则库

`domain_rules.yaml` 是开放贡献的——欢迎补充国内/国外平台规则。提 PR 时按现有 yaml 格式追加即可。

## 协议

提交贡献即视为同意你的代码以 Apache-2.0 协议发布。

---

## DCO（Developer Certificate of Origin）

每个 commit 应当包含 `Signed-off-by` 行（`git commit -s`），声明你拥有提交内容的合法权利且同意以本项目协议发布。
