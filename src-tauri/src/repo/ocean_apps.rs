//! 巨量引擎应用仓储：多应用 CRUD。

use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::models::OceanApp;

/// 列出全部应用（含 Secret，供授权与调度刷新使用；勿直接下发前端）
pub fn list(conn: &Connection) -> AppResult<Vec<OceanApp>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, app_id, app_secret, token_expires_at
         FROM ocean_apps ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(OceanApp {
                id: row.get(0)?,
                name: row.get(1)?,
                app_id: row.get(2)?,
                app_secret: row.get(3)?,
                token_expires_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<OceanApp> {
    conn.query_row(
        "SELECT id, name, app_id, app_secret, token_expires_at
         FROM ocean_apps WHERE id = ?1",
        [id],
        |row| {
            Ok(OceanApp {
                id: row.get(0)?,
                name: row.get(1)?,
                app_id: row.get(2)?,
                app_secret: row.get(3)?,
                token_expires_at: row.get(4)?,
            })
        },
    )
    .map_err(|e| AppError::Message(format!("巨量应用不存在: {e}")))
}

/// 同 App ID 视为重复（UNIQUE 约束的友好化）
pub fn insert(
    conn: &Connection,
    name: &str,
    app_id: &str,
    app_secret: &str,
) -> AppResult<OceanApp> {
    let dup: Option<i64> = conn
        .query_row(
            "SELECT id FROM ocean_apps WHERE app_id = ?1",
            [app_id],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(AppError::Db(other)),
        })?;
    if dup.is_some() {
        return Err(AppError::Message(format!("App ID {app_id} 已存在")));
    }
    conn.execute(
        "INSERT INTO ocean_apps (name, app_id, app_secret, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, app_id, app_secret, crate::db::now()],
    )?;
    get(conn, conn.last_insert_rowid())
}

/// 删除应用并解除其广告账户归属（报表数据保留，仅不再刷新）
pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE ad_accounts SET ocean_app_id = NULL WHERE ocean_app_id = ?1",
        [id],
    )?;
    conn.execute("DELETE FROM ocean_apps WHERE id = ?1", [id])?;
    Ok(())
}

/// 授权成功后记录 token 过期时间
pub fn set_token_expires(conn: &Connection, id: i64, expires_at: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE ocean_apps SET token_expires_at = ?1 WHERE id = ?2",
        rusqlite::params![expires_at, id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn insert_list_delete_roundtrip_and_dup_rejected() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            let a = insert(conn, "主应用", "1700000001", "sec-a")?;
            insert(conn, "备用", "1700000002", "sec-b")?;
            assert_eq!(list(conn)?.len(), 2);
            // 同 App ID 拒绝
            assert!(insert(conn, "再录", "1700000001", "sec-x").is_err());
            // 删除解除账户归属
            crate::repo::leads::attach_ad_account(conn, "17001", "", a.id)?;
            delete(conn, a.id)?;
            let accounts = crate::repo::leads::ad_accounts(conn)?;
            assert_eq!(accounts.len(), 1);
            assert_eq!(accounts[0].ocean_app_id, None);
            assert_eq!(list(conn)?.len(), 1);
            Ok(())
        })
        .unwrap();
    }
}
