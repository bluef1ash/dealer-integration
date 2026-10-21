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

/// 门店巨懂车会话 cookie（整罐；仅 passport sessionid 不够，见 issue #3）。
/// Windows 凭据管理器单条上限 2560 UTF-16 字符，整罐常超限——按分片存多条，读取拼接还原。
const COOKIE_CHUNK_CHARS: usize = 2000;
const COOKIE_CHUNK_CAP: usize = 64;

fn cookie_chunk_name(account: &str, i: usize) -> String {
    format!("store:{account}:cookie#{i}")
}

/// 按字符数安全分片（不会切断多字节字符）
fn chunk_text(text: &str, size: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    for (idx, _) in text.char_indices() {
        if idx - start >= size {
            chunks.push(&text[start..idx]);
            start = idx;
        }
    }
    if start < text.len() {
        chunks.push(&text[start..]);
    }
    chunks
}

pub fn save_store_cookie(account: &str, cookie: &str) -> crate::error::AppResult<()> {
    let chunks = chunk_text(cookie, COOKIE_CHUNK_CHARS);
    for (i, chunk) in chunks.iter().enumerate() {
        set(&cookie_chunk_name(account, i), chunk)?;
    }
    // 本次比上次短时清理多余旧分片
    let mut i = chunks.len();
    while i < COOKIE_CHUNK_CAP && get(&cookie_chunk_name(account, i))?.is_some() {
        delete(&cookie_chunk_name(account, i))?;
        i += 1;
    }
    eprintln!(
        "[凭证] 会话已存 account={account} 共 {} 字符 / {} 片",
        cookie.chars().count(),
        chunks.len()
    );
    Ok(())
}

pub fn load_store_cookie(account: &str) -> crate::error::AppResult<Option<String>> {
    let mut parts: Vec<String> = Vec::new();
    for i in 0..COOKIE_CHUNK_CAP {
        match get(&cookie_chunk_name(account, i))? {
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

pub fn delete_store(account: &str) -> crate::error::AppResult<()> {
    // 兼容清理：早期单条格式
    delete(&format!("store:{account}:cookie"))?;
    for i in 0..COOKIE_CHUNK_CAP {
        if get(&cookie_chunk_name(account, i))?.is_none() {
            break;
        }
        delete(&cookie_chunk_name(account, i))?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 分片→拼接无损，且每片不超限；多字节字符不被切断
    #[test]
    fn chunking_roundtrips_and_respects_limit() {
        let jar = "a=1; ".repeat(600); // 2400 字符 → 2 片
        let chunks = chunk_text(&jar, COOKIE_CHUNK_CHARS);
        assert!(chunks.len() >= 2);
        assert!(chunks
            .iter()
            .all(|c| c.chars().count() <= COOKIE_CHUNK_CHARS));
        assert_eq!(chunks.concat(), jar);

        let cjk = "会话测试值; ".repeat(700); // 多字节
        let chunks = chunk_text(cjk.trim_end_matches("; "), COOKIE_CHUNK_CHARS);
        assert_eq!(chunks.concat(), cjk.trim_end_matches("; "));
        assert!(chunks
            .iter()
            .all(|c| std::str::from_utf8(c.as_bytes()).is_ok()));
    }
}
