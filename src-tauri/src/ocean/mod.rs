//! 巨量引擎 Marketing API 客户端与 OAuth。
//! 端点/字段口径以官方文档为准，真实链路联调在手动冒烟中校正（spec 测试决策：平台联调不自动化）。

pub mod parse;

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::{AdReportRow, PlanReportRow};

const TOKEN_URL: &str = "https://ad.oceanengine.com/open_api/oauth2/access_token/";
const REPORT_URL: &str = "https://ad.oceanengine.com/open_api/2/report/integrated/get/";
const AUTHORIZE_URL: &str = "https://www.oceanengine.com/external/oauth/authorize";

/// 桌面端 OAuth 回调：本机回环固定端口
pub const REDIRECT_PORT: u16 = 1720;
pub fn redirect_uri() -> String {
    format!("http://127.0.0.1:{REDIRECT_PORT}/callback")
}

/// 拼授权链接（与开放平台「APPID 管理」页生成的默认授权 URL 同构）
pub fn authorize_url(app_id: &str) -> String {
    format!(
        "{AUTHORIZE_URL}?client_id={app_id}&redirect_uri={}&state=dealer-integration",
        urlencode(&redirect_uri())
    )
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
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
        redirect_uri: &str,
    ) -> AppResult<parse::TokenData> {
        let body = parse::token_request_body(app_id, secret, auth_code, redirect_uri);
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

    /// 拉取一页报表（data_level：账户=ADVERTISER_AD_DAY，计划=AD_AD_DAY）
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
        let body = serde_json::json!({
            "data_level": query.data_level,
            "start_date": query.from_date,
            "end_date": query.to_date,
            "fields": ["advertiser_id", "promotion_id", "promotion_name", "ad_id", "ad_name",
                       "stat_time_day", "spend", "impression", "click", "conversion"],
            "filter": { "advertiser_ids": query.advertiser_ids },
            "page": query.page,
            "page_size": 200,
        });
        let text = self
            .http
            .post(REPORT_URL)
            .header("Access-Token", access_token)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        // 帧校验提前到客户端层，解析由 parse 模块负责
        let env: Envelope = serde_json::from_str(&text)
            .map_err(|e| AppError::Message(format!("报表响应解析失败: {e}")))?;
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
    fn authorize_url_encodes_redirect() {
        let url = authorize_url("1700000000");
        assert!(url.starts_with(
            "https://www.oceanengine.com/external/oauth/authorize?client_id=1700000000"
        ));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A1720%2Fcallback"));
    }
}
