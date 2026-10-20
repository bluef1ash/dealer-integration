# 巨懂车测试夹具

录制自真实环境、脱敏后的响应样本，供入库接缝回放测试使用（spec #1 测试决策）。原始抓包与脱敏脚本不入库。

## leads_list_page1.json

「线索管理 → 流出线索」列表接口第 1 页真实响应（2026-10-19 录制，10 条，total 131）。

请求契约（spike 重放验证结论，见 #3）：

- `POST https://www.autoengine.com/motor/crm_wb/leads/scan/list?__method=window.fetch`
- 认证：浏览器会话 cookie（整罐；仅 passport `sessionid` 不够，CRM 层有独立会话）
- 必需头：`content-type: application/json`、`user-agent`、`jdc-saas-header-app-id`（固定应用实例 id）；`origin`/`referer` 非必需
- 请求体：`{"leads_time":{"min":<unix秒>,"max":<unix秒>},"page_key":"dealer_out_leads_list","only_out_leads":true,"is_dy":true,"page_number":1,"page_size":10}`
- **无签名参数**（无 msToken / X-Bogus），带会话即可直调——主路径成立
- 分页：`page_number` + `page_size`；响应含 `next_cursor`（格式 `"<leads_id>#"<leads_time>"`）与 `total`

脱敏规则：手机号（含 `phone_city.key`）→ `1380000xxxx`；人名/抖音昵称 → `测试用户N`/`示例抖音号N`；公司名 → `示例公司N`；抖音 UID/会话 ID → 假数字串；头像 URL 清空。业务 ID（`leads_id`、`ad_id`、`promotion_id`、门店编码等）保留以维持解析与归因测试的真实性。

响应中值得实现的字段：`leads_id`（主键）、`status.key/label`（线索状态，**导出表无此列**）、`leads_time`/`create_time`（unix 秒）、`intention_series`/`intention_car`（意向车型）、`followup_dealer`（跟进门店）、`extra.ad_id/promotion_id/promotion_name`（线索原生携带广告计划归因）。

## leads_export.xlsx

「流出线索」全量导出表（2026-10-20 经 async_export 任务 29225938 下载，时间窗 2026-04-24～2026-10-20，131 行 × 32 列）。

导出三段式契约（均已重放验证）：

1. `POST /motor/crm_wb/leads/scan/async_export`，请求体同列表接口但**不带分页**（全量）→ 返回 `{status:0, data:"<task_id>"}`
2. 轮询/列表：`GET /motor/crm_wb/task/instance/list?scene=105&page_number=1&page_size=10` → `{total, tasks:[{task_id, task_name:"导出<起>至<止>的数据", operator_name, status(2=完成), download_url(恒空), hint_msg, create_time}]}`
3. 下载：`POST /motor/crm_wb/task/crm_data/export`，请求体 `{"task_id":"<task_id>"}` → **直接返回 xlsx 字节流**（application/octet-stream，无中间 URL）

行为约束：点击导出会弹「风险提示」确认框（有感知行为）；导出记录列表实测 4 条、跨天保留——只做每日对账快照，不做高频导出。认证同列表接口（整罐 cookie + app-id 头，无签名参数）。

脱敏：按列策略替换——客户手机号/微信号 → `1380000xxxx`，抖音昵称 → `测试用户N`，商家/来源抖音昵称与商家名称 → `示例公司N`，广告/组件名称 → `示例计划N`，docProps 操作人已替换。状态、车型、城市、时间、全部业务 ID 原样保留；单元格数与原始一致。

导出表与列表接口的口径差异：导出列有 `线索推送ID`、`组件ID/组件名称`、`客户微信号` 等 JSON 里没有的字段；JSON 里的 `status`（线索状态）在导出表没有——对账合并时以 `线索ID` 为主键互补字段。
