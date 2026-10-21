//! 门店与采集状态仓储。

use rusqlite::Connection;

use crate::error::AppResult;
use crate::models::{PullState, Store};

pub fn insert(conn: &Connection, name: &str, account: &str) -> AppResult<Store> {
    conn.execute(
        "INSERT INTO stores (name, account, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![name, account, crate::db::now()],
    )?;
    Ok(Store {
        id: conn.last_insert_rowid(),
        name: name.into(),
        account: account.into(),
    })
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    // 门店删除级联清理映射与采集状态；线索保留（历史数据不随账号删除）
    conn.execute("DELETE FROM store_ad_accounts WHERE store_id = ?1", [id])?;
    conn.execute("DELETE FROM store_pull_state WHERE store_id = ?1", [id])?;
    conn.execute("DELETE FROM stores WHERE id = ?1", [id])?;
    Ok(())
}

pub fn list(conn: &Connection) -> AppResult<Vec<Store>> {
    let mut stmt = conn.prepare("SELECT id, name, account FROM stores ORDER BY id")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Store {
                id: row.get(0)?,
                name: row.get(1)?,
                account: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Store> {
    conn.query_row(
        "SELECT id, name, account FROM stores WHERE id = ?1",
        [id],
        |row| {
            Ok(Store {
                id: row.get(0)?,
                name: row.get(1)?,
                account: row.get(2)?,
            })
        },
    )
    .map_err(|e| crate::error::AppError::Message(format!("门店不存在: {e}")))
}

pub fn pull_state(conn: &Connection, store_id: i64) -> AppResult<PullState> {
    conn.query_row(
        "SELECT store_id, last_success_at, last_attempt_at, last_error, backfilled
         FROM store_pull_state WHERE store_id = ?1",
        [store_id],
        |row| {
            Ok(PullState {
                store_id: row.get(0)?,
                last_success_at: row.get(1)?,
                last_attempt_at: row.get(2)?,
                last_error: row.get(3)?,
                backfilled: row.get::<_, i64>(4)? != 0,
            })
        },
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(PullState {
            store_id,
            ..Default::default()
        }),
        other => Err(crate::error::AppError::Db(other)),
    })
}

pub fn pull_states(conn: &Connection) -> AppResult<Vec<PullState>> {
    let mut stmt = conn.prepare(
        "SELECT store_id, last_success_at, last_attempt_at, last_error, backfilled FROM store_pull_state",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PullState {
                store_id: row.get(0)?,
                last_success_at: row.get(1)?,
                last_attempt_at: row.get(2)?,
                last_error: row.get(3)?,
                backfilled: row.get::<_, i64>(4)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 记录一次拉取结果：成功刷新 last_success_at 并清空错误；失败只记错误
pub fn record_pull(
    conn: &Connection,
    store_id: i64,
    ok: bool,
    error: &str,
    now_ts: i64,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO store_pull_state (store_id, last_success_at, last_attempt_at, last_error, backfilled)
         VALUES (?1, ?2, ?3, ?4, 0)
         ON CONFLICT(store_id) DO UPDATE SET
            last_attempt_at = excluded.last_attempt_at,
            last_success_at = CASE WHEN ?5 THEN excluded.last_success_at ELSE store_pull_state.last_success_at END,
            last_error = CASE WHEN ?5 THEN '' ELSE excluded.last_error END",
        rusqlite::params![store_id, if ok { Some(now_ts) } else { None }, now_ts, error, ok],
    )?;
    Ok(())
}

pub fn mark_backfilled(conn: &Connection, store_id: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO store_pull_state (store_id, backfilled) VALUES (?1, 1)
         ON CONFLICT(store_id) DO UPDATE SET backfilled = 1",
        [store_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn add_list_delete_roundtrip() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            let s = insert(conn, "城东店", "13800000001")?;
            insert(conn, "城西店", "13800000002")?;
            assert_eq!(list(conn)?.len(), 2);
            delete(conn, s.id)?;
            assert_eq!(list(conn)?[0].account, "13800000002");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn pull_state_success_then_failure_keeps_last_success() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            let s = insert(conn, "城东店", "13800000001")?;
            record_pull(conn, s.id, true, "", 100)?;
            record_pull(conn, s.id, false, "会话失效", 200)?;
            let st = pull_state(conn, s.id)?;
            assert_eq!(st.last_success_at, Some(100));
            assert_eq!(st.last_attempt_at, Some(200));
            assert_eq!(st.last_error, "会话失效");
            record_pull(conn, s.id, true, "", 300)?;
            let st = pull_state(conn, s.id)?;
            assert_eq!(st.last_success_at, Some(300));
            assert_eq!(st.last_error, "");
            Ok(())
        })
        .unwrap();
    }
}

/// 门店 ↔ 广告账户映射（多对多）
pub fn set_mapping(
    conn: &Connection,
    store_id: i64,
    ad_account_id: &str,
    enable: bool,
) -> AppResult<()> {
    if enable {
        conn.execute(
            "INSERT OR IGNORE INTO store_ad_accounts (store_id, ad_account_id) VALUES (?1, ?2)",
            rusqlite::params![store_id, ad_account_id],
        )?;
    } else {
        conn.execute(
            "DELETE FROM store_ad_accounts WHERE store_id = ?1 AND ad_account_id = ?2",
            rusqlite::params![store_id, ad_account_id],
        )?;
    }
    Ok(())
}

pub fn mapping_account_ids(conn: &Connection, store_id: i64) -> AppResult<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT ad_account_id FROM store_ad_accounts WHERE store_id = ?1")?;
    let rows = stmt
        .query_map([store_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
