//! 出库接缝（spec #1 测试决策）：seed SQLite → 查询函数 → 断言筛选与聚合。
//! 看板、明细筛选、状态选项都从这里的函数走，Tauri command 只做薄转发。
//!
//! 门店范围统一用「(?N IS NULL OR store_id = ?N)」可空参数表达，避免 SQL 分支拼接。

use chrono::TimeZone;
use rusqlite::Connection;

use crate::error::AppResult;
use crate::models::{Dashboard, LeadDetail, LeadPage, LeadsFilter, PlanLine, Range, StoreCard};

const DAY: i64 = 86_400;

/// 时间范围 → unix 秒区间（本地时区；自定义原样返回）
pub fn range_bounds(range: &Range) -> (i64, i64) {
    let today = chrono::Local::now().date_naive();
    let start_of = |d: chrono::NaiveDate| {
        chrono::Local
            .from_local_datetime(&d.and_hms_opt(0, 0, 0).expect("零点有效"))
            .single()
            .expect("零点唯一")
            .timestamp()
    };
    match range {
        Range::Today => (start_of(today), crate::db::now()),
        Range::Last7 => (
            start_of(today - chrono::Duration::days(6)),
            crate::db::now(),
        ),
        Range::Last30 => (
            start_of(today - chrono::Duration::days(29)),
            crate::db::now(),
        ),
        Range::Custom { from, to } => (*from, *to),
    }
}

/// 状态筛选选项（随库内真实 label 走，避免与平台枚举硬编码脱节）
pub fn distinct_statuses(conn: &Connection, store_id: Option<i64>) -> AppResult<Vec<String>> {
    let rows = match store_id {
        Some(id) => {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT status_label FROM leads WHERE status_label <> '' AND store_id = ?1 ORDER BY status_label",
            )?;
            let rows = stmt
                .query_map([id], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        }
        None => {
            let mut stmt =
                conn.prepare("SELECT DISTINCT status_label FROM leads WHERE status_label <> '' ORDER BY status_label")?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        }
    };
    Ok(rows)
}

/// 线索分页查询（时间/门店/状态可组合；留资时间倒序；page_size=0 不分页，供导出复用同一筛选）
pub fn leads_page(conn: &Connection, filter: &LeadsFilter) -> AppResult<LeadPage> {
    let mut where_sql = String::from("1=1");
    let mut bind: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(store_id) = filter.store_id {
        where_sql.push_str(&format!(" AND l.store_id = ?{}", bind.len() + 1));
        bind.push(Box::new(store_id));
    }
    if let Some(status) = &filter.status {
        where_sql.push_str(&format!(" AND l.status_label = ?{}", bind.len() + 1));
        bind.push(Box::new(status.clone()));
    }
    if let Some(q) = filter.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        where_sql.push_str(&format!(
            " AND (l.phone LIKE ?{} OR l.name LIKE ?{n} OR l.intention LIKE ?{n} OR l.promotion_name LIKE ?{n})",
            bind.len() + 1,
            n = bind.len() + 1
        ));
        bind.push(Box::new(format!("%{q}%")));
    }
    if let Some(from) = filter.from {
        where_sql.push_str(&format!(" AND l.leads_time >= ?{}", bind.len() + 1));
        bind.push(Box::new(from));
    }
    if let Some(to) = filter.to {
        where_sql.push_str(&format!(" AND l.leads_time <= ?{}", bind.len() + 1));
        bind.push(Box::new(to));
    }

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM leads l WHERE {where_sql}"),
        rusqlite::params_from_iter(bind.iter().map(|p| p.as_ref())),
        |r| r.get(0),
    )?;

    let limit_sql = if filter.page_size == 0 {
        String::new()
    } else {
        format!(
            " LIMIT {} OFFSET {}",
            filter.page_size,
            (filter.page as u64) * (filter.page_size as u64)
        )
    };
    let sql = format!(
        "SELECT l.leads_id, l.store_id, s.name, l.phone, l.name, l.status_label, l.source_label,
                l.intention, l.leads_time, l.promotion_name
         FROM leads l JOIN stores s ON s.id = l.store_id
         WHERE {where_sql}
         ORDER BY l.leads_time DESC, l.leads_id DESC{limit_sql}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            rusqlite::params_from_iter(bind.iter().map(|p| p.as_ref())),
            |r| {
                Ok(LeadDetail {
                    leads_id: r.get(0)?,
                    store_id: r.get(1)?,
                    store_name: r.get(2)?,
                    phone: r.get(3)?,
                    name: r.get(4)?,
                    status_label: r.get(5)?,
                    source_label: r.get(6)?,
                    intention: r.get(7)?,
                    leads_time: r.get(8)?,
                    promotion_name: r.get(9)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LeadPage { total, rows })
}

fn local_date(ts: i64) -> String {
    chrono::Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| d.date_naive().to_string())
        .unwrap_or_default()
}

/// 看板聚合：主指标 + 门店卡片（14 天走势）+ 计划扁平表。
/// 消耗统计经 store_ad_accounts 映射到店；计划表列出全部已知计划（含范围内无消耗的）。
pub fn dashboard(conn: &Connection, range: &Range, store_id: Option<i64>) -> AppResult<Dashboard> {
    let (from, to) = range_bounds(range);
    let now = crate::db::now();
    // 报表按日粒度：秒级窗口换算本地日期串；越界侧收敛到今天，防自定义远期溢出
    let from_date = if from <= 0 {
        "1970-01-01".to_string()
    } else {
        local_date(from)
    };
    let to_date = if to >= now {
        local_date(now)
    } else {
        local_date(to)
    };
    let today = local_date(now);

    let leads: i64 = conn.query_row(
        "SELECT COUNT(*) FROM leads l WHERE l.leads_time BETWEEN ?1 AND ?2 AND (?3 IS NULL OR l.store_id = ?3)",
        rusqlite::params![from, to, store_id],
        |r| r.get(0),
    )?;
    let deals: i64 = conn.query_row(
        "SELECT COUNT(*) FROM leads l
         WHERE l.leads_time BETWEEN ?1 AND ?2 AND l.status_label = '已成交' AND (?3 IS NULL OR l.store_id = ?3)",
        rusqlite::params![from, to, store_id],
        |r| r.get(0),
    )?;
    let spend: f64 = conn.query_row(
        "SELECT COALESCE(SUM(spend), 0) FROM ad_reports_daily
         WHERE date BETWEEN ?1 AND ?2
           AND (?3 IS NULL OR ad_account_id IN (SELECT ad_account_id FROM store_ad_accounts WHERE store_id = ?3))",
        rusqlite::params![from_date, to_date, store_id],
        |r| r.get(0),
    )?;
    let (conversions, impressions): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(conversions), 0), COALESCE(SUM(impressions), 0) FROM ad_reports_daily
         WHERE date BETWEEN ?1 AND ?2
           AND (?3 IS NULL OR ad_account_id IN (SELECT ad_account_id FROM store_ad_accounts WHERE store_id = ?3))",
        rusqlite::params![from_date, to_date, store_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let cpl = if leads > 0 {
        (spend / leads as f64).round() as i64
    } else {
        0
    };

    // 门店卡片：逐店统计线索/消耗与 14 天走势，附采集状态
    let stores = crate::repo::stores::list(conn)?;
    let pull_states: std::collections::HashMap<i64, crate::models::PullState> =
        crate::repo::stores::pull_states(conn)?
            .into_iter()
            .map(|s| (s.store_id, s))
            .collect();
    let today_start = range_bounds(&Range::Today).0;
    let mut store_cards = Vec::new();
    for store in stores {
        if let Some(sid) = store_id {
            if store.id != sid {
                continue;
            }
        }
        let s_leads: i64 = conn.query_row(
            "SELECT COUNT(*) FROM leads WHERE store_id = ?1 AND leads_time BETWEEN ?2 AND ?3",
            rusqlite::params![store.id, from, to],
            |r| r.get(0),
        )?;
        let s_spend: f64 = conn.query_row(
            "SELECT COALESCE(SUM(spend), 0) FROM ad_reports_daily
             WHERE date BETWEEN ?1 AND ?2
               AND ad_account_id IN (SELECT ad_account_id FROM store_ad_accounts WHERE store_id = ?3)",
            rusqlite::params![from_date, to_date, store.id],
            |r| r.get(0),
        )?;
        let mut trend = Vec::with_capacity(14);
        for k in (0..14).rev() {
            let day_start = today_start - k * DAY;
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM leads WHERE store_id = ?1 AND leads_time BETWEEN ?2 AND ?3",
                rusqlite::params![store.id, day_start, day_start + DAY - 1],
                |r| r.get(0),
            )?;
            trend.push(n);
        }
        let state = pull_states.get(&store.id);
        // 会话有效性：从未拉取为 None；最近一次成功不晚于最近一次尝试视为有效
        let session_ok = state.and_then(|s| match (s.last_success_at, s.last_attempt_at) {
            (None, _) => None,
            (Some(ok), Some(attempt)) => Some(ok >= attempt),
            (Some(_), None) => Some(true),
        });
        store_cards.push(StoreCard {
            store,
            leads: s_leads,
            spend: s_spend,
            trend,
            last_success_at: state.and_then(|s| s.last_success_at),
            session_ok,
        });
    }

    // 计划扁平表：全部已知计划；范围内消耗求和，今日有消耗视为投放中
    let today_spend: std::collections::HashMap<String, f64> = conn
        .prepare(
            "SELECT promotion_id, SUM(spend) FROM plan_reports_daily
             WHERE date = ?1 AND (?2 IS NULL OR ad_account_id IN (SELECT ad_account_id FROM store_ad_accounts WHERE store_id = ?2))
             GROUP BY promotion_id",
        )?
        .query_map(rusqlite::params![today, store_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })?
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    let mut plans: Vec<PlanLine> = conn
        .prepare(
            "SELECT promotion_id, MAX(promotion_name), SUM(spend), SUM(conversions), SUM(impressions)
             FROM plan_reports_daily
             WHERE (?1 IS NULL OR ad_account_id IN (SELECT ad_account_id FROM store_ad_accounts WHERE store_id = ?1))
             GROUP BY promotion_id ORDER BY 3 DESC",
        )?
        .query_map(rusqlite::params![store_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(promotion_id, promotion_name, spend, conversions, impressions)| PlanLine {
            state: if today_spend.get(&promotion_id).copied().unwrap_or(0.0) > 0.0 { "投放中" } else { "已暂停" },
            leads: 0,
            conversions,
            impressions,
            promotion_id,
            promotion_name,
            spend,
        })
        .collect();
    let mut leads_by_promo: std::collections::HashMap<String, i64> = conn
        .prepare(
            "SELECT promotion_id, COUNT(*) FROM leads
             WHERE promotion_id <> '' AND leads_time BETWEEN ?1 AND ?2 AND (?3 IS NULL OR store_id = ?3)
             GROUP BY promotion_id",
        )?
        .query_map(rusqlite::params![from, to, store_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    for plan in &mut plans {
        plan.leads = leads_by_promo.remove(&plan.promotion_id).unwrap_or(0);
    }

    Ok(Dashboard {
        spend,
        cpl,
        leads,
        deals,
        conversions,
        impressions,
        store_cards,
        plans,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::models::{AdReportRow, LeadRow, PlanReportRow};

    /// 确定性时间基准：以今天零点为锚，构造跨今天的日期数据
    fn today_start() -> i64 {
        range_bounds(&Range::Today).0
    }
    const DAY: i64 = 86_400;

    fn lead(id: &str, time: i64, status: &str, promo: &str) -> LeadRow {
        LeadRow {
            leads_id: id.into(),
            phone: format!("13800001{n:03}", n = id.len()),
            status_label: status.into(),
            promotion_id: promo.into(),
            leads_time: time,
            ..Default::default()
        }
    }

    /// 统一测试数据：
    /// L1 城东店 今10时 未联系 P1；L2 城东店 今11时 已成交 P1；
    /// L3 城东店 3天前 未联系 P2；L5 城东店 40天前 已成交 P2；L4 城西店 今9时 未联系 P1
    /// A1（映射城东店）今日消耗 100、3天前 50、40天前 999；计划 P1 今日 60、P2 3天前 40
    fn seed(db: &Db) {
        db.with(|conn| {
            crate::repo::stores::insert(conn, "城东店", "13800000001")?;
            crate::repo::stores::insert(conn, "城西店", "13800000002")?;
            let t0 = today_start();
            crate::repo::leads::upsert_leads(
                conn,
                1,
                &[
                    lead("L1", t0 + 36000, "未联系", "P1"),
                    lead("L2", t0 + 39600, "已成交", "P1"),
                    lead("L3", t0 - 3 * DAY, "未联系", "P2"),
                    lead("L5", t0 - 40 * DAY, "已成交", "P2"),
                ],
            )?;
            crate::repo::leads::upsert_leads(conn, 2, &[lead("L4", t0 + 32400, "未联系", "P1")])?;
            crate::repo::leads::ensure_ad_account(conn, "A1", "账户一")?;
            conn.execute(
                "INSERT INTO store_ad_accounts (store_id, ad_account_id) VALUES (1, 'A1')",
                [],
            )?;
            let d = |days_ago: i64| {
                chrono::Local
                    .timestamp_opt(t0 - days_ago * DAY, 0)
                    .single()
                    .unwrap()
                    .date_naive()
                    .to_string()
            };
            crate::repo::leads::upsert_ad_reports(
                conn,
                &[
                    AdReportRow {
                        ad_account_id: "A1".into(),
                        date: d(0),
                        spend: 100.0,
                        ..Default::default()
                    },
                    AdReportRow {
                        ad_account_id: "A1".into(),
                        date: d(3),
                        spend: 50.0,
                        ..Default::default()
                    },
                    AdReportRow {
                        ad_account_id: "A1".into(),
                        date: d(40),
                        spend: 999.0,
                        ..Default::default()
                    },
                ],
            )?;
            crate::repo::leads::upsert_plan_reports(
                conn,
                &[
                    PlanReportRow {
                        ad_account_id: "A1".into(),
                        promotion_id: "P1".into(),
                        promotion_name: "计划甲".into(),
                        date: d(0),
                        spend: 60.0,
                        ..Default::default()
                    },
                    PlanReportRow {
                        ad_account_id: "A1".into(),
                        promotion_id: "P2".into(),
                        promotion_name: "计划乙".into(),
                        date: d(3),
                        spend: 40.0,
                        ..Default::default()
                    },
                ],
            )?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn filters_combine_time_store_status() {
        let db = Db::open_in_memory();
        seed(&db);
        db.with(|conn| {
            // 仅状态
            let page = leads_page(
                conn,
                &LeadsFilter {
                    status: Some("未联系".into()),
                    page_size: 10,
                    ..Default::default()
                },
            )?;
            assert_eq!(page.total, 3);
            // 状态 + 门店
            let page = leads_page(
                conn,
                &LeadsFilter {
                    status: Some("未联系".into()),
                    store_id: Some(1),
                    page_size: 10,
                    ..Default::default()
                },
            )?;
            assert_eq!(page.total, 2);
            // 仅时间窗（3 天前后各留一天，只圈中 L3）
            let t0 = today_start();
            let page = leads_page(
                conn,
                &LeadsFilter {
                    from: Some(t0 - 4 * DAY),
                    to: Some(t0 - 2 * DAY),
                    page_size: 10,
                    ..Default::default()
                },
            )?;
            assert_eq!(page.total, 1);
            assert_eq!(page.rows[0].leads_id, "L3");
            // 分页（page 从 0 计）：留资时间倒序 L2,L1,L4,L3,L5 → 第 2 页为 L4,L3
            let page = leads_page(
                conn,
                &LeadsFilter {
                    page: 1,
                    page_size: 2,
                    ..Default::default()
                },
            )?;
            assert_eq!(page.total, 5);
            assert_eq!(page.rows.len(), 2);
            assert_eq!(page.rows[0].leads_id, "L4");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn dashboard_seven_and_thirty_day_ranges() {
        let db = Db::open_in_memory();
        seed(&db);
        db.with(|conn| {
            // 近7天：L1/L2/L4（今日）+ L3（3天前）；消耗 100+50=150
            let d7 = dashboard(conn, &Range::Last7, None)?;
            assert_eq!(d7.leads, 4);
            assert_eq!(d7.spend, 150.0);
            // 近30天：同近7天（L5 在 40 天前，依旧排除）
            let d30 = dashboard(conn, &Range::Last30, None)?;
            assert_eq!(d30.leads, 4);
            assert_eq!(d30.spend, 150.0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn status_options_are_distinct_and_sorted() {
        let db = Db::open_in_memory();
        seed(&db);
        db.with(|conn| {
            assert_eq!(
                distinct_statuses(conn, None)?,
                vec!["已成交".to_string(), "未联系".to_string()]
            );
            assert_eq!(
                distinct_statuses(conn, Some(2))?,
                vec!["未联系".to_string()]
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn dashboard_aggregates_custom_range() {
        let db = Db::open_in_memory();
        seed(&db);
        db.with(|conn| {
            // 近 5 天 ～ 今天：排除 40 天前的 L5 与 999 消耗
            let t0 = today_start();
            let d = dashboard(
                conn,
                &Range::Custom {
                    from: t0 - 5 * DAY,
                    to: t0 + DAY,
                },
                None,
            )?;
            assert_eq!(d.leads, 4);
            assert_eq!(d.deals, 1);
            assert_eq!(d.spend, 150.0);
            assert_eq!(d.cpl, 38);
            // 门店卡片：城东店 3 条全部消耗归属，城西店 1 条无消耗
            let east = d
                .store_cards
                .iter()
                .find(|c| c.store.name == "城东店")
                .unwrap();
            assert_eq!(east.leads, 3);
            assert_eq!(east.spend, 150.0);
            let west = d
                .store_cards
                .iter()
                .find(|c| c.store.name == "城西店")
                .unwrap();
            assert_eq!(west.leads, 1);
            assert_eq!(west.spend, 0.0);
            // 计划扁平表：P1 60 元 3 线索（L1/L2/L4）；P2 40 元 1 线索（L3）
            assert_eq!(d.plans.len(), 2);
            let p1 = d.plans.iter().find(|p| p.promotion_id == "P1").unwrap();
            assert_eq!((p1.spend, p1.leads), (60.0, 3));
            let p2 = d.plans.iter().find(|p| p.promotion_id == "P2").unwrap();
            assert_eq!((p2.spend, p2.leads), (40.0, 1));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn dashboard_today_scope_and_trend() {
        let db = Db::open_in_memory();
        seed(&db);
        db.with(|conn| {
            let d = dashboard(conn, &Range::Today, None)?;
            assert_eq!(d.leads, 3);
            assert_eq!(d.deals, 1);
            assert_eq!(d.spend, 100.0);
            assert_eq!(d.cpl, 33);
            // 14 天走势：末位为今天各店线索量
            let east = d
                .store_cards
                .iter()
                .find(|c| c.store.name == "城东店")
                .unwrap();
            assert_eq!(east.trend.len(), 14);
            assert_eq!(*east.trend.last().unwrap(), 2);
            assert_eq!(*east.trend.first().unwrap(), 0);
            // 计划状态：P1 今日有消耗=投放中，P2 今日无消耗=已暂停
            let p1 = d.plans.iter().find(|p| p.promotion_id == "P1").unwrap();
            let p2 = d.plans.iter().find(|p| p.promotion_id == "P2").unwrap();
            assert_eq!(p1.state, "投放中");
            assert_eq!(p2.state, "已暂停");
            Ok(())
        })
        .unwrap();
    }
}
