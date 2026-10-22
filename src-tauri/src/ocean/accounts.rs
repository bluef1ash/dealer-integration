//! 授权账户角色同步：token 授权返回的 id 角色多样，报表接口仅认广告账户
//! （ADVERTISER）角色——角色不符即 40002「doesn't exist or the role is wrong」。
//! 按官方口径展开（口径：oauth2/advertiser/get 的 account_type 枚举 + SDK 请求模型）：
//! - 工作台管理员/协作者（CUSTOMER_ADMIN/OPERATOR）→ /2/ebp/advertiser/list/ 分 AD/LOCAL 展开
//! - 代理商（AGENT/CHILD_AGENT）→ /2/agent/advertiser/select/ GET 展开名下广告主
//! - 企业号（PLATFORM_ROLE_ENTERPRISE_BP_*）与抖音号/来客等角色不能出报表，跳过并记日志

use serde_json::Value;

use super::Client;
use crate::error::{AppError, AppResult};

/// 展开后的可核算账户（已到广告账户粒度）
#[derive(Debug, Clone)]
pub struct AuthorizedAccount {
    pub id: String,
    pub name: String,
    /// 角色/类型原始枚举（联调日志用）
    pub role: String,
}

#[derive(Debug, PartialEq)]
enum Expansion {
    Direct,
    Ebp,
    Agent,
    Skip,
}

/// 角色 → 展开方式。注意 PLATFORM_ROLE_ENTERPRISE_BP_* 是抖音「企业号」，
/// 与升级版巨量引擎「工作台」（CUSTOMER_*）是两套体系，不能混
fn expansion(role: &str) -> Expansion {
    match role {
        "ADVERTISER" => Expansion::Direct,
        "CUSTOMER_ADMIN" | "CUSTOMER_OPERATOR" => Expansion::Ebp,
        "AGENT" | "CHILD_AGENT" => Expansion::Agent,
        _ => Expansion::Skip,
    }
}

/// 同步各步骤间的小间隔（账户接口同样吃开发者级频控，真机 40110）
const SYNC_PACE: std::time::Duration = std::time::Duration::from_millis(300);

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

    /// 代理商名下广告主 id 列表（GET；分页到取尽）。响应无名称，alias 留空待用户补
    pub async fn fetch_agent_advertisers(
        &self,
        token: &str,
        agent_id: &str,
    ) -> AppResult<Vec<AuthorizedAccount>> {
        let mut out = Vec::new();
        let mut page = 1i64;
        loop {
            let params = vec![
                ("advertiser_id".to_string(), agent_id.to_string()),
                ("page".to_string(), page.to_string()),
                ("page_size".to_string(), "100".to_string()),
            ];
            let text = self
                .get_json(super::AGENT_ADVERTISER_SELECT_URL, token, &params)
                .await?;
            let rows = parse_agent_advertiser_ids(&text)?;
            let got = rows.len();
            out.extend(rows);
            if got < 100 || page > 100 {
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

/// 把授权范围展开为可核算广告账户。仅「已授权账户接口本身失败」时回退 token 返回 id
/// （对抖动容错）；接口正常但无可用广告账户就返回空——回退塞入的非广告账户 id
/// 只会让调度轮空转报 40002（真机教训）。
pub async fn sync_advertiser_accounts(
    client: &Client,
    token: &str,
    token_ids: &[Value],
) -> Vec<AuthorizedAccount> {
    let authorized = match client.fetch_authorized_accounts(token).await {
        Ok(list) => list,
        Err(e) => {
            eprintln!("[OAuth] 已授权账户查询失败，回退 token 返回 id: {e}");
            return fallback_accounts(token_ids);
        }
    };
    if authorized.is_empty() {
        eprintln!("[OAuth] 已授权账户列表为空，该登录名下没有可授权账户");
        return Vec::new();
    }
    let mut out: Vec<AuthorizedAccount> = Vec::new();
    for acc in &authorized {
        // 每账户一行角色日志：40002 类问题的确诊线索
        eprintln!(
            "[OAuth] 授权账户 {} 角色={} 名称={}",
            acc.id, acc.role, acc.name
        );
        match expansion(&acc.role) {
            Expansion::Direct => out.push(acc.clone()),
            Expansion::Ebp => {
                // 工作台账户按官方要求分 AD / LOCAL 两源展开
                for source in ["AD", "LOCAL"] {
                    match client.fetch_ebp_accounts(token, &acc.id, source).await {
                        Ok(rows) if !rows.is_empty() => {
                            eprintln!(
                                "[OAuth] 工作台 {} {source} 源展开出 {} 个广告账户",
                                acc.id,
                                rows.len()
                            );
                            out.extend(rows);
                        }
                        Ok(_) => eprintln!("[OAuth] 工作台 {} {source} 源无挂接广告账户", acc.id),
                        Err(e) => eprintln!(
                            "[OAuth] 工作台 {}（角色 {}）{source} 源展开失败: {e}",
                            acc.id, acc.role
                        ),
                    }
                    tokio::time::sleep(SYNC_PACE).await;
                }
            }
            Expansion::Agent => match client.fetch_agent_advertisers(token, &acc.id).await {
                Ok(rows) if !rows.is_empty() => {
                    eprintln!("[OAuth] 代理商 {} 展开出 {} 个广告主", acc.id, rows.len());
                    out.extend(rows);
                }
                Ok(_) => eprintln!("[OAuth] 代理商 {} 名下无广告主", acc.id),
                Err(e) => eprintln!(
                    "[OAuth] 代理商 {}（角色 {}）广告主展开失败: {e}",
                    acc.id, acc.role
                ),
            },
            Expansion::Skip => {
                eprintln!(
                    "[OAuth] 账户 {} 角色 {} 不能出报表，已跳过",
                    acc.id, acc.role
                );
            }
        }
        tokio::time::sleep(SYNC_PACE).await;
    }
    // 跨角色展开可能重复给出同一账户，按 id 去重保序
    let mut seen = std::collections::HashSet::new();
    out.retain(|acc| seen.insert(acc.id.clone()));
    if out.is_empty() {
        eprintln!("[OAuth] 角色展开后无可用广告账户（详见上方各账户角色日志）");
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

/// 已授权账户行：account_id/account_type 为现行字段，advertiser_id/account_role 已废弃仍兜底
fn parse_authorized_accounts(body: &str) -> AppResult<Vec<AuthorizedAccount>> {
    let env = parse_envelope(body)?;
    let mut out = Vec::new();
    if let Some(rows) = env["data"]["list"].as_array() {
        for row in rows {
            let mut id = js(&row["account_id"]);
            if id.is_empty() {
                id = js(&row["advertiser_id"]);
            }
            if id.is_empty() {
                continue;
            }
            let role = row["account_type"]
                .as_str()
                .or_else(|| row["account_role"].as_str())
                .unwrap_or("UNKNOWN")
                .to_string();
            let name = row["account_name"]
                .as_str()
                .or_else(|| row["advertiser_name"].as_str())
                .unwrap_or("")
                .to_string();
            // is_valid 缺省视为可用（部分角色行不带该字段）
            if row["is_valid"].as_bool().unwrap_or(true) {
                out.push(AuthorizedAccount { id, name, role });
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

/// 代理商名下广告主：data.list 为纯 id 数组
fn parse_agent_advertiser_ids(body: &str) -> AppResult<Vec<AuthorizedAccount>> {
    let env = parse_envelope(body)?;
    let mut out = Vec::new();
    if let Some(rows) = env["data"]["list"].as_array() {
        for row in rows {
            let id = js(row);
            if !id.is_empty() {
                out.push(AuthorizedAccount {
                    id,
                    name: String::new(),
                    role: "AGENT_ADVERTISER".to_string(),
                });
            }
        }
    }
    Ok(out)
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
        // 工作台（CUSTOMER_*）走 EBP 展开；企业号（ENTERPRISE_BP）是另一套体系，跳过
        assert_eq!(expansion("CUSTOMER_ADMIN"), Expansion::Ebp);
        assert_eq!(expansion("CUSTOMER_OPERATOR"), Expansion::Ebp);
        assert_eq!(
            expansion("PLATFORM_ROLE_ENTERPRISE_BP_ADMIN"),
            Expansion::Skip
        );
        assert_eq!(expansion("AGENT"), Expansion::Agent);
        assert_eq!(expansion("CHILD_AGENT"), Expansion::Agent);
        assert_eq!(expansion("PLATFORM_ROLE_AWEME"), Expansion::Skip);
        assert_eq!(expansion("PLATFORM_ROLE_LIFE"), Expansion::Skip);
    }

    /// 现行字段 account_id/account_type 优先，废弃字段 advertiser_id/account_role 兜底；
    /// 仅保留 is_valid 的行
    #[test]
    fn authorized_accounts_parse_prefers_current_fields() {
        let body = r#"{"code":0,"message":"OK","data":{"list":[
            {"account_id":1714295786109960,"account_name":"工作台甲",
             "account_type":"CUSTOMER_ADMIN","advertiser_id":1714295786109960,
             "advertiser_name":"旧名甲","account_role":"CUSTOMER_ADMIN","is_valid":true},
            {"advertiser_id":7366911332093904950,"advertiser_name":"来客乙",
             "account_role":"PLATFORM_ROLE_LIFE","is_valid":false},
            {"account_id":1734414657010702,"account_name":"客户丙",
             "account_type":"ADVERTISER"}
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

    /// 代理商名下广告主：data.list 为纯 id 数组
    #[test]
    fn agent_advertiser_ids_parse() {
        let body = r#"{"code":0,"message":"OK","data":{
            "list":[1714295786109960,1734414657010702],
            "page_info":{"page":1,"page_size":100,"total_number":2}}}"#;
        let rows = parse_agent_advertiser_ids(body).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "1714295786109960");
        assert_eq!(rows[0].role, "AGENT_ADVERTISER");
    }
}
