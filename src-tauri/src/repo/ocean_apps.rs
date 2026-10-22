//! 巨量引擎应用仓储：多应用 CRUD 与旧版单应用凭证（settings 键）的启动迁移。

use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::models::OceanApp;

/// 旧版单应用的 settings 键（多应用化前的存储位；迁移成功后即清除）
pub const LEGACY_KEY_APP_ID: &str = "ocean.app_id";
pub const LEGACY_KEY_APP_SECRET: &str = "ocean.app_secret";

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

/// 旧版单应用凭证迁移（幂等，纯数据库步骤）：
/// settings 里还有 ocean.app_id 键且应用表为空时，迁入首行「默认应用」并回填存量账户归属；
/// 返回迁移来源的 App ID（供调用方接续迁移凭证库旧 token、清除旧键），无旧键时返回 None。
/// 旧键不清除前每次启动都会走到这里（重复启动只会补齐缺口，不重复插入）。
pub fn migrate_legacy_db(conn: &Connection) -> AppResult<Option<String>> {
    let read = |key: &str| -> AppResult<Option<String>> {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(AppError::Db(other)),
        })
    };
    let Some(app_id) = read(LEGACY_KEY_APP_ID)? else {
        return Ok(None);
    };
    let Some(secret) = read(LEGACY_KEY_APP_SECRET)? else {
        return Ok(None);
    };
    if list(conn)?.is_empty() {
        let app = insert(conn, "默认应用", app_id.trim(), secret.trim())?;
        conn.execute(
            "UPDATE ad_accounts SET ocean_app_id = ?1 WHERE ocean_app_id IS NULL",
            [app.id],
        )?;
    }
    Ok(Some(app_id))
}

/// 迁移收尾：消费掉旧 settings 键，防止用户删除应用后重启又被迁回来
pub fn clear_legacy_keys(conn: &Connection) -> AppResult<()> {
    for key in [LEGACY_KEY_APP_ID, LEGACY_KEY_APP_SECRET] {
        conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
    }
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

    #[test]
    fn legacy_migration_backfills_once_and_clears_on_demand() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            // 无旧键：不迁移
            assert_eq!(migrate_legacy_db(conn)?, None);

            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('ocean.app_id', '1700000009')",
                [],
            )?;
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('ocean.app_secret', 'sec-old')",
                [],
            )?;
            // 存量账户应被回填归属
            crate::repo::leads::ensure_ad_account(conn, "17009", "")?;

            let app_id = migrate_legacy_db(conn)?.expect("应识别出旧键");
            assert_eq!(app_id, "1700000009");
            let apps = list(conn)?;
            assert_eq!(apps.len(), 1);
            assert_eq!(apps[0].name, "默认应用");
            assert_eq!(apps[0].app_id, "1700000009");
            assert_eq!(apps[0].app_secret, "sec-old");
            let accounts = crate::repo::leads::ad_accounts(conn)?;
            assert_eq!(accounts[0].ocean_app_id, Some(apps[0].id));

            // 旧键未清时重复迁移：不重复插入（幂等）
            assert!(migrate_legacy_db(conn)?.is_some());
            assert_eq!(list(conn)?.len(), 1);

            clear_legacy_keys(conn)?;
            assert_eq!(migrate_legacy_db(conn)?, None);
            Ok(())
        })
        .unwrap();
    }
}
