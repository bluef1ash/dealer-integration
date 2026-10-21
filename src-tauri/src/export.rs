//! Excel 导出（出库接缝的一部分）：与明细页同筛选函数，保证「导出与所见一致」。

use chrono::TimeZone;
use rusqlite::Connection;

use crate::error::AppResult;
use crate::models::LeadsFilter;

/// 按筛选导出线索明细为 xlsx；返回导出行数
pub fn export_leads(
    conn: &Connection,
    filter: &LeadsFilter,
    path: &std::path::Path,
) -> AppResult<usize> {
    // page_size=0 即不分页：导出与明细页共用同一筛选实现
    let page = crate::query::leads_page(
        conn,
        &LeadsFilter {
            page: 0,
            page_size: 0,
            ..filter.clone()
        },
    )?;

    let mut book = rust_xlsxwriter::Workbook::new();
    let sheet = book.add_worksheet();
    sheet
        .set_name("线索明细")
        .map_err(|e| crate::error::AppError::Excel(e.to_string()))?;
    let headers = [
        "线索ID",
        "留资时间",
        "门店",
        "意向车型",
        "手机号",
        "客户姓名",
        "状态",
        "来源",
        "计划名称",
    ];
    for (col, h) in headers.iter().enumerate() {
        sheet
            .write(0, col as u16, *h)
            .map_err(|e| crate::error::AppError::Excel(e.to_string()))?;
    }
    for (i, r) in page.rows.iter().enumerate() {
        let time = chrono::Local
            .timestamp_opt(r.leads_time, 0)
            .single()
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
        let row = (i + 1) as u32;
        let values: [String; 9] = [
            r.leads_id.clone(),
            time,
            r.store_name.clone(),
            r.intention.clone(),
            r.phone.clone(),
            r.name.clone(),
            r.status_label.clone(),
            r.source_label.clone(),
            r.promotion_name.clone(),
        ];
        for (col, v) in values.iter().enumerate() {
            sheet
                .write(row, col as u16, v)
                .map_err(|e| crate::error::AppError::Excel(e.to_string()))?;
        }
    }
    book.save(path)
        .map_err(|e| crate::error::AppError::Excel(e.to_string()))?;
    Ok(page.rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::models::LeadRow;

    fn lead(id: &str, store: i64, time: i64, status: &str) -> LeadRow {
        LeadRow {
            leads_id: id.into(),
            phone: format!("13800012{n:03}", n = id.len()),
            status_label: status.into(),
            leads_time: time,
            ..Default::default()
        }
    }

    #[test]
    fn exported_file_matches_filter() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            crate::repo::stores::insert(conn, "城东店", "13800000001")?;
            crate::repo::stores::insert(conn, "城西店", "13800000002")?;
            crate::repo::leads::upsert_leads(
                conn,
                1,
                &[lead("L1", 1, 1000, "未联系"), lead("L2", 1, 2000, "已成交")],
            )?;
            crate::repo::leads::upsert_leads(conn, 2, &[lead("L4", 2, 500, "未联系")])?;
            Ok(())
        })
        .unwrap();

        let path = std::env::temp_dir().join("dealer-integration-export-test.xlsx");
        let n = db
            .with(|conn| {
                export_leads(
                    conn,
                    &LeadsFilter {
                        status: Some("未联系".into()),
                        page_size: 0,
                        ..Default::default()
                    },
                    &path,
                )
            })
            .unwrap();
        assert_eq!(n, 2);

        let mut book = calamine::open_workbook_auto(&path).unwrap();
        use calamine::Reader;
        let range = book.worksheet_range_at(0).unwrap().unwrap();
        let rows: Vec<_> = range.rows().collect();
        assert_eq!(rows[0][0], "线索ID");
        assert_eq!(rows.len(), 3); // 表头 + 2 行（L1、L4）
        assert!(rows.iter().skip(1).all(|r| r[6] == "未联系"));
        let _ = std::fs::remove_file(&path);
    }
}
