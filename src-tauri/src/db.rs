//! 数据库连接与 schema：全库唯一连接（Mutex 串行化），启动时执行幂等建表。

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// 打开（不存在则创建）库文件并执行建表
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(SCHEMA)?;
        Self::migrate(&conn);
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 渐进迁移：已建库补列（CREATE TABLE IF NOT EXISTS 不会更新既有表）
    fn migrate(conn: &Connection) {
        let add_column = |sql: &str| {
            // 列已存在时 ALTER 报错，忽略即可
            let _ = conn.execute_batch(sql);
        };
        add_column("ALTER TABLE leads ADD COLUMN feiyu_leads_id TEXT NOT NULL DEFAULT ''");
        add_column(
            "ALTER TABLE plan_reports_daily ADD COLUMN impressions INTEGER NOT NULL DEFAULT 0",
        );
        // 多应用化：广告账户记录归属的巨量应用（旧行为 NULL，启动迁移回填）
        add_column(
            "ALTER TABLE ad_accounts ADD COLUMN ocean_app_id INTEGER REFERENCES ocean_apps(id)",
        );
        // 巨懂车密码改存 SQLite（用户决策：随库走便于备份；旧值启动时从凭证库迁入）
        add_column("ALTER TABLE stores ADD COLUMN password TEXT NOT NULL DEFAULT ''");
        let _ = conn
            .execute_batch("CREATE INDEX IF NOT EXISTS idx_leads_feiyu ON leads(feiyu_leads_id)");
    }

    /// 内存库：测试与一次性工具用
    pub fn open_in_memory() -> Self {
        let conn = Connection::open_in_memory().expect("打开内存库");
        conn.execute_batch(SCHEMA).expect("执行建表");
        Self::migrate(&conn);
        Self {
            conn: Mutex::new(conn),
        }
    }

    /// 串行化访问连接；锁中毒视为不可恢复错误
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("数据库连接锁中毒".into()))?;
        f(&conn)
    }

    /// 同上，但闭包返回 rusqlite::Result（自动转 AppError）
    pub fn with_sql<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> AppResult<T> {
        self.with(|conn| f(conn).map_err(AppError::Db))
    }
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- 凭证键值表（会话 cookie、巨量 token；键名见 credentials.rs）
CREATE TABLE IF NOT EXISTS credentials (
    name  TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS stores (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    account    TEXT NOT NULL UNIQUE,
    password   TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS leads (
    leads_id       TEXT PRIMARY KEY,
    store_id       INTEGER NOT NULL REFERENCES stores(id),
    phone          TEXT NOT NULL DEFAULT '',
    name           TEXT NOT NULL DEFAULT '',
    status_key     TEXT NOT NULL DEFAULT '',
    status_label   TEXT NOT NULL DEFAULT '',
    source_label   TEXT NOT NULL DEFAULT '',
    intention      TEXT NOT NULL DEFAULT '',
    leads_time     INTEGER NOT NULL,
    feiyu_leads_id TEXT NOT NULL DEFAULT '',
    ad_id          TEXT NOT NULL DEFAULT '',
    promotion_id   TEXT NOT NULL DEFAULT '',
    promotion_name TEXT NOT NULL DEFAULT '',
    raw            TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_leads_store_time ON leads(store_id, leads_time);
CREATE INDEX IF NOT EXISTS idx_leads_time ON leads(leads_time);

CREATE TABLE IF NOT EXISTS store_pull_state (
    store_id        INTEGER PRIMARY KEY REFERENCES stores(id),
    last_success_at INTEGER,
    last_attempt_at INTEGER,
    last_error      TEXT NOT NULL DEFAULT '',
    backfilled      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS ocean_apps (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT NOT NULL,
    app_id           TEXT NOT NULL UNIQUE,
    app_secret       TEXT NOT NULL,
    token_expires_at INTEGER,
    created_at       INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS ad_accounts (
    id           TEXT PRIMARY KEY,
    alias        TEXT NOT NULL DEFAULT '',
    ocean_app_id INTEGER REFERENCES ocean_apps(id),
    created_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS store_ad_accounts (
    store_id      INTEGER NOT NULL REFERENCES stores(id),
    ad_account_id TEXT NOT NULL REFERENCES ad_accounts(id),
    PRIMARY KEY (store_id, ad_account_id)
);

CREATE TABLE IF NOT EXISTS ad_reports_daily (
    ad_account_id TEXT NOT NULL REFERENCES ad_accounts(id),
    date          TEXT NOT NULL,
    spend         REAL NOT NULL DEFAULT 0,
    impressions   INTEGER NOT NULL DEFAULT 0,
    clicks        INTEGER NOT NULL DEFAULT 0,
    conversions   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (ad_account_id, date)
);

CREATE TABLE IF NOT EXISTS plan_reports_daily (
    ad_account_id  TEXT NOT NULL REFERENCES ad_accounts(id),
    promotion_id   TEXT NOT NULL,
    promotion_name TEXT NOT NULL DEFAULT '',
    date           TEXT NOT NULL,
    spend          REAL NOT NULL DEFAULT 0,
    conversions    INTEGER NOT NULL DEFAULT 0,
    impressions    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (ad_account_id, promotion_id, date)
);

CREATE TABLE IF NOT EXISTS snapshots (
    date        TEXT PRIMARY KEY,
    imported_at INTEGER NOT NULL,
    leads_seen  INTEGER NOT NULL DEFAULT 0
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    /// 建表脚本可重复执行（升级路径幂等）
    #[test]
    fn schema_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch(SCHEMA).unwrap();
    }
}
