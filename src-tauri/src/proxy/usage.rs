use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 从 SSE 流中提取的 usage 数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub model: String,
    pub finish_reason: String,
}

/// SSE 事件解析器，从字节块中提取 Anthropic/OpenAI 格式的 usage 数据
pub struct UsageExtractor {
    /// 累积的 usage 快照
    pub snapshot: UsageSnapshot,
    /// 是否为 Anthropic 格式（/v1/messages）
    is_anthropic: bool,
}

/// 快速字节级扫描：检查 data 中是否包含 usage 相关关键字
/// 避免对每个 chunk 都做 JSON 解析
fn contains_usage_hint(data: &str) -> bool {
    // Anthropic 事件: 包含 "usage" 或 "input_tokens" 或 "output_tokens"
    // OpenAI 事件: 包含 "usage" 或 "prompt_tokens" 或 "completion_tokens"
    // 也匹配 "message_start" / "message_delta" / "message_stop"（Anthropic 信封事件）
    data.contains("\"usage\"")
        || data.contains("\"input_tokens\"")
        || data.contains("\"output_tokens\"")
        || data.contains("\"prompt_tokens\"")
        || data.contains("\"completion_tokens\"")
        || data.contains("\"message_start\"")
        || data.contains("\"message_delta\"")
        || data.contains("\"message_stop\"")
}

impl UsageExtractor {
    pub fn new(endpoint: &str) -> Self {
        Self {
            snapshot: UsageSnapshot::default(),
            is_anthropic: endpoint.contains("/messages"),
        }
    }

    /// 处理一个字节块，提取 usage 数据
    /// 优化：先做字节级快速扫描，只在检测到 usage 关键字时才解析 JSON
    pub fn process_chunk(&mut self, chunk: &[u8]) {
        let text = match std::str::from_utf8(chunk) {
            Ok(s) => s,
            Err(_) => return,
        };

        let mut found_sse = false;
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                found_sse = true;
                if data == "[DONE]" {
                    continue;
                }
                if !contains_usage_hint(data) {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    if self.is_anthropic {
                        self.parse_anthropic_event(&json);
                    } else {
                        self.parse_openai_event(&json);
                    }
                }
            }
        }

        // 非流式响应：整个 chunk 是一个 JSON 对象，无 SSE data: 前缀
        if !found_sse && contains_usage_hint(text) {
            if let Ok(json) = serde_json::from_str::<Value>(text) {
                if self.is_anthropic {
                    self.parse_non_stream_anthropic(&json);
                } else {
                    self.parse_openai_event(&json);
                }
            }
        }
    }

    /// 解析 Anthropic SSE 事件
    ///
    /// message_start: { message: { usage: { input_tokens, cache_read_input_tokens, cache_creation_input_tokens } } }
    /// message_delta: { usage: { output_tokens, input_tokens?, cache_read_input_tokens?, cache_creation_input_tokens? } }
    fn parse_anthropic_event(&mut self, json: &Value) {
        let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match event_type {
            "message_start" => {
                if let Some(usage) = json.pointer("/message/usage") {
                    self.snapshot.input_tokens =
                        usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    self.snapshot.cache_read_tokens = usage
                        .get("cache_read_input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    self.snapshot.cache_creation_tokens = usage
                        .get("cache_creation_input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                }
                if let Some(model) = json.pointer("/message/model").and_then(|v| v.as_str()) {
                    self.snapshot.model = model.to_string();
                }
            }
            "message_delta" => {
                // output_tokens 是必有的
                if let Some(ot) = json.pointer("/usage/output_tokens").and_then(|v| v.as_u64()) {
                    self.snapshot.output_tokens = ot;
                }
                // 某些供应商会在 message_delta 中覆盖 input_tokens 等
                if let Some(it) = json.pointer("/usage/input_tokens").and_then(|v| v.as_u64()) {
                    self.snapshot.input_tokens = it;
                }
                if let Some(cr) = json
                    .pointer("/usage/cache_read_input_tokens")
                    .and_then(|v| v.as_u64())
                {
                    self.snapshot.cache_read_tokens = cr;
                }
                if let Some(cc) = json
                    .pointer("/usage/cache_creation_input_tokens")
                    .and_then(|v| v.as_u64())
                {
                    self.snapshot.cache_creation_tokens = cc;
                }
                if let Some(fr) = json
                    .pointer("/delta/stop_reason")
                    .and_then(|v| v.as_str())
                {
                    self.snapshot.finish_reason = fr.to_string();
                }
            }
            "message_stop" => {
                // 最终事件，snapshot 已经是最新的
            }
            _ => {}
        }
    }

    /// 解析 Anthropic 非流式 JSON 响应
    ///
    /// { usage: { input_tokens, output_tokens }, model, stop_reason }
    fn parse_non_stream_anthropic(&mut self, json: &Value) {
        if let Some(model) = json.get("model").and_then(|v| v.as_str()) {
            self.snapshot.model = model.to_string();
        }
        if let Some(usage) = json.get("usage") {
            self.snapshot.input_tokens = usage
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            self.snapshot.output_tokens = usage
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            self.snapshot.cache_read_tokens = usage
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            self.snapshot.cache_creation_tokens = usage
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
        }
        if let Some(sr) = json.get("stop_reason").and_then(|v| v.as_str()) {
            self.snapshot.finish_reason = sr.to_string();
        }
    }

    /// 解析 OpenAI SSE 事件
    ///
    /// 最后一个 chunk 包含 usage 字段:
    /// { usage: { prompt_tokens, completion_tokens, total_tokens } }
    fn parse_openai_event(&mut self, json: &Value) {
        // 模型信息
        if let Some(model) = json.get("model").and_then(|v| v.as_str()) {
            self.snapshot.model = model.to_string();
        }

        // usage 字段（通常在最后一个 chunk）
        if let Some(usage) = json.get("usage") {
            // OpenAI Chat: prompt_tokens / completion_tokens
            // OpenAI Responses: input_tokens / output_tokens
            self.snapshot.input_tokens = usage
                .get("prompt_tokens")
                .or_else(|| usage.get("input_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(self.snapshot.input_tokens);
            self.snapshot.output_tokens = usage
                .get("completion_tokens")
                .or_else(|| usage.get("output_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(self.snapshot.output_tokens);
            // OpenAI 标准格式: usage.prompt_tokens_details.cached_tokens
            if let Some(cached) = usage
                .get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(|v| v.as_u64())
            {
                self.snapshot.cache_read_tokens = cached;
            }
        }

        // finish_reason
        if let Some(fr) = json.pointer("/choices/0/finish_reason").and_then(|v| v.as_str()) {
            self.snapshot.finish_reason = fr.to_string();
        }
    }

    /// 是否有有效的 usage 数据
    pub fn has_usage(&self) -> bool {
        self.snapshot.input_tokens > 0 || self.snapshot.output_tokens > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_message_start() {
        let mut extractor = UsageExtractor::new("/v1/messages");
        let chunk = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-sonnet-4-20250514\",\"usage\":{\"input_tokens\":100,\"cache_read_input_tokens\":20,\"cache_creation_input_tokens\":10}}}\n\n";
        extractor.process_chunk(chunk);
        assert_eq!(extractor.snapshot.input_tokens, 100);
        assert_eq!(extractor.snapshot.cache_read_tokens, 20);
        assert_eq!(extractor.snapshot.cache_creation_tokens, 10);
        assert_eq!(extractor.snapshot.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_anthropic_message_delta() {
        let mut extractor = UsageExtractor::new("/v1/messages");
        // 先处理 message_start
        let start = b"data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":100}}}\n\n";
        extractor.process_chunk(start);
        // 再处理 message_delta
        let delta = b"data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":50}}\n\n";
        extractor.process_chunk(delta);
        assert_eq!(extractor.snapshot.output_tokens, 50);
        assert_eq!(extractor.snapshot.finish_reason, "end_turn");
        assert_eq!(extractor.snapshot.input_tokens, 100);
    }

    #[test]
    fn test_openai_usage() {
        let mut extractor = UsageExtractor::new("/v1/chat/completions");
        let chunk = b"data: {\"id\":\"chatcmpl-1\",\"model\":\"gpt-4o\",\"choices\":[{\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":80,\"completion_tokens\":30,\"total_tokens\":110}}\n\ndata: [DONE]\n\n";
        extractor.process_chunk(chunk);
        assert_eq!(extractor.snapshot.input_tokens, 80);
        assert_eq!(extractor.snapshot.output_tokens, 30);
        assert_eq!(extractor.snapshot.model, "gpt-4o");
        assert_eq!(extractor.snapshot.finish_reason, "stop");
    }

    #[test]
    fn test_has_usage() {
        let mut extractor = UsageExtractor::new("/v1/messages");
        assert!(!extractor.has_usage());
        extractor.snapshot.input_tokens = 10;
        assert!(extractor.has_usage());
    }

    #[test]
    fn test_stats_counters() {
        let mut extractor = UsageExtractor::new("/v1/messages");

        // chunk 1: 纯内容数据（无 usage 关键字）→ 快速跳过
        let content = b"data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\n";
        extractor.process_chunk(content);
        assert!(!extractor.has_usage());

        // chunk 2: message_start（含 usage）→ 提取
        let start = b"data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":100}}}\n\n";
        extractor.process_chunk(start);
        assert!(extractor.has_usage());
        assert_eq!(extractor.snapshot.input_tokens, 100);

        // chunk 3: message_delta（含 usage）→ 更新
        let delta = b"data: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":50}}\n\n";
        extractor.process_chunk(delta);
        assert_eq!(extractor.snapshot.output_tokens, 50);
    }
}
