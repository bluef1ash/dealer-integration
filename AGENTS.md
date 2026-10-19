# AGENTS.md

## 仓库

桌面客户端（Tauri 2 + Rust + Vue 3 + TypeScript），整合巨懂车线索与巨量引擎广告报表。常用命令见 README「开发」节：`pnpm tauri dev` 本地开发、`pnpm tauri build` 出安装包、`pnpm typecheck` 类型检查、`cargo check`（src-tauri/，需平台系统依赖）。


## 代码编写硬规则

- 注释：必须用中文解释复杂逻辑；简单赋值、直观调用不要写空洞注释。
- 禁止禁用代码检测：严禁使用 `//nolint`、`//lint:ignore`、`eslint-disable`、`@ts-ignore`、`@ts-nocheck`、`biome-ignore` 等注释绕过静态检查、类型检查或格式检查。检测失败必须通过修复实现、拆分职责或改正类型解决，不得以任何禁用注释压制。
- 增量修改：优先小步、安全、可验证的修改，避免无必要的大规模重写。
- 文件规模：手写业务代码单文件建议不超过 **300 行**，最大上限 **500 行**，严禁超过 **1000 行**。超过限制时必须按职责拆分为同 package / 同模块内的多个文件。
- 生成代码例外：由工具生成的代码不适用手写代码行数限制，禁止手工拆分或编辑。
- 功能拆分：复杂功能先按职责拆成多个小功能和稳定结构，再逐个实现，避免一开始陷入细节。
- 复用优先：开发新功能前必须先检查现有组件、函数、工具库；已有相似实现时扩展原实现，不新建同类轮子。
- 工具函数归口：后端可复用通用工具函数统一放入 `backend/utils/common/`；前端通用工具函数按子项目规则放入对应 `src/utils/`。
- 文档同步：涉及 proto、数据库表结构、业务范围控制或跨服务链路变更时，必须同步根/子目录 `README.md`、根/子目录 `AGENTS.md` 和 `docs/databases.md` 的当前状态说明。

## 工作流

- `main` 为默认分支；功能开发在 `feature/*` 分支上进行（当前：`feature/ui`）。
- Issue 记录在 `bluef1ash/dealer-integration` 的 GitHub Issues 中；所有创建/读取/打标签/关闭操作遵循 `docs/agents/issue-tracker.md`。

## 技能

- `.agents/skills/` 存放 mattpocock 工程技能，由 `skills-lock.json` 锁定（每个技能的来源路径 + 哈希）。不要手工修改 `.agents/skills/` 下的任何文件——手改会偏离锁定的哈希；如需变更请通过 lockfile 更新。

## Agent skills

### Issue tracker

Issue 记录在本仓库的 GitHub Issues（仓库：bluef1ash/dealer-integration），通过 `gh` CLI 操作。见 `docs/agents/issue-tracker.md`。

### Triage labels

五个规范化 triage 角色，标签字符串与角色名相同。见 `docs/agents/triage-labels.md`。

### Domain docs

单一上下文（single-context）：仓库根目录一份 `GLOSSARY.md` + `docs/adr/`。见 `docs/agents/domain.md`。
