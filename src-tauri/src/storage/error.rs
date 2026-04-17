//! storage 层统一错误类型。
//!
//! 背景：本模块下原先混合返回 `Result<T, rusqlite::Error>`（repository.rs）和
//! `Result<T, String>`（seed_rules.rs / retention.rs）。调用方要处理两种错误类型，
//! 也无法按错误种类做分支（String 丢了类型信息）。
//!
//! 现在 seed_rules / retention / commands 所有新写的 storage helper 统一返 StorageError。
//! repository.rs 底层仍然返 `rusqlite::Error`（那是最内层），上层包装成 StorageError。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("yaml parse: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Message(String),
}

impl From<String> for StorageError {
    fn from(s: String) -> Self {
        StorageError::Message(s)
    }
}

impl From<&str> for StorageError {
    fn from(s: &str) -> Self {
        StorageError::Message(s.to_string())
    }
}

/// 方便 Tauri command 把 StorageError 转成 String（前端要 String 错误）
impl From<StorageError> for String {
    fn from(e: StorageError) -> String {
        e.to_string()
    }
}
