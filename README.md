# dealer-integration

经销商线索 × 广告投放整合桌面客户端：整合多个门店的巨懂车线索与巨量引擎广告投放报表，提供汇总看板、竞价投放、线索明细查询与 Excel 导出。自用工具（Tauri 2 + Rust + Vue 3 + TypeScript + naive-ui + pinia + vue-echarts）。

## 功能概览（v1.0）

- **总览看板**：单条线索成本主数字、门店卡片（14 天走势 sparkline）、近 14 天线索量柱图与均值。
- **竞价投放**：账户/计划层级投放表现（消耗、转化、展示、转化率、单线索成本），按日/近 7 天/近 30 天切换；「拉取巨量报表」手动触发（与调度器同链路）。
- **线索明细**：手机号/姓名/车型搜索，状态/线索类型/询价城市/流量类型/转化状态/来源场景六维筛选，留资时间日期范围，Excel 导出；「拉取线索」手动触发巨懂车采集。
- **多门店与多应用**：每店一个巨懂车账号（支持自动重登）；巨量引擎应用可添加多个（OAuth 授权在应用内弹窗完成）。
- **采集调度**：间隔可配（15-1440 分钟）的自动拉取 + 每日对账快照；窗口关闭可驻留系统托盘后台继续采集（首次关闭弹窗询问，设置页可改）。
- **数据与凭证**：全部存本地 SQLite（业务数据、会话 cookie、巨懂车密码、巨量 token 与应用凭证），跨平台随库文件迁移，不依赖系统凭证库。

## 开发

```bash
pnpm install          # 安装前端依赖
pnpm tauri dev        # 本地开发（起 Tauri 壳 + Vite 热更新）
pnpm tauri build      # 出安装包（Windows: .exe；macOS: .dmg；Linux: .deb/.AppImage）
pnpm typecheck        # 仅类型检查
pnpm build            # 仅构建前端（vue-tsc + vite build，产物在 dist/）
cargo check           # Rust 编译检查（src-tauri/ 下运行）
cargo clippy          # Rust 静态检查（门槛见 src-tauri/Cargo.toml [lints]）
cargo fmt             # Rust 格式化（配置见根目录 rustfmt.toml）
```

注意：clippy 与 rustfmt 位于 Rust 发行版的 `clippy-preview/bin`、`rustfmt-preview/bin` 目录。

### 提交钩子

husky 在每次提交前依次跑 `pnpm typecheck`、`cargo fmt --check`、`cargo clippy`、`cargo test`（脚本见 `.husky/pre-commit`）。`pnpm install` 时经 `prepare` 脚本自动启用，无需手动配置。

紧急跳过用 `git commit --no-verify`（逃生口只在提交命令上，代码内不允许压制检测）。

注意：`pnpm tauri dev/build` 需要目标平台的系统依赖（Windows: WebView2；macOS: Xcode CLT；Linux: webkit2gtk-4.0/4.1 开发库），不能跨平台交叉编译——各平台在其本机出包。

### 多平台打包（CI）

GitHub Actions（`.github/workflows/release.yml`）在三个平台出安装包：

- 触发：PR 到 main（跨平台编译验证）、手动 workflow_dispatch、推 `v*` tag。
- 产物：Windows（nsis，中文安装向导）、macOS universal dmg（Apple Silicon 与 Intel 通用）、Linux（deb/AppImage）。
- **tag 触发时版本号跟随 tag**：`scripts/sync-version.mjs` 把 tag 版本写入 package.json / tauri.conf.json / Cargo.toml 后再出包，并创建草稿 Release 供确认发布；PR / 手动触发则从 Actions Artifacts 下载产物，不建 Release。
- **tag 命名必须是三段式 `v1.0.0`**：workflow 只认 `v*` 前缀，且版本同步脚本用严格 `x.y.z` 正则校验（`v1.0` / `1.0` 都不会生效）。

## 文档

- [docs/spec/0001](docs/spec/0001-dealer-lead-ad-integration-v1.md)：v1 spec（GitHub issue #1）
- [docs/adr/](docs/adr/)：关键决策（混合集成策略、技术栈、巨懂车取数方案）
- [docs/databases.md](docs/databases.md)：本地数据库表结构（含凭证存储与升级说明）
- [GLOSSARY.md](GLOSSARY.md)：领域术语表
- UI 方向：变体 B（顶部大字看板），原型存档于 `prototype/ui-v1` 分支

## 工单与进度

见 GitHub Issues。#2-#13 已全部关闭：门店/线索/明细/巨量接入/看板/调度/恢复/对账均已实现，凭证全面入 SQLite（keyring 已移除）。

巨量引擎链路经 2026-10 真机四轮校正打通：OAuth 官方授权形态 + 应用内弹窗拦截回调、报表 v3.0 自定义报表（`api.oceanengine.com/open_api/v3.0/report/custom/get/`）、授权账户按角色展开（工作台 EBP / 代理商 select，企业号等非广告角色跳过）。端点与字段口径的探测结论留档在各提交信息与 `src-tauri/src/ocean/` 模块注释。

注意：自凭证库版本升级**不自动迁移**凭证，需重新登录门店并重录巨量应用（详见 docs/databases.md 升级说明）。
