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

/// 报表行包装：真实响应为 {dimensions, metrics} 嵌套，兼容旧式平铺字段（含脱敏夹具）。
/// 指标名做多候选映射（v2/v3 文档口径不一致：spend/stat_cost、conversion/convert 等）。
#[derive(Debug, Deserialize, Default)]
#[serde(transparent)]
pub struct ReportItem {
    raw: serde_json::Value,
}

const NULL: serde_json::Value = serde_json::Value::Null;

impl ReportItem {
    /// 依次查 dimensions → metrics → 顶层
    fn get(&self, key: &str) -> &serde_json::Value {
        self.raw
            .get("dimensions")
            .and_then(|d| d.get(key))
            .or_else(|| self.raw.get("metrics").and_then(|m| m.get(key)))
            .or_else(|| self.raw.get(key))
            .unwrap_or(&NULL)
    }

    /// 多候选键名取首个非空字符串值
    fn str_field(&self, keys: &[&str]) -> String {
        for key in keys {
            let v = self.get(key);
            if !v.is_null() {
                let s = as_string(v);
                if !s.is_empty() && s != "0" {
                    return s;
                }
            }
        }
        String::new()
    }
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

/// 通用报表明细行 → （账户级日行, 计划级日行）；计划维度取不到时计划行为空
pub fn rows_from_item(item: &ReportItem) -> (Option<AdReportRow>, Option<PlanReportRow>) {
    let advertiser_id = item.str_field(&["advertiser_id"]);
    if advertiser_id.is_empty() {
        return (None, None);
    }
    let date = item.str_field(&["stat_time_day"]);
    let spend = as_f64(item.get("spend"));
    let impressions = as_i64(item.get("impression"));
    let clicks = as_i64(item.get("click"));
    let conversions = as_i64(item.get("conversion"));
    let ad = AdReportRow {
        ad_account_id: advertiser_id.clone(),
        date: date.clone(),
        spend,
        impressions,
        clicks,
        conversions,
    };
    // 计划维度：优先 promotion_id，缺省回落 ad_id（口径随平台联调校正，issue #8）
    let dim = item.str_field(&["promotion_id", "ad_id"]);
    let plan = (!dim.is_empty() && dim != advertiser_id).then(|| PlanReportRow {
        ad_account_id: advertiser_id,
        promotion_id: dim,
        promotion_name: item.str_field(&["promotion_name", "ad_name"]),
        date,
        spend,
        conversions,
        impressions,
    });
    (Some(ad), plan)
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
    // 有行却全被跳过：字段口径对不上，打出首行原文便于联调校正
    if !resp.data.list.is_empty() && ads.is_empty() {
        eprintln!(
            "[报表] 行字段未识别（首行原文前 300 字）：{}",
            serde_json::to_string(&resp.data.list[0].raw)
                .unwrap_or_default()
                .chars()
                .take(300)
                .collect::<String>()
        );
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

    /// 真实响应形状：list 行为 {dimensions, metrics} 嵌套，指标为数字
    #[test]
    fn dimensions_metrics_shape_parses() {
        let body = r#"{"code":0,"message":"OK","data":{"list":[
            {"dimensions":{"stat_time_day":"2026-10-22","advertiser_id":"1700000000",
                           "promotion_id":"7691306192144973887","promotion_name":"计划甲"},
             "metrics":{"spend":12.34,"impression":1000,"click":30,"conversion":2}},
            {"dimensions":{"stat_time_day":"2026-10-22","advertiser_id":"1700000000"},
             "metrics":{"spend":99.9,"impression":5000,"click":120,"conversion":4}}
        ],"page_info":{"page":1,"total_number":2}}}"#;
        let (ads, plans) = parse_report_response(body).unwrap();
        // 嵌套形状下账户行由计划行派生（口径同平面形状），账户行 2 条、计划行 1 条
        assert_eq!(ads.len(), 2);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].promotion_name, "计划甲");
        assert_eq!(plans[0].spend, 12.34);
        assert_eq!(ads[1].conversions, 4);
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
