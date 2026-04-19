//! TextView 云端认证（email OTP + JWT）— R3.1 基础。
//!
//! 架构分层（关键：async HTTP 和 sync DB 不混）：
//! - http 层：async 网络请求，0 DB 接触
//! - persist 层：sync 操作 DB + keyring，不跨 await
//! - Command 编排：先 async HTTP → 拿结果后同步 persist
//!
//! 这是因为 rusqlite::Connection 不是 Send，MutexGuard 不能跨 await 点。
//!
//! 数据保存位置：
//! - refresh_token → OS keyring（Windows Credential Manager / macOS Keychain）
//! - access_token → 进程内存（Mutex，重启丢失靠 refresh 重建）
//! - user_id / email → clipboard_local.settings 表（方便 UI 判登录态）

use std::sync::Mutex;

use once_cell::sync::Lazy;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub mod http;

pub fn api_base() -> String {
    std::env::var("TEAMO_API_BASE").unwrap_or_else(|_| "https://textview.cn".to_string())
}

const KEYRING_SERVICE: &str = "cn.textview.teamo";
const KEYRING_USER_REFRESH: &str = "refresh_token";

static ACCESS_TOKEN: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthState {
    pub logged_in: bool,
    pub user: Option<AuthUser>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyOtpResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: AuthUser,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    access_token: String,
    refresh_token: String,
}

// ── Keyring（sync）─────────────────────────────────

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER_REFRESH)
        .map_err(|e| format!("keyring init: {e}"))
}

pub fn save_refresh_token(token: &str) -> Result<(), String> {
    keyring_entry()?
        .set_password(token)
        .map_err(|e| format!("keyring save: {e}"))
}

pub fn load_refresh_token() -> Option<String> {
    keyring_entry().ok()?.get_password().ok()
}

pub fn delete_refresh_token() -> Result<(), String> {
    match keyring_entry()?.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keyring delete: {e}")),
    }
}

// ── Access token（sync，进程内）─────────────────────

pub fn get_access_token() -> Option<String> {
    ACCESS_TOKEN.lock().ok()?.clone()
}

pub fn set_access_token(token: String) {
    if let Ok(mut g) = ACCESS_TOKEN.lock() {
        *g = Some(token);
    }
}

pub fn clear_access_token() {
    if let Ok(mut g) = ACCESS_TOKEN.lock() {
        *g = None;
    }
}

// ── User 信息存 settings（sync）─────────────────────

const SETTING_USER_ID: &str = "auth.user_id";
const SETTING_USER_EMAIL: &str = "auth.user_email";

pub fn save_user_sync(conn: &Connection, user: &AuthUser) -> rusqlite::Result<()> {
    crate::storage::repository::set_setting(conn, SETTING_USER_ID, Some(&user.id))?;
    crate::storage::repository::set_setting(conn, SETTING_USER_EMAIL, Some(&user.email))?;
    Ok(())
}

pub fn load_user_sync(conn: &Connection) -> Option<AuthUser> {
    let id = crate::storage::repository::get_setting(conn, SETTING_USER_ID).ok()??;
    let email = crate::storage::repository::get_setting(conn, SETTING_USER_EMAIL).ok()??;
    Some(AuthUser { id, email })
}

pub fn clear_user_sync(conn: &Connection) -> rusqlite::Result<()> {
    crate::storage::repository::set_setting(conn, SETTING_USER_ID, None)?;
    crate::storage::repository::set_setting(conn, SETTING_USER_EMAIL, None)?;
    Ok(())
}

// ── HTTP 层（async，不碰 DB）─────────────────────────

pub async fn send_otp_http(email: &str) -> Result<(), String> {
    let url = format!("{}/api/auth/send-otp", api_base());
    let client = http::http_client();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await
        .map_err(|e| format!("请求失败：{e}"))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Err(format!("发送验证码失败（{status}）：{text}"))
    }
}

pub async fn verify_otp_http(email: &str, code: &str) -> Result<VerifyOtpResponse, String> {
    let url = format!("{}/api/auth/verify-otp", api_base());
    let client = http::http_client();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "email": email, "code": code }))
        .send()
        .await
        .map_err(|e| format!("请求失败：{e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("验证码错误或已过期（{status}）：{text}"));
    }

    resp.json::<VerifyOtpResponse>()
        .await
        .map_err(|e| format!("响应解析失败：{e}"))
}

/// 用 refresh_token 换新 access（纯 async + 纯 keyring，不碰 DB）
pub async fn refresh() -> Result<(), String> {
    let refresh_token =
        load_refresh_token().ok_or_else(|| "未登录（keyring 无 refresh_token）".to_string())?;

    let url = format!("{}/api/auth/refresh", api_base());
    let client = http::http_client();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .map_err(|e| format!("refresh 请求失败：{e}"))?;

    if resp.status() == 401 {
        return Err("refresh_token 已失效".to_string());
    }
    if !resp.status().is_success() {
        return Err(format!("refresh 失败：{}", resp.status()));
    }

    let body: RefreshResponse = resp
        .json()
        .await
        .map_err(|e| format!("refresh 响应解析失败：{e}"))?;

    save_refresh_token(&body.refresh_token)?;
    set_access_token(body.access_token);
    Ok(())
}

// ── 登录态（sync 查询）─────────────────────────────

pub fn current_auth_state(conn: &Connection) -> AuthState {
    let user = load_user_sync(conn);
    let has_refresh = load_refresh_token().is_some();
    let logged_in = user.is_some() && has_refresh;
    AuthState {
        logged_in,
        user: if logged_in { user } else { None },
    }
}

/// 启动时 hydrate access_token（async，但不需要 DB）
pub async fn hydrate_on_startup() {
    if load_refresh_token().is_some() {
        match refresh().await {
            Ok(()) => tracing::info!("Startup token refresh ok"),
            Err(e) => tracing::info!("Startup token refresh failed: {e}"),
        }
    }
}
