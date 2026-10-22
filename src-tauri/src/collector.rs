//! 采集服务：手动「立即拉取」与调度器共用的拉取编排。
//! 窗口口径：常规拉取 7 天（状态变更 1 小时内覆盖），首次回填 90 天（spec #10）。

use serde::{Deserialize, Serialize};

use crate::credentials;
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::judongche::client::{Client as JdClient, LeadsWindow};
use crate::models::Store;
use std::time::Duration;

pub const PAGE_SIZE: u32 = 50;
const MAX_PAGES: u32 = 200;

/// 拉取窗口口径：已回填的店重拉近 7 天覆盖状态变更，未回填的走 90 天首次回填（spec #10）
pub fn window_days_for(backfilled: bool) -> i64 {
    if backfilled {
        7
    } else {
        90
    }
}

/// 会话失效特征（spike 实测 code 10014）
pub fn is_session_error(err: &AppError) -> bool {
    let text = err.to_string();
    text.contains("10014") || text.contains("授权过期")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullOutcome {
    pub store_id: i64,
    pub ok: bool,
    pub ingested: usize,
    pub session_expired: bool,
    pub error: String,
}

/// 拉取单店线索：复用会话直调列表接口，按页循环入库
pub async fn pull_store_leads(
    db: &Db,
    client: &JdClient,
    store: &Store,
    window_days: i64,
) -> AppResult<usize> {
    let cookie = db
        .with(|conn| credentials::load_store_cookie(conn, &store.account))?
        .ok_or_else(|| AppError::Message(format!("门店 {} 尚未登录巨懂车", store.name)))?;
    let now = crate::db::now();
    let window = LeadsWindow::list(now - window_days * 86_400, now, 1, PAGE_SIZE, false);
    let mut total = 0usize;
    for page in 1..=MAX_PAGES {
        let mut body = window.clone();
        body.page_number = Some(page);
        let text = client.list_leads(&cookie, &body).await?;
        let ingested =
            db.with(|conn| crate::ingest::ingest_list_response(conn, store.id, &text))?;
        total += ingested.upserted;
        if ingested.seen < PAGE_SIZE as usize {
            break;
        }
        // 温和翻页：行为特征接近真人浏览
        tokio::time::sleep(Duration::from_millis(800)).await;
    }
    Ok(total)
}

/// 拉取并登记结果；返回供调度与角标使用
pub async fn pull_store(
    db: &Db,
    client: &JdClient,
    store: &Store,
    window_days: i64,
) -> PullOutcome {
    match pull_store_leads(db, client, store, window_days).await {
        Ok(count) => {
            let _ = db.with(|conn| {
                crate::repo::stores::record_pull(conn, store.id, true, "", crate::db::now())
            });
            PullOutcome {
                store_id: store.id,
                ok: true,
                ingested: count,
                session_expired: false,
                error: String::new(),
            }
        }
        Err(e) => {
            eprintln!("[拉取] {} 失败: {e}", store.name);
            let expired = is_session_error(&e);
            let _ = db.with(|conn| {
                crate::repo::stores::record_pull(
                    conn,
                    store.id,
                    false,
                    &e.to_string(),
                    crate::db::now(),
                )
            });
            PullOutcome {
                store_id: store.id,
                ok: false,
                ingested: 0,
                session_expired: expired,
                error: e.to_string(),
            }
        }
    }
}

/// 巨量引擎报表拉取：单应用视角，逐账户取「账户×天」与「计划×天」两层，按日近 N 天。
/// v3.0 自定义报表单次仅支持一个账户；单账户/单层失败只记日志不中断（下轮调度仍可补）。
/// 多应用时由调用方逐应用传入各自的 access_token 与应用行 ID。
pub async fn pull_ocean_reports(
    db: &Db,
    client: &crate::ocean::Client,
    ocean_app_row: i64,
    access_token: &str,
    days: i64,
) -> AppResult<usize> {
    let (advertiser_ids, from_date, to_date) = db.with(|conn| {
        let ids: Vec<String> = crate::repo::leads::ad_account_ids_for_app(conn, ocean_app_row)?;
        let to = chrono::Local::now().date_naive();
        let from = to - chrono::Duration::days(days.max(1) - 1);
        Ok::<_, AppError>((ids, from.to_string(), to.to_string()))
    })?;
    if advertiser_ids.is_empty() {
        return Ok(0);
    }
    let mut total = 0usize;
    for account in &advertiser_ids {
        // 仅维度组合不同：账户级出点击数完整的账户日行，计划级出计划日行
        let base = |dimensions| crate::ocean::ReportQuery {
            advertiser_id: account.clone(),
            dimensions,
            from_date: from_date.clone(),
            to_date: to_date.clone(),
            page: 1,
        };
        // 开发者级频控（真机 40110）：请求间保持间隔，温和串行
        tokio::time::sleep(OCEAN_PACE).await;
        total += pull_level(db, client, access_token, base(ACCOUNT_DIMS), false).await?;
        tokio::time::sleep(OCEAN_PACE).await;
        total += pull_level(db, client, access_token, base(PLAN_DIMS), true).await?;
    }
    Ok(total)
}

/// 自定义报表维度组合：账户级仅统计日期；计划级加计划 ID 与名称
const ACCOUNT_DIMS: &[&str] = &["stat_time_day"];
const PLAN_DIMS: &[&str] = &["stat_time_day", "promotion_id", "promotion_name"];

/// 巨量报表请求节奏（12 账户 × 2 层 ≈ 24 次请求/轮，500ms 间隔留足频控余量）
const OCEAN_PACE: Duration = Duration::from_millis(500);

/// 拉取单账户单层报表并入库（翻页直到取尽）；失败只记日志并返回已入库行数。
/// plan_level=false 只入账户行、true 只入计划行（计划行不含点击数，账户行点击以账户级调用为准）
async fn pull_level(
    db: &Db,
    client: &crate::ocean::Client,
    access_token: &str,
    mut query: crate::ocean::ReportQuery,
    plan_level: bool,
) -> AppResult<usize> {
    let account = query.advertiser_id.clone();
    let label = if plan_level { "计划级" } else { "账户级" };
    let mut total = 0usize;
    let mut page = 1i64;
    loop {
        query.page = page;
        let data = match client.fetch_report(access_token, &query).await {
            Ok(data) => data,
            Err(e) => {
                eprintln!("[调度] 账户 {account} {label}报表拉取失败: {e}");
                return Ok(total);
            }
        };
        let (mut ads, mut plans) = crate::ocean::parse::parse_report_response(&data)?;
        let got = if plan_level { plans.len() } else { ads.len() };
        // 自定义报表可不带 advertiser 维度：账户 ID 由本地按所查账户注入
        for row in &mut ads {
            if row.ad_account_id.is_empty() {
                row.ad_account_id = account.to_string();
            }
        }
        for row in &mut plans {
            if row.ad_account_id.is_empty() {
                row.ad_account_id = account.to_string();
            }
        }
        let (ads, plans) = if plan_level {
            (Vec::new(), plans)
        } else {
            (ads, Vec::new())
        };
        total += db.with(|conn| crate::ocean::ingest_reports(conn, &ads, &plans))?;
        if got < crate::ocean::REPORT_PAGE_SIZE || page > 50 {
            return Ok(total);
        }
        tokio::time::sleep(OCEAN_PACE).await;
        page += 1;
    }
}
