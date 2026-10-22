//! 入库接缝（spec #1 测试决策）：原始响应文本 → 解析 → 按线索 ID upsert。
//! 测试断言库内状态；同一夹具跑两遍库不变（幂等）。

use rusqlite::Connection;

use crate::error::AppResult;

#[derive(Debug, PartialEq, Eq)]
pub struct IngestStats {
    pub seen: usize,
    pub upserted: usize,
}

/// 巨懂车「线索管理」列表接口响应入库
pub fn ingest_list_response(
    conn: &Connection,
    store_id: i64,
    body: &str,
) -> AppResult<IngestStats> {
    let rows = crate::judongche::parse::lead_rows_from_list(body)?;
    let upserted = crate::repo::leads::upsert_leads(conn, store_id, &rows)?;
    Ok(IngestStats {
        seen: rows.len(),
        upserted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::error::AppError;
    use crate::models::LeadRow;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/judongche/leads_list_page1.json"
    );

    fn fixture_body() -> String {
        std::fs::read_to_string(FIXTURE).expect("读取列表夹具")
    }

    fn setup_db() -> Db {
        let db = Db::open_in_memory();
        db.with(|conn| crate::repo::stores::insert(conn, "城东店", "13800009999").map(|_| ()))
            .unwrap();
        db
    }

    fn ingest(db: &Db) -> IngestStats {
        db.with(|conn| ingest_list_response(conn, 1, &fixture_body()))
            .unwrap()
    }

    /// 回放夹具：解析与字段映射正确，行数与关键列入库
    #[test]
    fn replay_fixture_parses_and_persists() {
        let db = setup_db();
        let stats = ingest(&db);
        assert_eq!(stats.seen, 10);
        assert_eq!(stats.upserted, 10);
        db.with(|conn| {
            let n: i64 = conn.query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))?;
            assert_eq!(n, 10);
            // 夹具首条线索（脱敏后），独立期望值来自夹具文件本身
            let (phone, status, source, time): (String, String, String, i64) = conn.query_row(
                "SELECT phone, status_label, source_label, leads_time FROM leads WHERE leads_id = '1115047139'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
            assert_eq!(phone, "13800001001");
            assert_eq!(status, "未联系");
            assert_eq!(source, "私聊线索");
            assert_eq!(time, 1791943715);
            // 广告直接线索行保留计划归因（真实业务 ID 在脱敏时保留）
            let (ad_id, promotion_id): (String, String) = conn.query_row(
                "SELECT ad_id, promotion_id FROM leads WHERE leads_id = '1109298389'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            assert_eq!(ad_id, "1877762578618523");
            assert_eq!(promotion_id, "7691306192144973887");
            Ok(())
        })
        .unwrap();
    }

    /// 幂等：同一夹具跑两遍，库内全部行完全不变
    #[test]
    fn replaying_same_fixture_twice_changes_nothing() {
        let db = setup_db();
        ingest(&db);
        let after_first = dump(&db);
        let stats = ingest(&db);
        assert_eq!(stats.seen, 10);
        assert_eq!(dump(&db), after_first);
    }

    fn dump(db: &Db) -> Vec<(String, String, String, i64)> {
        db.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT leads_id, phone, status_label, leads_time FROM leads ORDER BY leads_id",
            )?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Db)?;
            Ok::<_, AppError>(rows)
        })
        .unwrap()
    }

    /// 远端业务失败（code 非 0）在入库前即报错，不写库
    #[test]
    fn non_zero_code_is_rejected() {
        let db = Db::open_in_memory();
        let err = db
            .with(|conn| {
                crate::repo::stores::insert(conn, "城东店", "13800009999")?;
                ingest_list_response(conn, 1, r#"{"status":10014,"message":"账号授权过期"}"#)
            })
            .unwrap_err();
        assert!(err.to_string().contains("10014"));
    }

    /// 归一化检查：LeadRow 经 serde 往返不丢字段（供导出快照复用同一结构）
    #[test]
    fn lead_row_survives_serde_roundtrip() {
        let row = LeadRow {
            leads_id: "L1".into(),
            phone: "13800000000".into(),
            status_label: "未联系".into(),
            leads_time: 42,
            ..Default::default()
        };
        let text = serde_json::to_string(&row).unwrap();
        let back: LeadRow = serde_json::from_str(&text).unwrap();
        assert_eq!(back.leads_id, "L1");
        assert_eq!(back.leads_time, 42);
    }
}
