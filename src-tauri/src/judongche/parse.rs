//! 巨懂车响应解析：接口 JSON → 领域行（LeadRow）。字段全部 `#[serde(default)]`，
//! 平台加字段不炸、缺字段落空值——解析只取客户端用到的列。

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::models::LeadRow;

/// 平台通用 KV 结构（{key, label}）
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Kv {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListResponse {
    #[serde(default)]
    pub status: i64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: ListData,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListData {
    #[serde(default)]
    pub list: Vec<LeadItem>,
    #[serde(default)]
    pub total: i64,
    /// 游标分页："<leads_id>#<leads_time>"；空串表示无下一页
    #[serde(rename = "next_cursor", default)]
    pub next_cursor: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct LeadItem {
    #[serde(default)]
    pub leads_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub status: Kv,
    #[serde(rename = "source_category", default)]
    pub source_category: Kv,
    #[serde(rename = "intention_series", default)]
    pub intention_series: Kv,
    #[serde(rename = "intention_car", default)]
    pub intention_car: Kv,
    /// 询价城市（city_info=省、phone_city=市；导出表「询价城市」为「省/市」同口径）
    #[serde(default)]
    pub city_info: Kv,
    #[serde(default)]
    pub phone_city: Kv,
    /// 流量类型（自然经营线索/广告直接线索等；导出表「流量类型」同口径）
    #[serde(default)]
    pub sub_product_type: Kv,
    /// 转化状态（私信/表单等转化动作；对应导出表「转化状态」）
    #[serde(default)]
    pub feiyu_action_type: Kv,
    /// 来源场景一级/二级（直播、短视频；导出表「一级/二级来源场景」同口径）
    #[serde(default)]
    pub from_scene: Kv,
    #[serde(default)]
    pub from_sub_scene: Kv,
    #[serde(default)]
    pub leads_time: i64,
    /// 飞鱼侧线索 ID（导出表线索 ID 同源）
    #[serde(default, rename = "feiyu_leads_id")]
    pub feiyu_leads_id: String,
    #[serde(default)]
    pub extra: Extra,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Extra {
    #[serde(rename = "ad_id", default)]
    pub ad_id: String,
    #[serde(rename = "promotion_id", default)]
    pub promotion_id: String,
    #[serde(rename = "promotion_name", default)]
    pub promotion_name: String,
}

/// 列表接口响应文本 → 领域行；业务失败（code 非 0）在解析阶段即报错
pub fn lead_rows_from_list(body: &str) -> AppResult<Vec<LeadRow>> {
    let resp: ListResponse = serde_json::from_str(body)
        .map_err(|e| AppError::Message(format!("列表响应解析失败: {e}")))?;
    if resp.status != 0 {
        return Err(AppError::remote(resp.status, &resp.message));
    }
    Ok(resp.data.list.iter().map(lead_item_to_row).collect())
}

/// 两级标签拼为「一级/二级」（省/市、来源场景一级/二级），空段跳过
fn join_labels(parts: [&str; 2]) -> String {
    parts
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("/")
}

fn lead_item_to_row(item: &LeadItem) -> LeadRow {
    let intention = [
        item.intention_series.label.as_str(),
        item.intention_car.label.as_str(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    LeadRow {
        leads_id: item.leads_id.clone(),
        phone: item.phone.clone(),
        name: item.name.clone(),
        status_key: item.status.key.clone(),
        status_label: item.status.label.clone(),
        source_label: item.source_category.label.clone(),
        intention,
        leads_time: item.leads_time,
        feiyu_leads_id: item.feiyu_leads_id.clone(),
        ad_id: item.extra.ad_id.clone(),
        promotion_id: item.extra.promotion_id.clone(),
        promotion_name: item.extra.promotion_name.clone(),
        city: join_labels([&item.city_info.label, &item.phone_city.label]),
        traffic_type: item.sub_product_type.label.clone(),
        conversion_status: item.feiyu_action_type.label.clone(),
        scene: join_labels([&item.from_scene.label, &item.from_sub_scene.label]),
        raw: serde_json::to_value(item).unwrap_or(serde_json::Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/judongche/leads_list_page1.json"
    );

    #[test]
    fn fixture_parses_to_ten_rows_with_expected_fields() {
        let body = std::fs::read_to_string(FIXTURE).unwrap();
        let rows = lead_rows_from_list(&body).unwrap();
        assert_eq!(rows.len(), 10);
        let first = rows.iter().find(|r| r.leads_id == "1115047139").unwrap();
        assert_eq!(first.phone, "13800001001");
        assert_eq!(first.status_label, "未联系");
        assert_eq!(first.intention, "iCAR超级V23 401两驱进阶版");
        assert_eq!(first.city, "辽宁/鞍山");
        assert_eq!(first.traffic_type, "自然经营线索");
        assert_eq!(first.conversion_status, "私信");
        assert_eq!(first.scene, "直播/直播进私留资");
        let ad_direct = rows.iter().find(|r| r.leads_id == "1109298389").unwrap();
        assert_eq!(ad_direct.promotion_id, "7691306192144973887");
        assert_eq!(ad_direct.source_label, "私聊线索");
        assert_eq!(ad_direct.traffic_type, "广告直接线索");
        assert_eq!(ad_direct.scene, "短视频/短视频进私留资");
    }

    #[test]
    fn business_error_is_surfaced() {
        let err = lead_rows_from_list(r#"{"status":1,"message":"未提供应用实例id"}"#).unwrap_err();
        assert!(err.to_string().contains("未提供应用实例id"));
    }
}
