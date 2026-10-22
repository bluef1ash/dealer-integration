//! 应用级设置（settings 键值表）：调度与对账等开关。
//! 巨量引擎应用凭证已多应用化，存 ocean_apps 表（见 repo::ocean_apps）。

use rusqlite::{Connection, OptionalExtension};

/// 对账快照触发小时（0-23），设置页可改
pub const KEY_SNAPSHOT_HOUR: &str = "snapshot.hour";
/// 自动拉取开关（默认开）
pub const KEY_SCHEDULER_ENABLED: &str = "scheduler.enabled";
/// 自动拉取间隔（分钟，默认 60）
pub const KEY_SCHEDULER_INTERVAL: &str = "scheduler.interval_minutes";

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .optional()
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
    fn save_read_roundtrip() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            save_setting(conn, KEY_SNAPSHOT_HOUR, "23").unwrap();
            assert_eq!(
                get_setting(conn, KEY_SNAPSHOT_HOUR).unwrap().as_deref(),
                Some("23")
            );
            save_setting(conn, KEY_SNAPSHOT_HOUR, "8").unwrap();
            assert_eq!(
                get_setting(conn, KEY_SNAPSHOT_HOUR).unwrap().as_deref(),
                Some("8")
            );
            Ok(())
        })
        .unwrap();
    }
}
