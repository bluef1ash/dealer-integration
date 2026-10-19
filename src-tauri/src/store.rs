//! 本地设置存储：SQLite `settings` 键值表，库文件位于应用数据目录。

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

const KEY_APP_ID: &str = "ocean.app_id";
const KEY_APP_SECRET: &str = "ocean.app_secret";

/// 巨量引擎开放平台应用凭证（设置页维护；None 表示从未保存过）
#[derive(Debug, Serialize)]
pub struct OceanCredentials {
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
}

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// 打开（不存在则创建）库文件并初始化表结构
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn get_ocean_credentials(&self) -> rusqlite::Result<OceanCredentials> {
        let conn = self.conn.lock().expect("settings 连接锁中毒");
        let read = |key: &str| -> rusqlite::Result<Option<String>> {
            conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0)).optional()
        };
        Ok(OceanCredentials {
            app_id: read(KEY_APP_ID)?,
            app_secret: read(KEY_APP_SECRET)?,
        })
    }

    /// 成对 upsert 两项凭证；事务保证读侧看到的始终是同一组配置
    pub fn save_ocean_credentials(&self, app_id: &str, app_secret: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("settings 连接锁中毒");
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_store() -> Store {
        Store::open(Path::new(":memory:")).expect("打开内存库")
    }

    /// 保存后可回读；重复保存以最后一次为准且不产生重复行（upsert 幂等）
    #[test]
    fn save_then_read_back_and_overwrite() {
        let store = mem_store();

        let empty = store.get_ocean_credentials().unwrap();
        assert!(empty.app_id.is_none() && empty.app_secret.is_none());

        store.save_ocean_credentials("id-1", "sec-1").unwrap();
        let got = store.get_ocean_credentials().unwrap();
        assert_eq!(got.app_id.as_deref(), Some("id-1"));
        assert_eq!(got.app_secret.as_deref(), Some("sec-1"));

        store.save_ocean_credentials("id-2", "sec-2").unwrap();
        store.save_ocean_credentials("id-2", "sec-2").unwrap();
        let got = store.get_ocean_credentials().unwrap();
        assert_eq!(got.app_id.as_deref(), Some("id-2"));
        assert_eq!(got.app_secret.as_deref(), Some("sec-2"));
    }
}
