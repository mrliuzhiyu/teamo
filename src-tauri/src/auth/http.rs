//! TextView API HTTP 客户端 — 带 access_token + 自动 refresh + 重放。
//!
//! 用法：
//!   let client = http_client();
//!   let resp = authed_post(client, "/api/memos/batch", json!({...})).await?;
//!
//! 401 响应会尝试 refresh 一次再重放；失败则返 Err 让调用方提示用户重新登录。

use once_cell::sync::Lazy;
use reqwest::Client;

static CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(format!("Teamo/{} (Windows)", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("reqwest client build")
});

pub fn http_client() -> &'static Client {
    &CLIENT
}

/// 带认证的 JSON POST；401 自动 refresh + 重放一次。
/// 注：verify_otp / send_otp / refresh 本身不用此函数（无需 access_token）
pub async fn authed_post<T: serde::Serialize>(
    path: &str,
    body: &T,
) -> Result<reqwest::Response, String> {
    let url = format!("{}{}", super::api_base(), path);
    let token = super::get_access_token()
        .ok_or_else(|| "未登录或 access_token 丢失".to_string())?;

    let resp = CLIENT
        .post(&url)
        .bearer_auth(&token)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("HTTP 请求失败：{e}"))?;

    if resp.status() != 401 {
        return Ok(resp);
    }

    // access 过期，尝试 refresh + 重放一次
    tracing::info!("401 on {path}, attempting refresh");
    super::refresh().await?;
    let new_token = super::get_access_token().ok_or_else(|| "refresh 后未拿到 access_token")?;

    CLIENT
        .post(&url)
        .bearer_auth(&new_token)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("HTTP 重放失败：{e}"))
}

// authed_multipart 延后到 R3.3（图片上传时再补）：需要 reqwest 开 "multipart" feature
