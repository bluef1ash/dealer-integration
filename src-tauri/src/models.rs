//! 领域模型：跨模块共享的数据结构与查询参数（serde 供 Tauri command 边界直传前端）。

use serde::{Deserialize, Serialize};

/// 门店（管理单位，每店一个巨懂车账号）
#[derive(Debug, Clone, Serialize)]
pub struct Store {
    pub id: i64,
    pub name: String,
    /// 巨懂车账号标识（手机号）
    pub account: String,
}

/// 门店采集状态（调度与会话恢复共用）
#[derive(Debug, Clone, Serialize, Default)]
pub struct PullState {
    pub store_id: i64,
    /// 最近成功采集 unix 秒；None 表示从未成功
    pub last_success_at: Option<i64>,
    pub last_attempt_at: Option<i64>,
    pub last_error: String,
    /// 是否已完成 90 天首次回填
    pub backfilled: bool,
}

/// 线索（入库统一行格式：列表接口、导出快照解析后都归一到此）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LeadRow {
    pub leads_id: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status_key: String,
    #[serde(default)]
    pub status_label: String,
    /// 线索类型（私聊线索/广告直接线索）
    #[serde(default)]
    pub source_label: String,
    /// 意向车型 label
    #[serde(default)]
    pub intention: String,
    /// 留资时间 unix 秒
    #[serde(default)]
    pub leads_time: i64,
    #[serde(default)]
    pub ad_id: String,
    #[serde(default)]
    pub promotion_id: String,
    #[serde(default)]
    pub promotion_name: String,
    /// 原始 JSON 兜底（导出快照无对应列时为 {}）
    #[serde(default)]
    pub raw: serde_json::Value,
}

/// 线索明细筛选（出库接缝参数）
#[derive(Debug, Clone, Deserialize, Default)]
pub struct LeadsFilter {
    pub store_id: Option<i64>,
    /// 按状态 label 精确匹配；选项由前端从 distinct 查询获得
    pub status: Option<String>,
    /// 留资时间窗口 unix 秒（含头含尾）
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub page: u32,
    pub page_size: u32,
}

/// 看板时间范围
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Range {
    Today,
    Last7,
    Last30,
    /// 自定义：unix 秒区间（含头含尾）
    Custom { from: i64, to: i64 },
}

/// 看板聚合结果
#[derive(Debug, Serialize, Default)]
pub struct Dashboard {
    /// 时间范围内的总消耗（元）
    pub spend: f64,
    /// 主数字：单条线索成本（元，四舍五入）
    pub cpl: i64,
    pub leads: i64,
    pub deals: i64,
    pub store_cards: Vec<StoreCard>,
    pub plans: Vec<PlanLine>,
}

/// 看板门店卡片
#[derive(Debug, Serialize)]
pub struct StoreCard {
    pub store: Store,
    pub leads: i64,
    pub spend: f64,
    /// 近 14 天线索量走势（按天，缺数补 0；末位为今天）
    pub trend: Vec<i64>,
    pub last_success_at: Option<i64>,
    /// 会话是否有效；None 表示从未拉取过
    pub session_ok: Option<bool>,
}

/// 投放计划扁平表行
#[derive(Debug, Serialize)]
pub struct PlanLine {
    pub promotion_id: String,
    pub promotion_name: String,
    pub spend: f64,
    pub leads: i64,
    /// 当日有消耗视为投放中
    pub state: &'static str,
}

/// 巨量引擎账户层级日报行
#[derive(Debug, Clone, Deserialize)]
pub struct AdReportRow {
    pub ad_account_id: String,
    /// YYYY-MM-DD
    pub date: String,
    #[serde(default)]
    pub spend: f64,
    #[serde(default)]
    pub impressions: i64,
    #[serde(default)]
    pub clicks: i64,
    #[serde(default)]
    pub conversions: i64,
}

/// 巨量引擎计划层级日报行
#[derive(Debug, Clone, Deserialize)]
pub struct PlanReportRow {
    pub ad_account_id: String,
    pub promotion_id: String,
    #[serde(default)]
    pub promotion_name: String,
    pub date: String,
    #[serde(default)]
    pub spend: f64,
    #[serde(default)]
    pub conversions: i64,
}

/// 巨量广告账户（OAuth 授权返回的 advertiser）
#[derive(Debug, Clone, Serialize)]
pub struct AdAccount {
    pub id: String,
    pub alias: String,
}
