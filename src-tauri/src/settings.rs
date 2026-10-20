//! 应用级设置（settings 键值表）：巨量引擎应用凭证等。

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

const KEY_APP_ID: &str = "ocean.app_id";
const KEY_APP_SECRET: &str = "ocean.app_secret";
/// 对账快照触发小时（0-23），设置页可改
pub const KEY_SNAPSHOT_HOUR: &str = "snapshot.hour";
/// 巨量 OAuth token 过期时间（unix 秒）
pub const KEY_OCEAN_TOKEN_EXPIRES: &str = "ocean.token.expires_at";

/// 巨量引擎开放平台应用凭证（设置页维护；None 表示从未保存过）
#[derive(Debug, Serialize)]
pub struct OceanCredentials {
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
}

pub fn get_ocean_credentials(conn: &Connection) -> rusqlite::Result<OceanCredentials> {
    let read = |key: &str| -> rusqlite::Result<Option<String>> {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0)).optional()
    };
    Ok(OceanCredentials {
        app_id: read(KEY_APP_ID)?,
        app_secret: read(KEY_APP_SECRET)?,
    })
}

/// 成对 upsert 两项凭证；事务保证读侧看到的始终是同一组配置
pub fn save_ocean_credentials(conn: &Connection, app_id: &str, app_secret: &str) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (key, value) in [(KEY_APP_ID, app_id), (KEY_APP_SECRET, app_secret)] {
        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
    }
    tx.commit()
}

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0)).optional()
}

pub fn save_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn save_then_read_back_and_overwrite() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            save_ocean_credentials(conn, "id-1", "sec-1").unwrap();
            save_ocean_credentials(conn, "id-2", "sec-2").unwrap();
            let got = get_ocean_credentials(conn).unwrap();
            assert_eq!(got.app_id.as_deref(), Some("id-2"));
            assert_eq!(got.app_secret.as_deref(), Some("sec-2"));
            save_setting(conn, KEY_SNAPSHOT_HOUR, "23").unwrap();
            assert_eq!(get_setting(conn, KEY_SNAPSHOT_HOUR).unwrap().as_deref(), Some("23"));
            Ok(())
        })
        .unwrap();
    }
}
