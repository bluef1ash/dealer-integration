//! 系统凭证库封装（Windows 凭据管理器 / macOS 钥匙串）。
//! SQLite 只存非敏感配置；会话 cookie、账号密码、巨量 token 一律走这里（spec 实现决策）。

use keyring::Entry;

const SERVICE: &str = "dealer-integration";

fn entry(name: &str) -> Entry {
    Entry::new(SERVICE, name).expect("创建凭证条目")
}

fn set(name: &str, value: &str) -> crate::error::AppResult<()> {
    let e = entry(name);
    // keyring 对已存在条目 set 会覆盖；先删后设规避平台差异（Windows 偶发冲突）
    let _ = e.delete_credential();
    e.set_password(value)?;
    Ok(())
}

fn get(name: &str) -> crate::error::AppResult<Option<String>> {
    match entry(name).get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn delete(name: &str) -> crate::error::AppResult<()> {
    let _ = entry(name).delete_credential();
    Ok(())
}

/// 门店巨懂车会话 cookie（整罐；仅 passport sessionid 不够，见 issue #3）
pub fn store_cookie_name(account: &str) -> String {
    format!("store:{account}:cookie")
}

pub fn save_store_cookie(account: &str, cookie: &str) -> crate::error::AppResult<()> {
    set(&store_cookie_name(account), cookie)
}

pub fn load_store_cookie(account: &str) -> crate::error::AppResult<Option<String>> {
    get(&store_cookie_name(account))
}

pub fn delete_store(account: &str) -> crate::error::AppResult<()> {
    delete(&store_cookie_name(account))?;
    delete(&store_password_name(account))?;
    Ok(())
}

/// 门店巨懂车密码（可选录入，供会话失效自动重登；短信路径不需要）
pub fn store_password_name(account: &str) -> String {
    format!("store:{account}:password")
}

pub fn save_store_password(account: &str, password: &str) -> crate::error::AppResult<()> {
    set(&store_password_name(account), password)
}

pub fn load_store_password(account: &str) -> crate::error::AppResult<Option<String>> {
    get(&store_password_name(account))
}

/// 巨量引擎 OAuth token（单一授权覆盖该公司全部授权账户）
pub fn save_ocean_tokens(access: &str, refresh: &str) -> crate::error::AppResult<()> {
    set("ocean:access_token", access)?;
    set("ocean:refresh_token", refresh)?;
    Ok(())
}

pub fn load_ocean_tokens() -> crate::error::AppResult<(Option<String>, Option<String>)> {
    Ok((get("ocean:access_token")?, get("ocean:refresh_token")?))
}
