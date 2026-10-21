# dealer-integration

经销商线索 × 广告投放整合桌面客户端：整合多个门店的巨懂车线索与巨量引擎广告投放报表，提供汇总看板、线索明细查询与 Excel 导出。自用工具（Tauri 2 + Rust + Vue 3 + TypeScript）。

## 开发

```bash
pnpm install          # 安装前端依赖
pnpm tauri dev        # 本地开发（起 Tauri 壳 + Vite 热更新）
pnpm tauri build      # 出安装包（Windows: .msi/.exe；macOS: .app/.dmg）
pnpm typecheck        # 仅类型检查
pnpm build            # 仅构建前端（vue-tsc + vite build，产物在 dist/）
cargo check           # Rust 编译检查（src-tauri/ 下运行）
cargo clippy          # Rust 静态检查（门槛见 src-tauri/Cargo.toml [lints]）
cargo fmt             # Rust 格式化（配置见根目录 rustfmt.toml）
```

注意：clippy 与 rustfmt 位于 Rust 发行版的 `clippy-preview/bin`、`rustfmt-preview/bin` 目录，需加入 PATH 后才能直接调用（本机示例：`D:\environment\rust\clippy-preview\bin`）。

注意：`pnpm tauri dev/build` 需要目标平台的系统依赖（Windows: WebView2；macOS: Xcode CLT；Linux: webkit2gtk-4.0/4.1 开发库），不能跨平台交叉编译——各平台在其本机出包。

## 文档

- [docs/spec/0001](docs/spec/0001-dealer-lead-ad-integration-v1.md)：v1 spec（GitHub issue #1）
- [docs/adr/](docs/adr/)：关键决策（混合集成策略、技术栈、巨懂车取数方案）
- [docs/databases.md](docs/databases.md)：本地数据库表结构
- [GLOSSARY.md](GLOSSARY.md)：领域术语表
- UI 方向：变体 B（顶部大字看板），原型存档于 `prototype/ui-v1` 分支

## 工单与进度

见 GitHub Issues。人工前置 #2（巨量开发者申请）、#3（巨懂车 spike）已完成；#5-#12（门店/线索/明细/巨量接入/看板/调度/恢复/对账）已实现，巨懂车主路径与导出三段式经真实抓包验证（ADR-0003 验证结论），平台真实联调（WebView 登录、OAuth 点击流）为手动冒烟项。
