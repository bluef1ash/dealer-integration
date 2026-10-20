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
pub async fn pull_store_leads(db: &Db, client: &JdClient, store: &Store, window_days: i64) -> AppResult<usize> {
    let cookie = credentials::load_store_cookie(&store.account)?
        .ok_or_else(|| AppError::Message(format!("门店 {} 尚未登录巨懂车", store.name)))?;
    let now = crate::db::now();
    let window = LeadsWindow::list(now - window_days * 86_400, now, 1, PAGE_SIZE, false);
    let mut total = 0usize;
    for page in 1..=MAX_PAGES {
        let mut body = window.clone();
        body.page_number = Some(page);
        let text = client.list_leads(&cookie, &body).await?;
        let ingested = db.with(|conn| crate::ingest::ingest_list_response(conn, store.id, &text))?;
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
pub async fn pull_store(db: &Db, client: &JdClient, store: &Store, window_days: i64) -> PullOutcome {
    match pull_store_leads(db, client, store, window_days).await {
        Ok(count) => {
            let _ = db.with(|conn| crate::repo::stores::record_pull(conn, store.id, true, "", crate::db::now()));
            PullOutcome { store_id: store.id, ok: true, ingested: count, session_expired: false, error: String::new() }
        }
        Err(e) => {
            let expired = is_session_error(&e);
            let _ = db.with(|conn| {
                crate::repo::stores::record_pull(conn, store.id, false, &e.to_string(), crate::db::now())
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

/// 巨量引擎报表拉取：账户 + 计划层级，按日近 N 天，入库并返回行数
pub async fn pull_ocean_reports(
    db: &Db,
    client: &crate::ocean::Client,
    access_token: &str,
    days: i64,
) -> AppResult<usize> {
    let (advertiser_ids, from_date, to_date) = db.with(|conn| {
        let ids: Vec<String> = crate::repo::leads::ad_account_ids(conn)?;
        let to = chrono::Local::now().date_naive();
        let from = to - chrono::Duration::days(days.max(1) - 1);
        Ok::<_, AppError>((ids, from.to_string(), to.to_string()))
    })?;
    if advertiser_ids.is_empty() {
        return Ok(0);
    }
    let mut total = 0usize;
    for level in ["ADVERTISER_AD_DAY", "AD_AD_DAY"] {
        let mut page = 1i64;
        loop {
            let data = client.fetch_report(access_token, level, &from_date, &to_date, &advertiser_ids, page).await?;
            let (ads, plans) = crate::ocean::parse::parse_report_response(&data)?;
            let batch = ads.len() + plans.len();
            total += db.with(|conn| crate::ocean::ingest_reports(conn, &ads, &plans))?;
            if batch == 0 || page > 50 {
                break;
            }
            page += 1;
        }
    }
    Ok(total)
}
