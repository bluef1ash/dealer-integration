//! 巨懂车 HTTP 客户端：spike 结论（issue #3）落地——
//! 认证 = 整罐会话 cookie + content-type + UA + `jdc-saas-header-app-id` 头，无签名参数。
//! 列表直调 + 导出三段式（提交任务 → 任务列表 → 按 task_id 直出字节流）。

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const BASE: &str = "https://www.autoengine.com";
const APP_ID_HEADER: &str = "549";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";

/// 列表/导出的共同筛选口径
#[derive(Debug, Clone, Serialize)]
pub struct LeadsWindow {
    pub leads_time: TimeRange,
    pub page_key: &'static str,
    pub only_out_leads: bool,
    pub is_dy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeRange {
    /// unix 秒（含）
    pub min: i64,
    /// unix 秒（含）
    pub max: i64,
}

impl LeadsWindow {
    /// 列表请求体（带分页）；常规门店口径为 only_out_leads=false，流出线索视图为 true
    pub fn list(
        min: i64,
        max: i64,
        page_number: u32,
        page_size: u32,
        only_out_leads: bool,
    ) -> Self {
        Self {
            leads_time: TimeRange { min, max },
            page_key: "dealer_out_leads_list",
            only_out_leads,
            is_dy: true,
            page_number: Some(page_number),
            page_size: Some(page_size),
        }
    }

    /// 导出请求体（全量，不带分页）
    pub fn export(min: i64, max: i64, only_out_leads: bool) -> Self {
        Self {
            leads_time: TimeRange { min, max },
            page_key: "dealer_out_leads_list",
            only_out_leads,
            is_dy: true,
            page_number: None,
            page_size: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ExportTask {
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub task_name: String,
    /// 2 = 完成（联调校正口径）
    #[serde(default)]
    pub status: i64,
    #[serde(default)]
    pub hint_msg: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct TaskListData {
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub tasks: Vec<ExportTask>,
}

#[derive(Debug, Deserialize, Default)]
pub struct TaskListResponse {
    #[serde(default)]
    pub status: i64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: TaskListData,
}

#[derive(Debug, Deserialize, Default)]
pub struct SimpleResponse {
    #[serde(default)]
    pub status: i64,
    #[serde(default)]
    pub message: String,
    /// 成功时为任务 ID 字符串
    #[serde(default)]
    pub data: String,
}

pub struct Client {
    http: reqwest::Client,
}

impl Client {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(UA)
            .build()
            .expect("构建 HTTP 客户端");
        Self { http }
    }

    fn get(&self, cookies: &str, url: &str) -> reqwest::RequestBuilder {
        self.http
            .get(url)
            .header("cookie", cookies)
            .header("content-type", "application/json")
            .header("jdc-saas-header-app-id", APP_ID_HEADER)
    }

    fn post(&self, cookies: &str, url: &str, body: &impl Serialize) -> reqwest::RequestBuilder {
        self.http
            .post(url)
            .header("cookie", cookies)
            .header("content-type", "application/json")
            .header("jdc-saas-header-app-id", APP_ID_HEADER)
            .json(body)
    }

    /// 线索列表（主路径：会话直调）
    pub async fn list_leads(&self, cookies: &str, window: &LeadsWindow) -> AppResult<String> {
        let url = format!("{BASE}/motor/crm_wb/leads/scan/list?__method=window.fetch");
        let resp = self
            .post(cookies, &url, window)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.text().await?)
    }

    /// 提交导出任务，返回任务 ID
    pub async fn create_export(&self, cookies: &str, window: &LeadsWindow) -> AppResult<String> {
        let url = format!("{BASE}/motor/crm_wb/leads/scan/async_export?__method=window.fetch");
        let text = self
            .post(cookies, &url, window)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let parsed: SimpleResponse = serde_json::from_str(&text)
            .map_err(|e| AppError::Message(format!("导出响应解析失败: {e}")))?;
        if parsed.status != 0 {
            return Err(AppError::remote(parsed.status, &parsed.message));
        }
        Ok(parsed.data)
    }

    /// 导出记录列表（轮询任务状态）
    pub async fn task_list(
        &self,
        cookies: &str,
        page: u32,
        page_size: u32,
    ) -> AppResult<TaskListResponse> {
        let url = format!(
            "{BASE}/motor/crm_wb/task/instance/list?__method=window.fetch&scene=105&page_number={page}&page_size={page_size}"
        );
        let text = self
            .get(cookies, &url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let parsed: TaskListResponse = serde_json::from_str(&text)
            .map_err(|e| AppError::Message(format!("任务列表解析失败: {e}")))?;
        if parsed.status != 0 {
            return Err(AppError::remote(parsed.status, &parsed.message));
        }
        Ok(parsed)
    }

    /// 按 task_id 下载导出文件（直出 xlsx 字节流）
    pub async fn download_export(&self, cookies: &str, task_id: &str) -> AppResult<Vec<u8>> {
        let url = format!("{BASE}/motor/crm_wb/task/crm_data/export?__method=window.fetch");
        let bytes = self
            .post(cookies, &url, &serde_json::json!({ "task_id": task_id }))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        if bytes.starts_with(b"PK") {
            Ok(bytes.to_vec())
        } else {
            Err(AppError::Message(
                "导出下载未返回 xlsx 内容（会话可能失效）".into(),
            ))
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
