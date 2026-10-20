//! 统一错误类型：各层错误归口到 AppError，Tauri command 边界转 String 展示。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("网络请求失败: {0}")]
    Http(#[from] reqwest::Error),
    #[error("凭证库错误: {0}")]
    Keyring(String),
    #[error("Excel 处理失败: {0}")]
    Excel(String),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("Tauri 错误: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("{0}")]
    Message(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    /// 远端业务失败（HTTP 200 但 code 非 0）
    pub fn remote(code: impl std::fmt::Display, message: impl Into<String>) -> Self {
        Self::Message(format!("远端错误 {code}: {}", message.into()))
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        Self::Keyring(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::Message(format!("JSON 解析失败: {e}"))
    }
}
