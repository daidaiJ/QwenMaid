/// 鉴权策略
#[derive(Debug, Clone, PartialEq)]
pub enum AuthStrategy {
    /// OpenAI: 直接透传 Bearer
    Passthrough,
    /// Anthropic: Bearer → x-api-key + anthropic-version
    BearerToApiKey,
    /// Gemini: Bearer → x-goog-api-key
    BearerToGoogleKey,
    /// 自定义头: 从 x-api-key / Authorization 中提取 key，注入到自定义头
    CustomHeader(String),
}

impl AuthStrategy {
    pub fn from_auth_type(auth_type: &str) -> Self {
        match auth_type {
            "anthropic" => Self::BearerToApiKey,
            "gemini" => Self::BearerToGoogleKey,
            _ => Self::Passthrough,
        }
    }

    /// 如果 provider 配置了 auth_header，优先使用自定义策略
    pub fn with_override(base: Self, auth_header: Option<&str>) -> Self {
        match auth_header {
            Some(h) if !h.is_empty() => Self::CustomHeader(h.to_string()),
            _ => base,
        }
    }
}

/// 从入站头中智能提取 API key
/// 优先级: x-api-key > Authorization: Bearer
pub fn extract_api_key(headers: &[(String, String)]) -> Option<String> {
    // 先找 x-api-key
    if let Some(val) = headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == "x-api-key")
        .map(|(_, v)| v.clone())
    {
        if !val.is_empty() {
            return Some(val);
        }
    }
    // 再找 Authorization: Bearer
    extract_bearer_token(headers)
}

/// 从入站 Authorization 头提取 key 值
pub fn extract_bearer_token(headers: &[(String, String)]) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == "authorization")
        .and_then(|(_, v)| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

/// 转换鉴权头，返回新的 (key, value) 列表
///
/// 对于 CustomHeader 策略：
/// - 从入站头中智能提取 key（x-api-key 或 Authorization）
/// - 如果提供了 api_key 参数，优先使用参数值
/// - 注入到自定义头名称
pub fn transform_auth_headers(
    headers: &[(String, String)],
    strategy: &AuthStrategy,
    api_key: &str,
) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = headers
        .iter()
        .filter(|(k, _)| {
            let lower = k.to_lowercase();
            lower != "authorization" && lower != "x-api-key"
        })
        .cloned()
        .collect();

    let key = if api_key.is_empty() {
        extract_api_key(headers).unwrap_or_default()
    } else {
        api_key.to_string()
    };

    match strategy {
        AuthStrategy::Passthrough => {
            if !key.is_empty() {
                result.push(("authorization".into(), format!("Bearer {}", key)));
            }
        }
        AuthStrategy::BearerToApiKey => {
            if !key.is_empty() {
                result.push(("x-api-key".into(), key));
            }
            result.push(("anthropic-version".into(), "2023-06-01".into()));
        }
        AuthStrategy::BearerToGoogleKey => {
            if !key.is_empty() {
                result.push(("x-goog-api-key".into(), key));
            }
        }
        AuthStrategy::CustomHeader(header_name) => {
            if !key.is_empty() {
                result.push((header_name.clone(), key));
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_headers(auth: &str) -> Vec<(String, String)> {
        vec![("authorization".into(), format!("Bearer {}", auth))]
    }

    fn make_x_api_key_headers(key: &str) -> Vec<(String, String)> {
        vec![("x-api-key".into(), key.into())]
    }

    #[test]
    fn test_passthrough() {
        let h = transform_auth_headers(
            &make_headers("sk-123"),
            &AuthStrategy::Passthrough,
            "sk-123",
        );
        assert!(h.iter().any(|(k, v)| k == "authorization" && v == "Bearer sk-123"));
    }

    #[test]
    fn test_anthropic_conversion() {
        let h = transform_auth_headers(
            &make_headers("sk-ant-xxx"),
            &AuthStrategy::BearerToApiKey,
            "sk-ant-xxx",
        );
        assert!(h.iter().any(|(k, v)| k == "x-api-key" && v == "sk-ant-xxx"));
        assert!(h.iter().any(|(k, v)| k == "anthropic-version" && v == "2023-06-01"));
        assert!(!h.iter().any(|(k, _)| k == "authorization"));
    }

    #[test]
    fn test_gemini_conversion() {
        let h = transform_auth_headers(
            &make_headers("AIza-xxx"),
            &AuthStrategy::BearerToGoogleKey,
            "AIza-xxx",
        );
        assert!(h.iter().any(|(k, v)| k == "x-goog-api-key" && v == "AIza-xxx"));
        assert!(!h.iter().any(|(k, _)| k == "authorization"));
    }

    #[test]
    fn test_custom_header_from_bearer() {
        let h = transform_auth_headers(
            &make_headers("sk-mimo-xxx"),
            &AuthStrategy::CustomHeader("mimo-api-key".into()),
            "sk-mimo-xxx",
        );
        assert!(h.iter().any(|(k, v)| k == "mimo-api-key" && v == "sk-mimo-xxx"));
        assert!(!h.iter().any(|(k, _)| k == "authorization"));
        assert!(!h.iter().any(|(k, _)| k == "x-api-key"));
    }

    #[test]
    fn test_custom_header_from_x_api_key() {
        // 入站头只有 x-api-key，没有 Authorization
        let h = transform_auth_headers(
            &make_x_api_key_headers("sk-mimo-xxx"),
            &AuthStrategy::CustomHeader("mimo-api-key".into()),
            "",
        );
        assert!(h.iter().any(|(k, v)| k == "mimo-api-key" && v == "sk-mimo-xxx"));
    }

    #[test]
    fn test_custom_header_prefers_param_over_inbound() {
        // api_key 参数优先于入站头提取
        let h = transform_auth_headers(
            &make_headers("old-key"),
            &AuthStrategy::CustomHeader("x-custom".into()),
            "new-key",
        );
        assert!(h.iter().any(|(k, v)| k == "x-custom" && v == "new-key"));
    }

    #[test]
    fn test_extract_api_key_prefers_x_api_key() {
        let mut headers = make_headers("from-bearer");
        headers.push(("x-api-key".into(), "from-x-api-key".into()));
        assert_eq!(extract_api_key(&headers), Some("from-x-api-key".into()));
    }

    #[test]
    fn test_extract_api_key_falls_back_to_bearer() {
        let headers = make_headers("from-bearer");
        assert_eq!(extract_api_key(&headers), Some("from-bearer".into()));
    }

    #[test]
    fn test_extract_bearer() {
        let h = make_headers("sk-test");
        assert_eq!(extract_bearer_token(&h), Some("sk-test".into()));
        assert_eq!(extract_bearer_token(&[]), None);
    }

    #[test]
    fn test_preserves_other_headers() {
        let mut h = make_headers("sk-123");
        h.push(("content-type".into(), "application/json".into()));
        let result = transform_auth_headers(&h, &AuthStrategy::BearerToApiKey, "sk-123");
        assert!(result.iter().any(|(k, _)| k == "content-type"));
    }

    #[test]
    fn test_with_override() {
        let base = AuthStrategy::from_auth_type("openai");
        let overridden = AuthStrategy::with_override(base.clone(), Some("x-custom-key"));
        assert_eq!(overridden, AuthStrategy::CustomHeader("x-custom-key".into()));

        let unchanged = AuthStrategy::with_override(base.clone(), None);
        assert_eq!(unchanged, AuthStrategy::Passthrough);
    }
}
