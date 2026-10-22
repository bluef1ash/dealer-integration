//! 巨量引擎 Marketing API 客户端与 OAuth。
//! 端点/字段口径以官方文档为准，真实链路联调在手动冒烟中校正（spec 测试决策：平台联调不自动化）。

pub mod parse;

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::{AdReportRow, PlanReportRow};

const TOKEN_URL: &str = "https://ad.oceanengine.com/open_api/oauth2/access_token/";
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

/// 报表拉取请求参数（原先六个结伴参数收拢于此）
pub struct ReportQuery {
    pub data_level: &'static str,
    pub from_date: String,
    pub to_date: String,
    pub advertiser_ids: Vec<String>,
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

    /// 拉取一页报表（v3.0 多合一预置报表：data_level 进路径，GET + 查询参数，真机校正 2026-10-22）
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
        let url = format!(
            "https://ad.oceanengine.com/open_api/v3.0/{data_level}/report/integrated/get/",
            data_level = query.data_level
        );
        let filter = serde_json::json!({ "advertiser_ids": query.advertiser_ids }).to_string();
        let params = [
            ("start_date", query.from_date.clone()),
            ("end_date", query.to_date.clone()),
            ("page", query.page.to_string()),
            ("page_size", "200".to_string()),
            ("filter", filter),
        ];
        let text = self
            .http
            .get(url)
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
