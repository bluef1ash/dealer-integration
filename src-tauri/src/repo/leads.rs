//! 线索与报表仓储：入库统一走 upsert，幂等由 ON CONFLICT 保证。

use rusqlite::Connection;

use crate::error::AppResult;
use crate::models::{AdReportRow, LeadRow, PlanReportRow};

/// 按线索 ID 批量 upsert（同一门店口径）；返回写入行数
pub fn upsert_leads(conn: &Connection, store_id: i64, rows: &[LeadRow]) -> AppResult<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for r in rows {
        n += tx.execute(
            "INSERT INTO leads (leads_id, store_id, phone, name, status_key, status_label,
                                source_label, intention, leads_time, ad_id, promotion_id, promotion_name, raw)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(leads_id) DO UPDATE SET
                phone = excluded.phone,
                name = excluded.name,
                status_key = excluded.status_key,
                status_label = excluded.status_label,
                source_label = excluded.source_label,
                intention = excluded.intention,
                leads_time = excluded.leads_time,
                ad_id = excluded.ad_id,
                promotion_id = excluded.promotion_id,
                promotion_name = excluded.promotion_name,
                raw = excluded.raw",
            rusqlite::params![
                r.leads_id,
                store_id,
                r.phone,
                r.name,
                r.status_key,
                r.status_label,
                r.source_label,
                r.intention,
                r.leads_time,
                r.ad_id,
                r.promotion_id,
                r.promotion_name,
                r.raw.to_string()
            ],
        )?;
    }
    tx.commit()?;
    Ok(n)
}

pub fn upsert_ad_reports(conn: &Connection, rows: &[AdReportRow]) -> AppResult<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for r in rows {
        n += tx.execute(
            "INSERT INTO ad_reports_daily (ad_account_id, date, spend, impressions, clicks, conversions)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(ad_account_id, date) DO UPDATE SET
                spend = excluded.spend,
                impressions = excluded.impressions,
                clicks = excluded.clicks,
                conversions = excluded.conversions",
            rusqlite::params![r.ad_account_id, r.date, r.spend, r.impressions, r.clicks, r.conversions],
        )?;
    }
    tx.commit()?;
    Ok(n)
}

pub fn upsert_plan_reports(conn: &Connection, rows: &[PlanReportRow]) -> AppResult<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for r in rows {
        n += tx.execute(
            "INSERT INTO plan_reports_daily (ad_account_id, promotion_id, promotion_name, date, spend, conversions)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(ad_account_id, promotion_id, date) DO UPDATE SET
                promotion_name = excluded.promotion_name,
                spend = excluded.spend,
                conversions = excluded.conversions",
            rusqlite::params![r.ad_account_id, r.promotion_id, r.promotion_name, r.date, r.spend, r.conversions],
        )?;
    }
    tx.commit()?;
    Ok(n)
}

/// 确保广告账户存在（OAuth 授权/报表入库时补档）
pub fn ensure_ad_account(conn: &Connection, id: &str, alias: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ad_accounts (id, alias, created_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(id) DO NOTHING",
        rusqlite::params![id, alias, crate::db::now()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn lead(id: &str, status: &str) -> LeadRow {
        LeadRow {
            leads_id: id.into(),
            status_label: status.into(),
            leads_time: 100,
            ..Default::default()
        }
    }

    /// 同一线索重复写入以新值为准且不产生重复行（幂等语义）
    #[test]
    fn upsert_leads_is_idempotent() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            crate::repo::stores::insert(conn, "城东店", "13800000001")?;
            upsert_leads(conn, 1, &[lead("L1", "未联系")])?;
            upsert_leads(conn, 1, &[lead("L1", "已联系"), lead("L2", "未联系")])?;
            let n: i64 = conn.query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))?;
            assert_eq!(n, 2);
            let label: String =
                conn.query_row("SELECT status_label FROM leads WHERE leads_id = 'L1'", [], |r| r.get(0))?;
            assert_eq!(label, "已联系");
            Ok(())
        })
        .unwrap();
    }
}
