# Issue 跟踪：GitHub

本仓库的 issue 和 spec 以 GitHub issue 的形式记录。所有操作使用 `gh` CLI。

## 约定

- **创建 issue**：`gh issue create --title "..." --body "..."`。多行正文用 heredoc。
- **读取 issue**：`gh issue view <number> --json number,title,body,labels,comments`。
- **列出 issue**：`gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`，并配合相应的 `--label` 和 `--state` 过滤条件。
- **把 issue 设为某个父 issue 的子 issue**：`gh issue create --parent <parent> ...`，或事后 `gh issue edit <parent> --add-sub-issue <child>`（`gh` 2.94+）。旧版 `gh`：`gh api --method POST repos/<owner>/<repo>/issues/<parent>/sub_issues -F sub_issue_id=<child-db-id>`（数据库 id，参见下文 **Blocking**）。无法使用 sub-issue 时，在子 issue 正文开头写 `Part of #<parent>`。
- **评论**：`gh issue comment <number> --body "..."`
- **添加 / 移除标签**：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **关闭**：`gh issue close <number> --comment "..."`

仓库归属从 `git remote -v` 推断；在 clone 内运行时 `gh` 会自动识别。

## Pull request 作为 triage 入口

**PR 作为请求入口：否。** _（如果本仓库把外部 PR 当作功能请求处理，改为 `yes`；`/triage` 会读取此标志。）_

设为 `yes` 后，PR 走与 issue 相同的标签和状态，命令换成 `gh pr` 等价形式：

- **读取 PR**：`gh pr view <number> --comments`；看 diff 用 `gh pr diff <number>`。
- **列出待 triage 的外部 PR**：`gh api --paginate 'repos/{owner}/{repo}/pulls?state=open' --jq '.[] | select(.author_association | IN("OWNER","MEMBER","COLLABORATOR") | not) | {number, title, author: .user.login, author_association, labels: [.labels[].name]}'`。
- **评论 / 打标签 / 关闭**：`gh pr comment`、`gh pr edit --add-label`/`--remove-label`、`gh pr close`。

GitHub 的 issue 和 PR 共用一套编号，所以 `#42` 可能是两者之一：先用 `gh pr view 42` 判断，失败时再用 `gh issue view 42`。

## 当技能说"发布到 issue 跟踪器"（publish to the issue tracker）

创建一个 GitHub issue。

## 当技能说"获取相关工单"（fetch the relevant ticket）

按上文 **读取 issue** 的方式读取。

## Wayfinder 操作

供 `/wayfinder` 使用。**地图（map）**是一个单独的 issue，其**子 issue** 即工单。

- **Map**：一个带 `wayfinder:map` 标签的 issue，正文包含 Notes / Decisions-so-far / Fog 三节。`gh issue create --label wayfinder:map`。
- **子工单**：作为 GitHub sub-issue 挂到 map 下（见 **把 issue 设为某个父 issue 的子 issue**）。sub-issue 不可用时，把子项加入 map 正文的任务列表，并在子 issue 正文开头写 `Part of #<map>`。标签：`wayfinder:<type>`（`research`/`prototype`/`grilling`/`task`）。被认领后，工单指派给驱动的开发者。
- **Blocking**：使用 GitHub 的**原生 issue 依赖（native issue dependencies）**，这是 UI 可见的权威表示。添加一条边：`gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`，其中 `<blocker-db-id>` 是阻塞方的数字**数据库 id**（`gh api repos/<owner>/<repo>/issues/<n> --jq .id`，不是 `#number`，也不是 `node_id`）。GitHub 通过 `issue_dependencies_summary.blocked_by` 报告（只统计未关闭的阻塞方，作为实时门禁）。依赖功能不可用时，回退为在子 issue 正文开头写 `Blocked by: #<n>, #<n>`。所有阻塞方都关闭后，工单即解除阻塞。
- **Frontier 查询**：列出 map 的未关闭子项（`gh issue list --state open`，限定在 map 的 sub-issue / 任务列表内），去掉有未关闭阻塞方（`issue_dependencies_summary.blocked_by > 0`，或 `Blocked by` 行中有未关闭 issue）或已有指派人的项；按 map 顺序取第一个。
- **认领**：`gh issue edit <n> --add-assignee @me`，这是会话的第一个写操作。
- **解决**：`gh issue comment <n> --body "<answer>"`，然后 `gh issue close <n>`，最后把上下文指针（gist + 链接）追加到 map 的 Decisions-so-far。
