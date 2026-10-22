//! 授权账户角色同步：token 授权返回的 id 角色多样（工作台管理员/抖音号/来客等），
//! 报表接口仅认客户（ADVERTISER）角色——角色不符即 40002「doesn't exist or the role is wrong」
//! （2026-10-22 真机校正）。按官方口径展开：已授权账户 oauth2/advertiser/get 供角色清单，
//! 工作台角色再经 2/ebp/advertiser/list 展开为真实广告账户。

use serde_json::Value;

use super::Client;
use crate::error::{AppError, AppResult};

/// 展开后的可核算账户（已到广告账户粒度）
#[derive(Debug, Clone)]
pub struct AuthorizedAccount {
    pub id: String,
    pub name: String,
    /// 角色原始枚举（联调日志用）
    pub role: String,
}

#[derive(Debug, PartialEq)]
enum Expansion {
    Direct,
    Ebp,
    Skip,
}

/// 角色 → 展开方式（口径见 oauth2/advertiser/get 文档 account_role 枚举）
fn expansion(role: &str) -> Expansion {
    match role {
        "ADVERTISER" => Expansion::Direct,
        // 升级版巨量引擎工作台管理员/协作者（新旧两代枚举都在用）
        "CUSTOMER_ADMIN"
        | "CUSTOMER_OPERATOR"
        | "PLATFORM_ROLE_ENTERPRISE_BP_ADMIN"
        | "PLATFORM_ROLE_ENTERPRISE_BP_OPERATOR" => Expansion::Ebp,
        _ => Expansion::Skip,
    }
}

impl Client {
    /// 已授权账户列表（含角色与有效性）
    pub async fn fetch_authorized_accounts(
        &self,
        token: &str,
    ) -> AppResult<Vec<AuthorizedAccount>> {
        parse_authorized_accounts(
            &self
                .get_json(super::AUTHORIZED_ACCOUNTS_URL, token, &[])
                .await?,
        )
    }

    /// 工作台下账户列表（分页到取尽）；source：AD = 巨量营销客户账号，LOCAL = 本地推
    pub async fn fetch_ebp_accounts(
        &self,
        token: &str,
        org_id: &str,
        source: &str,
    ) -> AppResult<Vec<AuthorizedAccount>> {
        let mut out = Vec::new();
        let mut page = 1i64;
        loop {
            let params = vec![
                ("enterprise_organization_id".to_string(), org_id.to_string()),
                ("account_source".to_string(), source.to_string()),
                ("page".to_string(), page.to_string()),
                ("page_size".to_string(), "100".to_string()),
            ];
            let text = self
                .get_json(super::EBP_ACCOUNT_LIST_URL, token, &params)
                .await?;
            let (rows, total_page) = parse_ebp_accounts(&text)?;
            out.extend(rows);
            if page >= total_page.max(1) || page > 100 {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    async fn get_json(
        &self,
        url: &str,
        token: &str,
        params: &[(String, String)],
    ) -> AppResult<String> {
        Ok(self
            .http
            .get(url)
            .header("Access-Token", token)
            .query(params)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?)
    }
}

/// 把授权范围展开为可核算广告账户。任何一步失败都不阻断授权流程：
/// 查不到角色清单时回退 token 返回 id（保持旧行为，由报表调用侧暴露角色问题）。
pub async fn sync_advertiser_accounts(
    client: &Client,
    token: &str,
    token_ids: &[Value],
) -> Vec<AuthorizedAccount> {
    let authorized = match client.fetch_authorized_accounts(token).await {
        Ok(list) if !list.is_empty() => list,
        Ok(_) => {
            eprintln!("[OAuth] 已授权账户列表为空，回退 token 返回 id");
            return fallback_accounts(token_ids);
        }
        Err(e) => {
            eprintln!("[OAuth] 已授权账户查询失败，回退 token 返回 id: {e}");
            return fallback_accounts(token_ids);
        }
    };
    let mut out: Vec<AuthorizedAccount> = Vec::new();
    for acc in &authorized {
        match expansion(&acc.role) {
            Expansion::Direct => out.push(acc.clone()),
            Expansion::Ebp => {
                // 工作台账户按官方要求分 AD / LOCAL 两源展开
                for source in ["AD", "LOCAL"] {
                    match client.fetch_ebp_accounts(token, &acc.id, source).await {
                        Ok(rows) => out.extend(rows),
                        Err(e) => {
                            eprintln!("[OAuth] 工作台 {} {source} 源账户展开失败: {e}", acc.id)
                        }
                    }
                }
            }
            Expansion::Skip => {
                eprintln!(
                    "[OAuth] 账户 {} 角色为 {}，不能直接出报表，已跳过",
                    acc.id, acc.role
                );
            }
        }
    }
    // 跨角色展开可能重复给出同一账户，按 id 去重保序
    let mut seen = std::collections::HashSet::new();
    out.retain(|acc| seen.insert(acc.id.clone()));
    if out.is_empty() {
        eprintln!("[OAuth] 角色展开后无可用广告账户，回退 token 返回 id");
        return fallback_accounts(token_ids);
    }
    out
}

fn fallback_accounts(token_ids: &[Value]) -> Vec<AuthorizedAccount> {
    token_ids
        .iter()
        .map(|v| AuthorizedAccount {
            id: js(v),
            name: String::new(),
            role: "TOKEN_RESPONSE".to_string(),
        })
        .filter(|acc| !acc.id.is_empty())
        .collect()
}

fn parse_authorized_accounts(body: &str) -> AppResult<Vec<AuthorizedAccount>> {
    let env = parse_envelope(body)?;
    let mut out = Vec::new();
    if let Some(rows) = env["data"]["list"].as_array() {
        for row in rows {
            let id = js(&row["advertiser_id"]);
            if id.is_empty() {
                continue;
            }
            let role = row["account_role"]
                .as_str()
                .unwrap_or("UNKNOWN")
                .to_string();
            // is_valid 缺省视为可用（部分角色行不带该字段）
            if row["is_valid"].as_bool().unwrap_or(true) {
                out.push(AuthorizedAccount {
                    id,
                    name: row["advertiser_name"].as_str().unwrap_or("").to_string(),
                    role,
                });
            } else {
                eprintln!("[OAuth] 授权账户 {id}（{role}）无操作权限，跳过");
            }
        }
    }
    Ok(out)
}

/// 工作台账户列表：data.account_list + page_info.total_page
fn parse_ebp_accounts(body: &str) -> AppResult<(Vec<AuthorizedAccount>, i64)> {
    let env = parse_envelope(body)?;
    let mut out = Vec::new();
    if let Some(rows) = env["data"]["account_list"].as_array() {
        for row in rows {
            let id = js(&row["account_id"]);
            if id.is_empty() {
                continue;
            }
            out.push(AuthorizedAccount {
                id,
                name: row["account_name"].as_str().unwrap_or("").to_string(),
                role: row["account_type"]
                    .as_str()
                    .unwrap_or("UNKNOWN")
                    .to_string(),
            });
        }
    }
    let total_page = env["data"]["page_info"]["total_page"].as_i64().unwrap_or(1);
    Ok((out, total_page))
}

/// 通用业务帧：code != 0 抛远端错误（错误码透传给上层日志）
fn parse_envelope(body: &str) -> AppResult<Value> {
    let env: Value = serde_json::from_str(body)
        .map_err(|e| AppError::Message(format!("账户接口响应解析失败: {e}")))?;
    let code = env["code"].as_i64().unwrap_or(0);
    if code != 0 {
        return Err(AppError::remote(
            code,
            env["message"].as_str().unwrap_or("未知错误"),
        ));
    }
    Ok(env)
}

/// 账户 id 字段口径：平台可能回数字或字符串
fn js(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_classified_for_expansion() {
        assert_eq!(expansion("ADVERTISER"), Expansion::Direct);
        assert_eq!(expansion("CUSTOMER_ADMIN"), Expansion::Ebp);
        assert_eq!(
            expansion("PLATFORM_ROLE_ENTERPRISE_BP_OPERATOR"),
            Expansion::Ebp
        );
        assert_eq!(expansion("PLATFORM_ROLE_AWEME"), Expansion::Skip);
        assert_eq!(expansion("AGENT"), Expansion::Skip);
    }

    /// 已授权账户：仅保留 is_valid 的行；字段缺省按可用处理
    #[test]
    fn authorized_accounts_parse_keeps_valid_rows() {
        let body = r#"{"code":0,"message":"OK","data":{"list":[
            {"advertiser_id":1714295786109960,"advertiser_name":"工作台甲",
             "account_role":"CUSTOMER_ADMIN","is_valid":true},
            {"advertiser_id":7366911332093904950,"advertiser_name":"抖音号乙",
             "account_role":"PLATFORM_ROLE_AWEME","is_valid":false},
            {"advertiser_id":1734414657010702,"advertiser_name":"客户丙",
             "account_role":"ADVERTISER"}
        ]}}"#;
        let rows = parse_authorized_accounts(body).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].role, "CUSTOMER_ADMIN");
        assert_eq!(rows[0].name, "工作台甲");
        assert_eq!(rows[1].role, "ADVERTISER");
        assert_eq!(rows[1].id, "1734414657010702");
    }

    #[test]
    fn business_error_surfaced_for_account_apis() {
        let err = parse_authorized_accounts(r#"{"code":40105,"message":"access_token无效"}"#)
            .unwrap_err();
        assert!(err.to_string().contains("40105"));
    }

    /// 工作台账户列表：account_list 键 + 分页 total_page
    #[test]
    fn ebp_account_list_parses() {
        let body = r#"{"code":0,"message":"OK","data":{"account_list":[
            {"account_id":1819438458456045,"account_type":"AD_NORMAL","account_name":"账户A"},
            {"account_id":1789000523652445,"account_type":"LOCAL","account_name":"账户B"}
        ],"page_info":{"page":1,"page_size":100,"total_page":1,"total_number":2}}}"#;
        let (rows, total_page) = parse_ebp_accounts(body).unwrap();
        assert_eq!(total_page, 1);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "1819438458456045");
        assert_eq!(rows[0].name, "账户A");
        assert_eq!(rows[1].role, "LOCAL");
    }
}
