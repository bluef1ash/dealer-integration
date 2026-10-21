//! 每日导出对账快照（issue #12）：巨懂车导出 xlsx → LeadRow → 按线索 ID 合并。
//! 纠偏范围 = 导出表实际携带的字段（手机号/意向车型/广告归因/缺失线索补全）；
//! 线索状态不在导出列中，其纠偏由每小时 7 天窗口重拉承担（见 ADR-0003）。
//!
//! 字段口径（真实导出表实测，issue #3 spike）：导出「广告ID」= 列表接口 extra.promotion_id。

use std::collections::HashMap;
use std::path::Path;

use chrono::TimeZone;
use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::models::LeadRow;

/// 解析巨懂车导出 xlsx 为领域行（表头按名对位，列顺序变化不敏感）
pub fn read_judongche_export(path: &Path) -> AppResult<Vec<LeadRow>> {
    let book = calamine::open_workbook_auto(path)
        .map_err(|e| AppError::Excel(format!("打开导出文件失败: {e}")))?;
    let range = {
        use calamine::Reader;
        let mut book = book;
        book.worksheet_range_at(0)
            .ok_or_else(|| AppError::Excel("导出文件无工作表".into()))?
            .map_err(|e| AppError::Excel(format!("读取工作表失败: {e}")))?
    };

    let mut rows_iter = range.rows();
    let header = rows_iter
        .next()
        .ok_or_else(|| AppError::Excel("导出文件为空".into()))?;
    let col_of: HashMap<&str, usize> = [
        "线索ID",
        "客户手机号",
        "客户姓名",
        "留资时间",
        "线索类型",
        "意向车型",
        "广告ID",
        "广告名称",
    ]
    .into_iter()
    .filter_map(|name| header.iter().position(|c| c == name).map(|idx| (name, idx)))
    .collect();
    let leads_id_col = *col_of
        .get("线索ID")
        .ok_or_else(|| AppError::Excel("导出缺少「线索ID」列".into()))?;

    let mut out = Vec::new();
    for row in rows_iter {
        let cell = |idx: usize| -> String { row.get(idx).map(cell_str).unwrap_or_default() };
        let leads_id = cell(leads_id_col);
        if leads_id.is_empty() {
            continue;
        }
        out.push(LeadRow {
            // 导出表的线索 ID 即飞鱼 ID（跨表合并键）；主键还原在 reconcile 中做
            feiyu_leads_id: leads_id.clone(),
            leads_id,
            phone: cell(*col_of.get("客户手机号").unwrap_or(&usize::MAX)),
            name: cell(*col_of.get("客户姓名").unwrap_or(&usize::MAX)),
            status_key: String::new(),
            status_label: String::new(),
            source_label: cell(*col_of.get("线索类型").unwrap_or(&usize::MAX)),
            intention: cell(*col_of.get("意向车型").unwrap_or(&usize::MAX)),
            leads_time: cell(*col_of.get("留资时间").unwrap_or(&usize::MAX)).parse_leads_time(),
            ad_id: String::new(),
            promotion_id: cell(*col_of.get("广告ID").unwrap_or(&usize::MAX)),
            promotion_name: cell(*col_of.get("广告名称").unwrap_or(&usize::MAX)),
            raw: serde_json::json!({ "from": "export_snapshot" }),
        });
    }
    Ok(out)
}

fn cell_str(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Empty => String::new(),
        // 仅数值型单元格转整型串；字符串单元格原样保留（19 位 ID 走 f64 会丢精度）
        calamine::Data::Float(f) if f.fract() == 0.0 => format!("{}", *f as i64),
        other => other.to_string(),
    }
}

trait ParseLeadsTime {
    fn parse_leads_time(self) -> i64;
}

impl ParseLeadsTime for String {
    fn parse_leads_time(self) -> i64 {
        chrono::NaiveDateTime::parse_from_str(&self, "%Y-%m-%d %H:%M:%S")
            .ok()
            .and_then(|ndt| chrono::Local.from_local_datetime(&ndt).single())
            .map(|t| t.timestamp())
            .unwrap_or(0)
    }
}

/// 今日是否已有快照登记（调度器的每日守卫）
pub fn has_snapshot_for(conn: &Connection, date: &str) -> AppResult<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM snapshots WHERE date = ?1",
        [date],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReconcileStats {
    pub seen: usize,
    pub upserted: usize,
}

/// 执行一次对账：合并快照行并登记快照记录（一天一条，重跑覆盖）。
/// 导出行先按飞鱼 ID 找回列表主键再合并；找不到的作为新线索以飞鱼 ID 入库。
pub fn reconcile(conn: &Connection, store_id: i64, path: &Path) -> AppResult<ReconcileStats> {
    let rows = read_judongche_export(path)?;
    let seen = rows.len();
    let mut remapped = rows;
    for row in &mut remapped {
        if let Some(existing) =
            crate::repo::leads::find_leads_id_by_feiyu(conn, &row.feiyu_leads_id)?
        {
            row.leads_id = existing;
        }
    }
    let upserted = crate::repo::leads::upsert_leads(conn, store_id, &remapped)?;
    conn.execute(
        "INSERT INTO snapshots (date, imported_at, leads_seen) VALUES (?1, ?2, ?3)
         ON CONFLICT(date) DO UPDATE SET imported_at = excluded.imported_at, leads_seen = excluded.leads_seen",
        rusqlite::params![chrono::Local::now().date_naive().to_string(), crate::db::now(), seen as i64],
    )?;
    Ok(ReconcileStats { seen, upserted })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn fixture(path: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/judongche"
        ))
        .join(path)
    }

    /// 真实导出夹具可解析：130 行数据、脱敏手机号、计划归因对上列表口径
    /// （接口 total 131 与导出 130 的差是两次采集间的数据漂移，快照口径以文件为准）
    #[test]
    fn real_export_fixture_parses() {
        let rows = read_judongche_export(&fixture("leads_export.xlsx")).unwrap();
        assert_eq!(rows.len(), 130);
        assert!(rows.iter().all(|r| !r.leads_id.is_empty()));
        assert!(rows.iter().all(|r| r.phone.starts_with("1380000")));
        assert!(rows.iter().all(|r| r.leads_time > 0));
        assert!(rows.iter().any(|r| r.promotion_id == "7691306192144973887"));
    }

    /// 纠偏：库内被改错的手机号被快照纠正，新线索补全，不产生重复
    #[test]
    fn reconcile_corrects_and_fills_without_duplicates() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            crate::repo::stores::insert(conn, "城东店", "13800009999")?;
            let body = std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/judongche/leads_list_page1.json"
            ))?;
            crate::ingest::ingest_list_response(conn, 1, &body)?;
            // 模拟增量拉取后字段漂移：手机号被写错、一条线索缺失
            conn.execute(
                "UPDATE leads SET phone = '99999999999' WHERE leads_id = '1115047139'",
                [],
            )?;
            conn.execute("DELETE FROM leads WHERE leads_id = '1109298389'", [])?;

            // 构造差异夹具：含 1115047139 的正确手机号 + 一条新线索
            let path = std::env::temp_dir().join("dealer-integration-reconcile-test.xlsx");
            write_diff_fixture(&path);

            let stats = reconcile(conn, 1, &path)?;
            assert_eq!(stats.seen, 2);
            let phone: String = conn.query_row(
                "SELECT phone FROM leads WHERE leads_id = '1115047139'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(phone, "13800001001");
            // 导出行无状态列，合并不得抹掉库内状态（spec：快照纠偏而非倒退）
            let status: String = conn.query_row(
                "SELECT status_label FROM leads WHERE leads_id = '1115047139'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(status, "未联系");
            let total: i64 = conn.query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))?;
            assert_eq!(total, 10); // 9 存量（10 - 1 漂移缺失）+ 1 新线索
            let snaps: i64 = conn.query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))?;
            assert_eq!(snaps, 1);
            // 快照重跑：仍不重复
            reconcile(conn, 1, &path)?;
            let total2: i64 = conn.query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))?;
            assert_eq!(total2, 10);
            let _ = std::fs::remove_file(&path);
            Ok(())
        })
        .unwrap();
    }

    /// 与真实导出表同表头的两行差异夹具
    fn write_diff_fixture(path: &std::path::Path) {
        use rust_xlsxwriter::Workbook;
        let mut book = Workbook::new();
        let sheet = book.add_worksheet();
        let headers = [
            "客户姓名",
            "客户手机号",
            "客户微信号",
            "抖音昵称",
            "抖音id",
            "询价城市",
            "意向车型",
            "留资时间",
            "流量类型",
            "线索类型",
            "转化状态",
            "互动类型",
            "一级来源场景",
            "二级来源场景",
            "来源抖音UID",
            "来源抖音号",
            "来源抖音昵称",
            "账号类型",
            "跟进商家抖音号",
            "商家抖音ID",
            "商家抖音昵称",
            "商家ID",
            "商家名称",
            "广告组件ID",
            "组件ID",
            "组件名称",
            "广告主ID",
            "广告主名称",
            "广告ID",
            "广告名称",
            "线索ID",
            "线索推送ID",
        ];
        for (c, h) in headers.iter().enumerate() {
            sheet.write(0, c as u16, *h).unwrap();
        }
        // 行 1：纠正 1115047139 的手机号（线索 ID 列填飞鱼 ID，与真实导出一致）
        let row1 = [
            "未命名",
            "13800001001",
            "",
            "",
            "",
            "辽宁/鞍山",
            "iCAR超级V23",
            "2026-10-14 10:08:35",
            "外部流量",
            "私聊线索",
            "外部流量",
            "私信",
            "直播",
            "直播进私留资",
            "",
            "",
            "",
            "企业主账号",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "7696340964679352339",
            "",
        ];
        for (c, v) in row1.iter().enumerate() {
            sheet.write(1, c as u16, *v).unwrap();
        }
        // 行 2：新线索（快照补全）
        let row2 = [
            "未命名",
            "13800007777",
            "",
            "",
            "",
            "辽宁/大连",
            "iCAR超级V23",
            "2026-10-15 09:00:00",
            "合法转化",
            "广告直接线索",
            "合法转化",
            "私信",
            "短视频",
            "短视频进私留资",
            "",
            "",
            "",
            "企业主账号",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "7691306192144973887",
            "示例计划",
            "9990000000999000001",
            "",
        ];
        for (c, v) in row2.iter().enumerate() {
            sheet.write(2, c as u16, *v).unwrap();
        }
        book.save(path).unwrap();
    }
}
