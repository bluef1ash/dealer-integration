//! 巨量引擎 Marketing API 客户端与 OAuth。
//! 端点/字段口径以官方文档为准，真实链路联调在手动冒烟中校正（spec 测试决策：平台联调不自动化）。

pub mod accounts;
pub mod parse;

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::{AdReportRow, PlanReportRow};

const TOKEN_URL: &str = "https://ad.oceanengine.com/open_api/oauth2/access_token/";
// v3.0 报表唯一存活路由是「自定义报表」custom/get，域名 api.oceanengine.com
// （2026-10-22 无凭证探测：ad./api. 两域的 v2 integrated 与 v3 预置路径全部 404，
// 仅 custom/get 返回业务码 40105；请求口径对照官方文档 1741387668314126）
const REPORT_URL: &str = "https://api.oceanengine.com/open_api/v3.0/report/custom/get/";
/// 请求指标（官方 v3 口径；若平台报字段不合法，用「获取自定义报表可用指标和维度」接口核对）
const REQUEST_METRICS: &[&str] = &["stat_cost", "show_cnt", "click_cnt", "convert_cnt"];
/// 自定义报表单页行数（官方上限 100）
pub const REPORT_PAGE_SIZE: usize = 100;
// 账户同步路由（2026-10-22 无凭证探测均在 api.oceanengine.com 存活）：
// token 授权返回的 id 角色多样，报表仅认客户角色，工作台角色需展开为真实广告账户
/// 已授权账户查询（带 account_role 角色）
const AUTHORIZED_ACCOUNTS_URL: &str = "https://api.oceanengine.com/open_api/oauth2/advertiser/get/";
/// 升级版工作台下账户列表（工作台角色展开用）
const EBP_ACCOUNT_LIST_URL: &str = "https://api.oceanengine.com/open_api/2/ebp/advertiser/list/";
/// 代理商名下广告主（AGENT/CHILD_AGENT 角色展开用；GET，响应 data.list 为纯 id 数组）
const AGENT_ADVERTISER_SELECT_URL: &str =
    "https://api.oceanengine.com/open_api/2/agent/advertiser/select/";
// 授权页真机校正（2026-10-22，与开发者后台生成的官方授权链接逐字比对）：
// 域名为 open.oceanengine.com/audit/oauth.html，参数名是 app_id（非 client_id），
// 不携带 redirect_uri（回调绑定在后台登记的地址）；旧 external/oauth/authorize 已 404，
// ad.oceanengine.com/openapi/audit/oauth.html 对该应用报「app有误」
const AUTHORIZE_URL: &str = "https://open.oceanengine.com/audit/oauth.html";

/// 拼授权链接（官方授权链接参数：app_id + state；素材授权 material_auth 与追踪 rid 按需可选）
pub fn authorize_url(app_id: &str) -> String {
    format!("{AUTHORIZE_URL}?app_id={app_id}&state=dealer-integration")
}

/// 从回调 URL 提取 auth_code（官方回调形如
/// `http://127.0.0.1:1720/?app_id=…&auth_code=…&scope=[…]&state=&uid=…`）
pub fn extract_auth_code(callback_url: &str) -> Option<String> {
    let query = callback_url.split_once('?')?.1;
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("auth_code=") {
            let decoded = percent_decode(value);
            if !decoded.is_empty() {
                return Some(decoded);
            }
        }
    }
    None
}

/// 百分号解码（scope/编码字符不会进 auth_code，仅兜底）
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub struct Client {
    http: reqwest::Client,
}

/// 自定义报表拉取参数（v3.0 单次仅支持一个广告账户，多账户由采集层循环）
pub struct ReportQuery {
    pub advertiser_id: String,
    /// 维度组合：账户级 ["stat_time_day"]；计划级再加 promotion_id/promotion_name
    pub dimensions: &'static [&'static str],
    pub from_date: String,
    pub to_date: String,
    pub page: i64,
}

impl Client {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// 授权码换 token
    pub async fn exchange_token(
        &self,
        app_id: i64,
        secret: &str,
        auth_code: &str,
    ) -> AppResult<parse::TokenData> {
        let body = parse::token_request_body(app_id, secret, auth_code);
        self.token(body).await
    }

    /// 刷新 token
    pub async fn refresh_token(
        &self,
        app_id: i64,
        secret: &str,
        refresh_token: &str,
    ) -> AppResult<parse::TokenData> {
        let body = parse::refresh_request_body(app_id, secret, refresh_token);
        self.token(body).await
    }

    async fn token(&self, body: serde_json::Value) -> AppResult<parse::TokenData> {
        let text = self
            .http
            .post(TOKEN_URL)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        parse::parse_token_response(&text)
    }

    /// 拉取一页自定义报表（GET；复杂参数序列化为 JSON 串，start/end 用官方 datetime 口径）
    pub async fn fetch_report(&self, access_token: &str, query: &ReportQuery) -> AppResult<String> {
        #[derive(Deserialize)]
        struct Envelope {
            #[serde(default)]
            code: i64,
            #[serde(default)]
            message: String,
            #[serde(default)]
            data: serde_json::Value,
        }
        let params = [
            ("advertiser_id", query.advertiser_id.clone()),
            ("data_topic", "BASIC_DATA".to_string()),
            (
                "dimensions",
                serde_json::json!(query.dimensions).to_string(),
            ),
            ("metrics", serde_json::json!(REQUEST_METRICS).to_string()),
            // 平台校验 filters 必传（真机 40000），无过滤条件传空数组
            ("filters", "[]".to_string()),
            ("start_time", format!("{} 00:00:00", query.from_date)),
            ("end_time", format!("{} 23:59:59", query.to_date)),
            ("page", query.page.to_string()),
            ("page_size", REPORT_PAGE_SIZE.to_string()),
        ];
        let text = self
            .http
            .get(REPORT_URL)
            .header("Access-Token", access_token)
            .query(&params)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        // 帧校验提前到客户端层，解析由 parse 模块负责；解析失败带原文片段便于联调
        let env: Envelope = serde_json::from_str(&text).map_err(|e| {
            AppError::Message(format!(
                "报表响应解析失败: {e}（原文前 300 字：{}）",
                text.chars().take(300).collect::<String>()
            ))
        })?;
        if env.code != 0 {
            return Err(AppError::remote(env.code, &env.message));
        }
        Ok(serde_json::to_string(&env.data)?)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

/// 报表入库：补档广告账户后 upsert 账户级与计划级日行
pub fn ingest_reports(
    conn: &rusqlite::Connection,
    ads: &[AdReportRow],
    plans: &[PlanReportRow],
) -> crate::error::AppResult<usize> {
    for id in ads
        .iter()
        .map(|r| r.ad_account_id.as_str())
        .chain(plans.iter().map(|r| r.ad_account_id.as_str()))
    {
        crate::repo::leads::ensure_ad_account(conn, id, "")?;
    }
    let n = crate::repo::leads::upsert_ad_reports(conn, ads)?
        + crate::repo::leads::upsert_plan_reports(conn, plans)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    /// 报表入库接缝：样例解析 → 补档账户 → upsert；重放幂等
    #[test]
    fn ingest_report_fixtures_persists_and_is_idempotent() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/oceanengine");
        let ad_body =
            std::fs::read_to_string(format!("{dir}/report_advertiser_sample.json")).unwrap();
        let plan_body = std::fs::read_to_string(format!("{dir}/report_plan_sample.json")).unwrap();
        let db = Db::open_in_memory();

        let run = |db: &Db| -> usize {
            db.with(|conn| {
                let (ads, _) = parse::parse_report_response(&ad_body)?;
                let (_, plans) = parse::parse_report_response(&plan_body)?;
                ingest_reports(conn, &ads, &plans)
            })
            .unwrap()
        };

        run(&db);
        let first = dump(&db);
        run(&db);
        assert_eq!(dump(&db), first);
        // 账户档已补：A=1700000000；账户日行 2 条、计划日行 2 条
        db.with(|conn| {
            let accounts: i64 =
                conn.query_row("SELECT COUNT(*) FROM ad_accounts", [], |r| r.get(0))?;
            assert_eq!(accounts, 1);
            let ad_rows: i64 =
                conn.query_row("SELECT COUNT(*) FROM ad_reports_daily", [], |r| r.get(0))?;
            assert_eq!(ad_rows, 2);
            let plan_rows: i64 =
                conn.query_row("SELECT COUNT(*) FROM plan_reports_daily", [], |r| r.get(0))?;
            assert_eq!(plan_rows, 2);
            Ok(())
        })
        .unwrap();
    }

    fn dump(db: &Db) -> Vec<(String, String, f64)> {
        db.with(|conn| {
            let mut stmt = conn
                .prepare("SELECT ad_account_id, date, spend FROM ad_reports_daily ORDER BY date")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(crate::error::AppError::Db)?;
            Ok::<_, crate::error::AppError>(rows)
        })
        .unwrap()
    }

    #[test]
    fn authorize_url_matches_official_shape() {
        let url = authorize_url("1700000000");
        // 与开发者后台生成的官方授权链接同构：open 域 + app_id 参数 + state，无 redirect_uri
        assert_eq!(
            url,
            "https://open.oceanengine.com/audit/oauth.html?app_id=1700000000&state=dealer-integration"
        );
    }

    /// 真机回调 URL（2026-10-22 实测）能正确提取 auth_code；scope 编码段不干扰
    #[test]
    fn extract_auth_code_from_real_callback() {
        let callback = "http://127.0.0.1:1720/?app_id=1877930494126810\
             &auth_code=674f2747c3e4a40c2f3fe505943eb14c432ed4a1\
             &material_auth_status=0&scope=%5B10000000%2C300000000%5D&state=&uid=3281404888618206";
        assert_eq!(
            extract_auth_code(callback).as_deref(),
            Some("674f2747c3e4a40c2f3fe505943eb14c432ed4a1")
        );
        assert_eq!(
            extract_auth_code("https://open.oceanengine.com/audit/oauth.html?app_id=1"),
            None
        );
    }
}
