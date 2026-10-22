//! 线索与报表仓储：入库统一走 upsert，幂等由 ON CONFLICT 保证。

use rusqlite::{Connection, OptionalExtension};

use crate::error::AppResult;
use crate::models::{AdReportRow, LeadRow, PlanReportRow};

/// 按线索 ID 批量 upsert（同一门店口径）；返回写入行数。
/// 四个扩展维度（城市/流量/转化/场景）空值不覆盖库内现值——导出快照可能缺列。
pub fn upsert_leads(conn: &Connection, store_id: i64, rows: &[LeadRow]) -> AppResult<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for r in rows {
        n += tx.execute(
            "INSERT INTO leads (leads_id, store_id, phone, name, status_key, status_label,
                                source_label, intention, leads_time, feiyu_leads_id,
                                ad_id, promotion_id, promotion_name, city, traffic_type,
                                conversion_status, scene, raw)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
             ON CONFLICT(leads_id) DO UPDATE SET
                phone = excluded.phone,
                name = excluded.name,
                -- 空状态 = 本次来源不带状态（导出快照无状态列），保留库内现值
                status_key = CASE WHEN excluded.status_key = '' THEN leads.status_key ELSE excluded.status_key END,
                status_label = CASE WHEN excluded.status_label = '' THEN leads.status_label ELSE excluded.status_label END,
                source_label = excluded.source_label,
                intention = excluded.intention,
                leads_time = excluded.leads_time,
                feiyu_leads_id = excluded.feiyu_leads_id,
                ad_id = excluded.ad_id,
                promotion_id = excluded.promotion_id,
                promotion_name = excluded.promotion_name,
                city = CASE WHEN excluded.city = '' THEN leads.city ELSE excluded.city END,
                traffic_type = CASE WHEN excluded.traffic_type = '' THEN leads.traffic_type ELSE excluded.traffic_type END,
                conversion_status = CASE WHEN excluded.conversion_status = '' THEN leads.conversion_status ELSE excluded.conversion_status END,
                scene = CASE WHEN excluded.scene = '' THEN leads.scene ELSE excluded.scene END,
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
                r.feiyu_leads_id,
                r.ad_id,
                r.promotion_id,
                r.promotion_name,
                r.city,
                r.traffic_type,
                r.conversion_status,
                r.scene,
                r.raw.to_string()
            ],
        )?;
    }
    tx.commit()?;
    Ok(n)
}

/// 快照合并键：导出表的线索 ID 即列表接口的 feiyu_leads_id（issue #12 口径），
/// 命中则沿用列表主键，避免广告线索双份
pub fn find_leads_id_by_feiyu(
    conn: &Connection,
    feiyu_leads_id: &str,
) -> AppResult<Option<String>> {
    let mut stmt = conn.prepare("SELECT leads_id FROM leads WHERE feiyu_leads_id = ?1")?;
    let found = stmt
        .query_row([feiyu_leads_id], |r| r.get::<_, String>(0))
        .optional()?;
    Ok(found)
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
            "INSERT INTO plan_reports_daily (ad_account_id, promotion_id, promotion_name, date, spend, conversions, impressions)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(ad_account_id, promotion_id, date) DO UPDATE SET
                promotion_name = excluded.promotion_name,
                spend = excluded.spend,
                conversions = excluded.conversions,
                impressions = excluded.impressions",
            rusqlite::params![
                r.ad_account_id,
                r.promotion_id,
                r.promotion_name,
                r.date,
                r.spend,
                r.conversions,
                r.impressions
            ],
        )?;
    }
    tx.commit()?;
    Ok(n)
}

/// 确保广告账户存在（报表入库时补档，不带应用归属）
pub fn ensure_ad_account(conn: &Connection, id: &str, alias: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ad_accounts (id, alias, created_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(id) DO NOTHING",
        rusqlite::params![id, alias, crate::db::now()],
    )?;
    Ok(())
}

/// OAuth 授权补档并绑定归属应用；同一账户再次在其他应用下授权时改判归属
pub fn attach_ad_account(
    conn: &Connection,
    id: &str,
    alias: &str,
    ocean_app_id: i64,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ad_accounts (id, alias, ocean_app_id, created_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            ocean_app_id = excluded.ocean_app_id,
            alias = CASE WHEN excluded.alias = '' THEN ad_accounts.alias ELSE excluded.alias END",
        rusqlite::params![id, alias, ocean_app_id, crate::db::now()],
    )?;
    Ok(())
}

/// 全部广告账户（映射表展示，带归属应用名）
pub fn ad_accounts(conn: &Connection) -> AppResult<Vec<crate::models::AdAccount>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.alias, a.ocean_app_id, o.name
         FROM ad_accounts a LEFT JOIN ocean_apps o ON o.id = a.ocean_app_id
         ORDER BY a.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(crate::models::AdAccount {
                id: r.get(0)?,
                alias: r.get(1)?,
                ocean_app_id: r.get(2)?,
                app_name: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 某应用名下的广告账户 ID（报表拉取范围）
pub fn ad_account_ids_for_app(conn: &Connection, ocean_app_id: i64) -> AppResult<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT id FROM ad_accounts WHERE ocean_app_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([ocean_app_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
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
            let label: String = conn.query_row(
                "SELECT status_label FROM leads WHERE leads_id = 'L1'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(label, "已联系");
            Ok(())
        })
        .unwrap();
    }
}
