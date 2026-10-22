//! 凭证存取：全部存 SQLite `credentials` 键值表（用户决策：跨平台使用，凭证随库迁移，
//! 不依赖系统凭证库；此前 CredMan 的 2560 字节分片是凭证库特有限制，入库后单条存储）。

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

fn get(conn: &Connection, name: &str) -> AppResult<Option<String>> {
    conn.query_row(
        "SELECT value FROM credentials WHERE name = ?1",
        [name],
        |r| r.get(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(AppError::Db(other)),
    })
}

fn set(conn: &Connection, name: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO credentials (name, value) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET value = excluded.value",
        [name, value],
    )?;
    Ok(())
}

fn delete(conn: &Connection, name: &str) -> AppResult<()> {
    conn.execute("DELETE FROM credentials WHERE name = ?1", [name])?;
    Ok(())
}

/// 门店巨懂车会话 cookie（整罐；仅 passport sessionid 不够，见 issue #3）
fn cookie_name(account: &str) -> String {
    format!("store:{account}:cookie")
}

pub fn save_store_cookie(conn: &Connection, account: &str, cookie: &str) -> AppResult<()> {
    set(conn, &cookie_name(account), cookie)?;
    eprintln!(
        "[凭证] 会话已存 account={account} 共 {} 字符",
        cookie.chars().count()
    );
    Ok(())
}

pub fn load_store_cookie(conn: &Connection, account: &str) -> AppResult<Option<String>> {
    get(conn, &cookie_name(account))
}

/// 删除门店时清理其会话
pub fn delete_store(conn: &Connection, account: &str) -> AppResult<()> {
    delete(conn, &cookie_name(account))
}

/// 巨量引擎 OAuth token（按应用分条；一个应用的授权覆盖其名下全部广告账户）
fn ocean_token_names(app_id: &str) -> (String, String) {
    (
        format!("ocean:{app_id}:access_token"),
        format!("ocean:{app_id}:refresh_token"),
    )
}

pub fn save_ocean_tokens(
    conn: &Connection,
    app_id: &str,
    access: &str,
    refresh: &str,
) -> AppResult<()> {
    let (access_name, refresh_name) = ocean_token_names(app_id);
    set(conn, &access_name, access)?;
    set(conn, &refresh_name, refresh)?;
    Ok(())
}

pub fn load_ocean_tokens(
    conn: &Connection,
    app_id: &str,
) -> AppResult<(Option<String>, Option<String>)> {
    let (access_name, refresh_name) = ocean_token_names(app_id);
    Ok((get(conn, &access_name)?, get(conn, &refresh_name)?))
}

/// 删除应用时清理其 token
pub fn delete_ocean_tokens(conn: &Connection, app_id: &str) -> AppResult<()> {
    let (access_name, refresh_name) = ocean_token_names(app_id);
    delete(conn, &access_name)?;
    delete(conn, &refresh_name)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn cookie_roundtrip_overwrite_delete() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            assert_eq!(load_store_cookie(conn, "13800000001")?, None);
            save_store_cookie(conn, "13800000001", "a=1; b=2")?;
            assert_eq!(
                load_store_cookie(conn, "13800000001")?.as_deref(),
                Some("a=1; b=2")
            );
            // 覆盖写 + 超长值不受旧分片限制
            let big = "motor_dealer_session_id=abc; ".repeat(500);
            save_store_cookie(conn, "13800000001", &big)?;
            assert_eq!(
                load_store_cookie(conn, "13800000001")?.as_deref(),
                Some(big.as_str())
            );
            delete_store(conn, "13800000001")?;
            assert_eq!(load_store_cookie(conn, "13800000001")?, None);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn ocean_tokens_roundtrip_and_delete() {
        let db = Db::open_in_memory();
        db.with(|conn| {
            assert_eq!(load_ocean_tokens(conn, "1700000001")?, (None, None));
            save_ocean_tokens(conn, "1700000001", "acc", "ref")?;
            assert_eq!(
                load_ocean_tokens(conn, "1700000001")?,
                (Some("acc".into()), Some("ref".into()))
            );
            save_ocean_tokens(conn, "1700000001", "acc2", "ref2")?;
            assert_eq!(
                load_ocean_tokens(conn, "1700000001")?,
                (Some("acc2".into()), Some("ref2".into()))
            );
            delete_ocean_tokens(conn, "1700000001")?;
            assert_eq!(load_ocean_tokens(conn, "1700000001")?, (None, None));
            Ok(())
        })
        .unwrap();
    }
}
