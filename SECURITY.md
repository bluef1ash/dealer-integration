# 安全政策（Security Policy）

## 支持版本

本项目采用滚动维护，仅支持 `main` 分支最新提交。发现疑似漏洞时，请先在最新 `main` 上确认仍可复现，再按下方渠道报告。

## 报告漏洞

**请勿在公开 Issue、PR、评论或任何公开渠道披露漏洞细节。**

请通过 GitHub 的私密漏洞报告提交：
[Private vulnerability reporting](https://github.com/bluef1ash/car-intent-server/security/advisories/new)

报告请包含：

- 影响的模块、接口或代码路径，以及复现步骤（或 PoC）；
- 影响评估（机密性 / 完整性 / 可用性）；
- 若涉及客户数据（PII），一律只提供脱敏样本——仓库隐私红线同样适用于漏洞报告。

维护者会在收到报告后确认并跟进修复；修复发布后在对应 advisory 中致谢报告者（可要求匿名）。

## 依赖安全实践

- **漏洞扫描**：后端全部 Go 模块以 `govulncheck` 扫描，以「代码可触达」的漏洞为升级驱动；仅存在于依赖清单但未被调用的问题单独评估。前端跟随 GitHub Dependabot。
- **自动化更新**：Dependabot 每周检查 Go 模块、前端 pnpm 依赖与 GitHub Actions 版本；minor/patch 合并为单一批量 PR，major 单独评估。
- **升级分级**：可触达的安全补丁优先合并；数据库驱动、消息客户端等结构性大版本升级独立分支，完成全量测试回归后合并。
- **内部 fork**：`github.com/ledongthuc/pdf` 使用内部 fork `bluef1ash/pdf`，上游安全修复需手工同步，在依赖审计时单独核对。
