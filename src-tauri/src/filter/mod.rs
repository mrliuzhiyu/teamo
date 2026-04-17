//! 端侧闸门（filter-engine）— 决定剪切板内容能否进入 pending_upload 状态。
//!
//! Phase 1（当前）：敏感数据检测（6 种类型）
//! Phase 2 留：App 黑白名单（从 bundle_id / exe_name 判断来源）
//! Phase 3 留：域名规则（URL 内容按 YAML 规则分类）
//!
//! 设计原则：
//! - 规则即数据（Phase 2/3 用 DB 存用户/云端规则）
//! - 高精度优先于高召回（误报杀伤大于漏报）
//! - 单次调用 < 1 ms（capture loop 每 500ms 跑一次，不能阻塞）

pub mod entropy;
pub mod idcard;
pub mod luhn;
pub mod sensitive;

use serde::{Deserialize, Serialize};

/// 敏感数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveType {
    Password,
    Token,
    CreditCard,
    IdCard,
    Phone,
    Email,
}

impl SensitiveType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SensitiveType::Password => "password",
            SensitiveType::Token => "token",
            SensitiveType::CreditCard => "credit_card",
            SensitiveType::IdCard => "id_card",
            SensitiveType::Phone => "phone",
            SensitiveType::Email => "email",
        }
    }
}

/// 闸门结果。写入 clipboard_local 的 state/blocked_reason/sensitive_type 列。
#[derive(Debug, Clone, Serialize)]
pub struct FilterDecision {
    /// `state` 列值："captured"（可上云）/ "local_only"（闸门拦截，永不上云）
    pub state: String,
    /// 人可读的拦截原因（比如 "sensitive:password"）。captured 时为 None。
    pub blocked_reason: Option<String>,
    /// 具体的敏感类型（password/token/...），未命中为 None。
    pub sensitive_type: Option<String>,
}

impl FilterDecision {
    pub fn captured() -> Self {
        Self {
            state: "captured".to_string(),
            blocked_reason: None,
            sensitive_type: None,
        }
    }

    pub fn blocked_sensitive(kind: SensitiveType) -> Self {
        Self {
            state: "local_only".to_string(),
            blocked_reason: Some(format!("sensitive:{}", kind.as_str())),
            sensitive_type: Some(kind.as_str().to_string()),
        }
    }
}

/// 对文本内容应用所有过滤器。
///
/// Phase 1 只做敏感检测；Phase 2/3 会在同一入口追加 App 黑白名单 + 域名规则。
pub fn apply_filters(content: &str) -> FilterDecision {
    if let Some(kind) = sensitive::detect(content) {
        return FilterDecision::blocked_sensitive(kind);
    }
    FilterDecision::captured()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_filters_clean_text() {
        let d = apply_filters("just some random notes about my day");
        assert_eq!(d.state, "captured");
        assert!(d.blocked_reason.is_none());
        assert!(d.sensitive_type.is_none());
    }

    #[test]
    fn test_apply_filters_sensitive_token() {
        let d = apply_filters("sk-abc123def456ghi789jklmnopqrstuvwx");
        assert_eq!(d.state, "local_only");
        assert_eq!(d.blocked_reason.as_deref(), Some("sensitive:token"));
        assert_eq!(d.sensitive_type.as_deref(), Some("token"));
    }

    #[test]
    fn test_apply_filters_sensitive_credit_card() {
        let d = apply_filters("4111 1111 1111 1111");
        assert_eq!(d.state, "local_only");
        assert_eq!(d.sensitive_type.as_deref(), Some("credit_card"));
    }
}
