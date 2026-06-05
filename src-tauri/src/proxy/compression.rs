use only_cc_lite::ccr::backends::CcrBackendConfig;
use only_cc_lite::ccr::CcrStore;
use only_cc_lite::{compress_request, Provider};

/// 压缩结果（简化版，供代理引擎使用）
#[derive(Debug, Clone)]
pub struct CompressResult {
    /// 压缩后的请求体 bytes
    pub body: Vec<u8>,
    /// 节省的 token 数
    pub tokens_saved: u64,
    /// 节省的字节数
    pub bytes_saved: u64,
    /// 使用的压缩策略列表
    pub strategies: Vec<String>,
}

/// 压缩引擎，封装 only-cc-lite
pub struct CompressionEngine {
    ccr_store: Box<dyn CcrStore>,
}

impl CompressionEngine {
    /// 使用 in-memory CCR 后端创建（测试用）
    pub fn new_in_memory() -> Self {
        let config = CcrBackendConfig::in_memory_default();
        let ccr_store = only_cc_lite::ccr::backends::from_config(&config)
            .expect("in-memory CCR init should not fail");
        Self { ccr_store }
    }

    /// 使用 SQLite CCR 后端创建（生产用）
    pub fn new_sqlite(path: &std::path::Path) -> Result<Self, String> {
        let config = CcrBackendConfig::sqlite_default(path.to_path_buf());
        let ccr_store = only_cc_lite::ccr::backends::from_config(&config)
            .map_err(|e| format!("CCR SQLite init failed: {}", e))?;
        Ok(Self { ccr_store })
    }

    /// 压缩请求体
    ///
    /// - body: 原始请求 JSON bytes
    /// - endpoint: 路径，用于推断 provider 格式
    /// - model: 模型 ID
    ///
    /// 压缩失败时返回原始 body（容错降级）
    pub fn compress(&self, body: &[u8], endpoint: &str, model: &str) -> CompressResult {
        let provider = endpoint_to_provider(endpoint);

        match compress_request(body, provider, model, Some(self.ccr_store.as_ref())) {
            Ok(outcome) => {
                let compressed_body = match outcome.body {
                    Some(b) => b,
                    None => body.to_vec(),
                };
                CompressResult {
                    body: compressed_body,
                    tokens_saved: outcome.tokens_saved as u64,
                    bytes_saved: outcome.bytes_saved as u64,
                    strategies: outcome.strategies.into_iter().map(|s| s.to_string()).collect(),
                }
            }
            Err(e) => {
                log::warn!("context compression failed, using original body: {}", e);
                CompressResult {
                    body: body.to_vec(),
                    tokens_saved: 0,
                    bytes_saved: 0,
                    strategies: vec![],
                }
            }
        }
    }
}

/// 根据端点路径推断 only-cc-lite 的 Provider 类型
fn endpoint_to_provider(endpoint: &str) -> Provider {
    if endpoint.contains("/messages") {
        Provider::Anthropic
    } else if endpoint.contains("/responses") {
        Provider::OpenAiResponses
    } else {
        Provider::OpenAiChat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_to_provider() {
        assert_eq!(endpoint_to_provider("/v1/messages"), Provider::Anthropic);
        assert_eq!(
            endpoint_to_provider("/v1/chat/completions"),
            Provider::OpenAiChat
        );
        assert_eq!(
            endpoint_to_provider("/v1/responses"),
            Provider::OpenAiResponses
        );
        assert_eq!(endpoint_to_provider("/chat/completions"), Provider::OpenAiChat);
    }

    #[test]
    fn test_compress_openai_with_large_content() {
        let engine = CompressionEngine::new_in_memory();

        let large_array: Vec<String> = (0..100).map(|i| format!("item_{}", i)).collect();
        let body = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {
                    "role": "user",
                    "content": format!(
                        "Here is the data: {}",
                        serde_json::to_string(&large_array).unwrap()
                    )
                }
            ]
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();

        let result = engine.compress(&body_bytes, "/v1/chat/completions", "gpt-4o");

        assert!(!result.body.is_empty());
        let _: serde_json::Value = serde_json::from_slice(&result.body).unwrap();
    }

    #[test]
    fn test_compress_anthropic_format() {
        let engine = CompressionEngine::new_in_memory();

        let body = serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "messages": [
                {
                    "role": "user",
                    "content": "Hello, this is a simple test message."
                }
            ]
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();

        let result =
            engine.compress(&body_bytes, "/v1/messages", "claude-sonnet-4-20250514");

        assert!(!result.body.is_empty());
        let _: serde_json::Value = serde_json::from_slice(&result.body).unwrap();
    }

    #[test]
    fn test_compress_passthrough_on_small_body() {
        let engine = CompressionEngine::new_in_memory();

        let body = serde_json::json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}]
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();
        let original_len = body_bytes.len();

        let result = engine.compress(&body_bytes, "/v1/chat/completions", "gpt-4o");

        assert!(!result.body.is_empty());
        assert!(result.body.len() <= original_len + 100);
    }

    #[test]
    fn test_compress_invalid_json_does_not_panic() {
        let engine = CompressionEngine::new_in_memory();

        let body = b"this is not json";

        let result = engine.compress(body, "/v1/chat/completions", "gpt-4o");
        assert_eq!(result.body, body);
        assert_eq!(result.tokens_saved, 0);
    }

    #[test]
    fn test_compress_logs_strategies() {
        let engine = CompressionEngine::new_in_memory();

        let log_content = (0..50)
            .map(|i| format!("[2026-01-01 12:00:{:02}] INFO processing item {}", i, i))
            .collect::<Vec<_>>()
            .join("\n");

        let body = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "user", "content": format!("Analyze these logs:\n{}", log_content)}
            ]
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();

        let result = engine.compress(&body_bytes, "/v1/chat/completions", "gpt-4o");

        assert!(!result.body.is_empty());
    }
}
