//! 巨量引擎响应解析：Marketing API 报表 → 领域行（入库接缝用脱敏样例驱动）。
//! 字段全部 default：平台加字段不炸；spend/conversion 平台可能返回字符串数字，容错转换。

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::{AdReportRow, PlanReportRow};

#[derive(Debug, Deserialize, Default)]
pub struct ApiResponse<T> {
    #[serde(default)]
    pub code: i64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: T,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReportData {
    #[serde(default)]
    pub list: Vec<ReportItem>,
    #[serde(default)]
    pub page_info: PageInfo,
}

#[derive(Debug, Deserialize, Default)]
pub struct PageInfo {
    #[serde(default)]
    pub page: i64,
    #[serde(default)]
    pub total_number: i64,
}

/// 报表行：字段名与层级随 data_level 不同，能取到的才填
#[derive(Debug, Deserialize, Default)]
pub struct ReportItem {
    #[serde(default)]
    pub advertiser_id: serde_json::Value,
    /// 账户层级无计划字段，留空
    #[serde(default)]
    pub promotion_id: serde_json::Value,
    #[serde(default)]
    pub promotion_name: String,
    #[serde(default)]
    pub ad_id: serde_json::Value,
    #[serde(default)]
    pub ad_name: String,
    /// YYYY-MM-DD
    #[serde(rename = "stat_time_day", default)]
    pub stat_time_day: String,
    #[serde(default)]
    pub spend: serde_json::Value,
    #[serde(default)]
    pub impression: serde_json::Value,
    #[serde(default)]
    pub click: serde_json::Value,
    #[serde(default)]
    pub conversion: serde_json::Value,
}

fn as_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

fn as_i64(v: &serde_json::Value) -> i64 {
    as_string(v).parse().unwrap_or(0)
}

fn as_f64(v: &serde_json::Value) -> f64 {
    as_string(v).parse().unwrap_or(0.0)
}

/// 计划维度 ID：优先 promotion_id，缺省回落 ad_id（口径随平台联调校正，issue #8）
fn dimension_id(item: &ReportItem) -> String {
    let pid = as_string(&item.promotion_id);
    if !pid.is_empty() && pid != "0" {
        return pid;
    }
    as_string(&item.ad_id)
}

fn dimension_name(item: &ReportItem) -> String {
    if !item.promotion_name.is_empty() {
        item.promotion_name.clone()
    } else {
        item.ad_name.clone()
    }
}

/// 通用报表明细行 → （账户级日行, 计划级日行）；账户级行无计划维度时计划行为空
pub fn rows_from_item(item: &ReportItem) -> (Option<AdReportRow>, Option<PlanReportRow>) {
    let advertiser_id = as_string(&item.advertiser_id);
    let ad = (!advertiser_id.is_empty()).then(|| AdReportRow {
        ad_account_id: advertiser_id.clone(),
        date: item.stat_time_day.clone(),
        spend: as_f64(&item.spend),
        impressions: as_i64(&item.impression),
        clicks: as_i64(&item.click),
        conversions: as_i64(&item.conversion),
    });
    let dim = dimension_id(item);
    let plan = (!dim.is_empty() && !dim.eq(&advertiser_id)).then(|| PlanReportRow {
        ad_account_id: advertiser_id,
        promotion_id: dim,
        promotion_name: dimension_name(item),
        date: item.stat_time_day.clone(),
        spend: as_f64(&item.spend),
        conversions: as_i64(&item.conversion),
        impressions: as_i64(&item.impression),
    });
    (ad, plan)
}

pub fn parse_report_response(body: &str) -> AppResult<(Vec<AdReportRow>, Vec<PlanReportRow>)> {
    let resp: ApiResponse<ReportData> = serde_json::from_str(body)
        .map_err(|e| AppError::Message(format!("报表响应解析失败: {e}")))?;
    if resp.code != 0 {
        return Err(AppError::remote(resp.code, &resp.message));
    }
    let mut ads = Vec::new();
    let mut plans = Vec::new();
    for item in &resp.data.list {
        let (ad, plan) = rows_from_item(item);
        if let Some(ad) = ad {
            ads.push(ad);
        }
        if let Some(plan) = plan {
            plans.push(plan);
        }
    }
    Ok((ads, plans))
}

#[derive(Debug, Deserialize, Default)]
pub struct TokenData {
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    /// 有效期（秒）
    #[serde(default)]
    pub expires_in: i64,
    #[serde(default)]
    pub advertiser_ids: Vec<serde_json::Value>,
}

pub fn parse_token_response(body: &str) -> AppResult<TokenData> {
    let resp: ApiResponse<TokenData> = serde_json::from_str(body)
        .map_err(|e| AppError::Message(format!("token 响应解析失败: {e}")))?;
    if resp.code != 0 {
        return Err(AppError::remote(resp.code, &resp.message));
    }
    Ok(resp.data)
}

/// OAuth 换 token 请求体（巨量的 grant_type 固定为 "auth_code"，
/// 不是标准 OAuth 的 "authorization_code"；官方文档参数不含 redirect_uri）
pub fn token_request_body(app_id: i64, secret: &str, auth_code: &str) -> serde_json::Value {
    serde_json::json!({
        "app_id": app_id,
        "secret": secret,
        "grant_type": "auth_code",
        "auth_code": auth_code,
    })
}

/// 刷新 token 请求体
pub fn refresh_request_body(app_id: i64, secret: &str, refresh_token: &str) -> serde_json::Value {
    serde_json::json!({
        "app_id": app_id,
        "secret": secret,
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/oceanengine");

    /// 账户层级样例 → 账户日行（无计划行）
    #[test]
    fn advertiser_sample_parses() {
        let body = std::fs::read_to_string(format!("{DIR}/report_advertiser_sample.json")).unwrap();
        let (ads, plans) = parse_report_response(&body).unwrap();
        assert_eq!(ads.len(), 2);
        assert!(plans.is_empty());
        assert_eq!(ads[0].ad_account_id, "1700000000");
        assert_eq!(ads[0].date, "2026-10-19");
        assert_eq!(ads[0].spend, 100.0);
        assert_eq!(ads[0].conversions, 3);
    }

    /// 计划层级样例 → 账户行 + 计划行
    #[test]
    fn plan_sample_parses() {
        let body = std::fs::read_to_string(format!("{DIR}/report_plan_sample.json")).unwrap();
        let (ads, plans) = parse_report_response(&body).unwrap();
        assert_eq!(ads.len(), 2);
        assert_eq!(plans.len(), 2);
        let p1 = plans
            .iter()
            .find(|p| p.promotion_id == "7691306192144973887")
            .unwrap();
        assert_eq!(p1.promotion_name, "示例计划甲");
        assert_eq!(p1.spend, 60.0);
    }

    #[test]
    fn business_error_surfaced() {
        let err = parse_report_response(r#"{"code":40100,"message":"token 失效"}"#).unwrap_err();
        assert!(err.to_string().contains("40100"));
    }

    #[test]
    fn token_request_shape_is_documented_contract() {
        let body = token_request_body(1700000000, "sec", "auth-code");
        assert_eq!(body["grant_type"], "auth_code");
        assert_eq!(body["auth_code"], "auth-code");
        assert_eq!(body["app_id"], 1700000000i64);
        assert!(body.get("redirect_uri").is_none());
        let refresh = refresh_request_body(1700000000, "sec", "r-token");
        assert_eq!(refresh["grant_type"], "refresh_token");
        assert_eq!(refresh["refresh_token"], "r-token");
    }
}
