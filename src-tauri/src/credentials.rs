//! 凭证存取：全部存 SQLite `credentials` 键值表（用户决策 2026-10-22：跨平台使用，
//! 凭证随库迁移，不再依赖系统凭证库；此前 2560 字节分片是 CredMan 特有限制，入库后不再需要）。
//! 旧版凭证库条目由 migrate_keyring_into_db 在启动时搬入库内并清除，迁移完成后本模块无 keyring 写入路径。

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

// ---------- 旧版系统凭证库迁移（只读旧条目并清除） ----------

const SERVICE: &str = "dealer-integration";
/// 旧版 cookie 分片上限（读取拼接旧数据用）
const COOKIE_CHUNK_CAP: usize = 64;

fn keyring_get(name: &str) -> AppResult<Option<String>> {
    let entry = keyring::Entry::new(SERVICE, name)
        .map_err(|e| AppError::Message(format!("创建凭证条目失败: {e}")))?;
    match entry.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn keyring_delete(name: &str) -> AppResult<()> {
    let entry = keyring::Entry::new(SERVICE, name)
        .map_err(|e| AppError::Message(format!("创建凭证条目失败: {e}")))?;
    let _ = entry.delete_credential();
    Ok(())
}

fn cookie_chunk_name(account: &str, i: usize) -> String {
    format!("store:{account}:cookie#{i}")
}

/// 读取旧版分片存储的 cookie 并拼接还原
fn keyring_cookie(account: &str) -> AppResult<Option<String>> {
    let mut parts: Vec<String> = Vec::new();
    for i in 0..COOKIE_CHUNK_CAP {
        match keyring_get(&cookie_chunk_name(account, i))? {
            Some(part) => parts.push(part),
            None => break,
        }
    }
    Ok(if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    })
}

fn keyring_delete_cookie(account: &str) -> AppResult<()> {
    // 兼容清理：早期单条格式 + 分片格式
    if let Ok(entry) = keyring::Entry::new(SERVICE, &format!("store:{account}:cookie")) {
        let _ = entry.delete_credential();
    }
    for i in 0..COOKIE_CHUNK_CAP {
        if keyring_get(&cookie_chunk_name(account, i))?.is_none() {
            break;
        }
        keyring_delete(&cookie_chunk_name(account, i))?;
    }
    Ok(())
}

fn keyring_tokens(app_id: &str) -> AppResult<(Option<String>, Option<String>)> {
    Ok((
        keyring_get(&format!("ocean:{app_id}:access_token"))?,
        keyring_get(&format!("ocean:{app_id}:refresh_token"))?,
    ))
}

fn keyring_delete_tokens(app_id: &str) -> AppResult<()> {
    keyring_delete(&format!("ocean:{app_id}:access_token"))?;
    keyring_delete(&format!("ocean:{app_id}:refresh_token"))?;
    Ok(())
}

/// 旧版凭证库 → SQLite 一次性迁移（幂等）：库内已有值时只清理旧条目。
/// 返回 (迁入会话数, 迁入 token 组数)；任一失败即返回错误，下次启动重试。
pub fn migrate_keyring_into_db(conn: &Connection) -> AppResult<(usize, usize)> {
    let mut cookies = 0;
    for store in crate::repo::stores::list(conn)? {
        let Some(jar) = keyring_cookie(&store.account)? else {
            continue;
        };
        if load_store_cookie(conn, &store.account)?.is_none() {
            save_store_cookie(conn, &store.account, &jar)?;
            cookies += 1;
        }
        keyring_delete_cookie(&store.account)?;
    }
    let mut tokens = 0;
    for app in crate::repo::ocean_apps::list(conn)? {
        let (Some(access), Some(refresh)) = keyring_tokens(&app.app_id)? else {
            continue;
        };
        if load_ocean_tokens(conn, &app.app_id)?.0.is_none() {
            save_ocean_tokens(conn, &app.app_id, &access, &refresh)?;
            tokens += 1;
        }
        keyring_delete_tokens(&app.app_id)?;
    }
    Ok((cookies, tokens))
}

/// 旧版全局 token 条目（多应用化前）迁入对应应用名下；无旧条目时为空操作
pub fn migrate_legacy_tokens_into_db(conn: &Connection, app_id: &str) -> AppResult<()> {
    let Some(access) = keyring_get("ocean:access_token")? else {
        return Ok(());
    };
    let Some(refresh) = keyring_get("ocean:refresh_token")? else {
        return Ok(());
    };
    save_ocean_tokens(conn, app_id, &access, &refresh)?;
    keyring_delete("ocean:access_token")?;
    keyring_delete("ocean:refresh_token")?;
    eprintln!("[凭证] 巨量旧全局 token 已迁入按应用条目 app_id={app_id}");
    Ok(())
}

// ---------- 旧版密码条目（密码已改存 stores.password，仅剩启动迁移读写） ----------

/// 旧版密码条目读取（迁移用，见 repo::stores::migrate_keyring_passwords）
pub fn load_legacy_password(account: &str) -> AppResult<Option<String>> {
    keyring_get(&format!("store:{account}:password"))
}

/// 旧版密码条目清理：密码已改存 SQLite（repo::stores），启动迁移时删除凭证库残留
pub fn delete_legacy_password(account: &str) -> AppResult<()> {
    keyring_delete(&format!("store:{account}:password"))
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
