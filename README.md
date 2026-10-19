# dealer-integration

经销商线索 × 广告投放整合桌面客户端：整合多个门店的巨懂车线索与巨量引擎广告投放报表，提供汇总看板、线索明细查询与 Excel 导出。自用工具（Tauri 2 + Rust + Vue 3 + TypeScript）。

## 开发

```bash
pnpm install          # 安装前端依赖
pnpm tauri dev        # 本地开发（起 Tauri 壳 + Vite 热更新）
pnpm tauri build      # 出安装包（Windows: .msi/.exe；macOS: .app/.dmg）
pnpm typecheck        # 仅类型检查
pnpm build            # 仅构建前端（vue-tsc + vite build，产物在 dist/）
```

注意：`pnpm tauri dev/build` 需要目标平台的系统依赖（Windows: WebView2；macOS: Xcode CLT；Linux: webkit2gtk-4.0/4.1 开发库），不能跨平台交叉编译——各平台在其本机出包。

## 文档

- [docs/spec/0001](docs/spec/0001-dealer-lead-ad-integration-v1.md)：v1 spec（GitHub issue #1）
- [docs/adr/](docs/adr/)：关键决策（混合集成策略、技术栈、巨懂车取数方案）
- [GLOSSARY.md](GLOSSARY.md)：领域术语表
- UI 方向：变体 B（顶部大字看板），原型存档于 `prototype/ui-v1` 分支

## 工单与进度

见 GitHub Issues：#2（巨量开发者申请）、#3（巨懂车 spike）为人工前置任务，其余按依赖关系推进。
