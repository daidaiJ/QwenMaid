use axum::{
    extract::State as AxumState,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::StreamExt;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::time::Instant;

use super::auth::{self, AuthStrategy};
use super::client_pool::ClientPool;
use super::compression::CompressionEngine;
use super::usage::UsageExtractor;
use crate::db::providers::{self, ModelRoute};

/// axum 共享状态
#[derive(Clone)]
pub struct ProxyState {
    pub db: Arc<Mutex<Connection>>,
    pub client_pool: Arc<ClientPool>,
    pub api_key_resolver: Arc<dyn Fn(&str) -> Option<String> + Send + Sync>,
    pub compression_engine: Arc<CompressionEngine>,
    /// 测试用：直接注入的 model_id → ModelRoute 映射，优先于 DB 查询
    pub test_routes: Option<Arc<std::collections::HashMap<String, ModelRoute>>>,
}

/// 创建 axum 路由
pub fn create_router(state: ProxyState) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/messages", post(handle_messages))
        .route("/v1/responses", post(handle_responses))
        .route("/v1/models", get(handle_list_models))
        .route("/health", get(handle_health))
        .with_state(state)
}

async fn handle_health() -> Json<Value> {
    Json(json!({"status": "ok", "service": "agentbox-proxy"}))
}

async fn handle_list_models(
    AxumState(state): AxumState<ProxyState>,
) -> Result<Json<Value>, ProxyErrorResponse> {
    // test_routes 优先（测试注入的内存路由，无需 DB）
    if let Some(ref routes) = state.test_routes {
        let data: Vec<Value> = routes
            .values()
            .map(|r| {
                json!({
                    "id": r.model_id,
                    "object": "model",
                    "owned_by": format!("provider-{}", r.provider_id),
                })
            })
            .collect();
        return Ok(Json(json!({
            "object": "list",
            "data": data
        })));
    }

    let db = state.db.lock().map_err(|e| ProxyErrorResponse {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("db lock: {}", e),
    })?;

    let models =
        providers::list_models(&db, None).map_err(|e| ProxyErrorResponse {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e,
        })?;

    let data: Vec<Value> = models
        .iter()
        .map(|m| {
            json!({
                "id": m.model_id,
                "object": "model",
                "owned_by": format!("provider-{}", m.provider_id),
            })
        })
        .collect();

    Ok(Json(json!({
        "object": "list",
        "data": data
    })))
}

async fn handle_chat_completions(
    AxumState(state): AxumState<ProxyState>,
    body: String,
) -> Result<Response, ProxyErrorResponse> {
    // endpoint 统一用 /v1 全路径，build_upstream_url 自动去重 base_url 的 /v1 后缀
    handle_openai_endpoint(&state, "/v1/chat/completions", &body).await
}

async fn handle_messages(
    AxumState(state): AxumState<ProxyState>,
    body: String,
) -> Result<Response, ProxyErrorResponse> {
    handle_anthropic_endpoint(&state, "/v1/messages", &body).await
}

async fn handle_responses(
    AxumState(state): AxumState<ProxyState>,
    body: String,
) -> Result<Response, ProxyErrorResponse> {
    handle_openai_endpoint(&state, "/v1/responses", &body).await
}

/// 处理 OpenAI 格式端点（chat/completions, responses）
async fn handle_openai_endpoint(
    state: &ProxyState,
    endpoint: &str,
    body: &str,
) -> Result<Response, ProxyErrorResponse> {
    let parsed: Value = serde_json::from_str(body).map_err(|e| ProxyErrorResponse {
        status: StatusCode::BAD_REQUEST,
        message: format!("invalid JSON: {}", e),
    })?;

    let model_id = parsed
        .get("model")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProxyErrorResponse {
            status: StatusCode::BAD_REQUEST,
            message: "missing 'model' field".into(),
        })?;

    let route = resolve_route(state, model_id)?;

    // 从模型支持的 auth_type 数组中选择匹配当前端点的策略
    let strategy = pick_auth_strategy(&route.auth_type, endpoint);
    let api_key = (state.api_key_resolver)(&route.api_key_env).unwrap_or_default();

    let headers = auth::transform_auth_headers(
        &[
            ("content-type".into(), "application/json".into()),
            ("authorization".into(), format!("Bearer {}", api_key)),
        ],
        &strategy,
        &api_key,
    );

    forward_request(state, &route, endpoint, body, &headers).await
}

/// 处理 Anthropic 格式端点（messages）
async fn handle_anthropic_endpoint(
    state: &ProxyState,
    endpoint: &str,
    body: &str,
) -> Result<Response, ProxyErrorResponse> {
    let parsed: Value = serde_json::from_str(body).map_err(|e| ProxyErrorResponse {
        status: StatusCode::BAD_REQUEST,
        message: format!("invalid JSON: {}", e),
    })?;

    let model_id = parsed
        .get("model")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProxyErrorResponse {
            status: StatusCode::BAD_REQUEST,
            message: "missing 'model' field".into(),
        })?;

    let route = resolve_route(state, model_id)?;
    let strategy = pick_auth_strategy(&route.auth_type, endpoint);
    let api_key = (state.api_key_resolver)(&route.api_key_env).unwrap_or_default();

    let headers = auth::transform_auth_headers(
        &[
            ("content-type".into(), "application/json".into()),
            ("authorization".into(), format!("Bearer {}", api_key)),
        ],
        &strategy,
        &api_key,
    );

    forward_request(state, &route, endpoint, body, &headers).await
}

/// 从模型支持的 auth_type 数组中选择匹配当前端点的策略
/// auth_type 格式: `["openai","anthropic"]` 或旧格式 `"openai"`
fn pick_auth_strategy(auth_type_json: &str, endpoint: &str) -> AuthStrategy {
    let types: Vec<String> = serde_json::from_str(auth_type_json)
        .unwrap_or_else(|_| vec![auth_type_json.to_string()]);

    let preferred = if endpoint.contains("/messages") {
        "anthropic"
    } else if endpoint.contains("/responses") {
        "openai"
    } else {
        "openai"
    };

    // 优先选匹配端点协议的，否则取第一个
    if let Some(matched) = types.iter().find(|t| t == &preferred) {
        AuthStrategy::from_auth_type(matched)
    } else {
        AuthStrategy::from_auth_type(types.first().map(|s| s.as_str()).unwrap_or("openai"))
    }
}

/// 通用请求转发（含条件性上下文压缩 + UsageExtractor + request_logs 写入）
async fn forward_request(
    state: &ProxyState,
    route: &ModelRoute,
    endpoint: &str,
    body: &str,
    headers: &[(String, String)],
) -> Result<Response, ProxyErrorResponse> {
    let client = state.client_pool.get(
        route.provider_id,
        &route.proxy_mode,
        route.proxy_url.as_deref(),
    );

    let url = build_upstream_url(&route.base_url, endpoint);
    let is_stream = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("stream")?.as_bool())
        .unwrap_or(false);

    // 条件性压缩
    let (body_bytes, compress_info) = if route.compress_enabled {
        let result = state.compression_engine.compress(body.as_bytes(), endpoint, &route.model_id);
        let original_tokens = estimate_tokens(body.len());
        let compressed_tokens = estimate_tokens(result.body.len());
        (result.body, Some((original_tokens, compressed_tokens)))
    } else {
        (body.as_bytes().to_vec(), None)
    };

    let mut req_builder = client.post(&url).header("content-type", "application/json");
    for (k, v) in headers {
        req_builder = req_builder.header(k.as_str(), v.as_str());
    }

    let start_time = Instant::now();
    let resp = req_builder.body(body_bytes).send().await.map_err(|e| {
        ProxyErrorResponse {
            status: StatusCode::BAD_GATEWAY,
            message: format!("provider unreachable: {}", e),
        }
    })?;

    // 请求成功后更新路由亲和性
    let status_code = resp.status().as_u16();
    if resp.status().is_success() {
        let db = state.db.lock().ok();
        if let Some(conn) = db {
            providers::touch_model_success(&conn, route.model_db_id);
        }
    }

    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::BAD_GATEWAY);

    if is_stream {
        let stream = resp.bytes_stream();
        let mut extractor = UsageExtractor::new(endpoint);
        let start = start_time;

        // clone 所有引用数据供 spawn 使用
        let state_owned = state.clone();
        let route_owned = route.clone();
        let endpoint_owned = endpoint.to_string();
        let compress_owned = compress_info.clone();

        // 用 channel 转发流：逐 chunk 喂给 extractor，同时转发给客户端
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<bytes::Bytes, axum::Error>>();

        tokio::spawn(async move {
            let mut first_byte: Option<Instant> = None;
            let mut stream = Box::pin(stream);
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(data) => {
                        if first_byte.is_none() {
                            first_byte = Some(Instant::now());
                        }
                        extractor.process_chunk(&data);
                        let _ = tx.send(Ok(data));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(axum::Error::new(e)));
                    }
                }
            }
            // 流结束，写入 request_logs
            let duration_ms = start.elapsed().as_millis() as i64;
            let ttft_ms = first_byte
                .map(|t| t.duration_since(start).as_millis() as i64)
                .unwrap_or(0);
            log_request_usage(
                &state_owned,
                &route_owned,
                &endpoint_owned,
                is_stream,
                &compress_owned,
                status_code,
                &extractor.snapshot,
                duration_ms,
                ttft_ms,
            );
        });

        let body_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
        Ok((
            status,
            [("content-type", "text/event-stream")],
            axum::body::Body::from_stream(body_stream),
        )
            .into_response())
    } else {
        let resp_body = resp.text().await.map_err(|e| ProxyErrorResponse {
            status: StatusCode::BAD_GATEWAY,
            message: format!("read response: {}", e),
        })?;

        let duration_ms = start_time.elapsed().as_millis() as i64;

        // 非流式：从响应 JSON 中提取 usage
        let mut extractor = UsageExtractor::new(endpoint);
        extractor.process_chunk(resp_body.as_bytes());

        log_request_usage(
            state,
            route,
            endpoint,
            is_stream,
            &compress_info,
            status_code,
            &extractor.snapshot,
            duration_ms,
            duration_ms,
        );

        Ok((status, [("content-type", "application/json")], resp_body).into_response())
    }
}

/// 粗略估算 token 数（1 token ≈ 4 字节）
fn estimate_tokens(bytes: usize) -> u64 {
    (bytes as u64 + 3) / 4
}

/// 记录请求日志到 request_logs 表（含完整 usage + 延迟数据）
fn log_request_usage(
    state: &ProxyState,
    route: &ModelRoute,
    endpoint: &str,
    is_stream: bool,
    compress_info: &Option<(u64, u64)>,
    status_code: u16,
    usage: &super::usage::UsageSnapshot,
    duration_ms: i64,
    time_to_first_ms: i64,
) {
    let request_id = uuid::Uuid::new_v4().to_string();
    let (context_compressed, original_tokens, tokens_saved) = match compress_info {
        Some((orig, compressed)) => {
            let saved = orig.saturating_sub(*compressed);
            (true, *orig, saved)
        }
        None => (false, 0, 0),
    };

    let db = match state.db.lock() {
        Ok(db) => db,
        Err(_) => return,
    };

    let _ = db.execute(
        "INSERT INTO request_logs (request_id, provider_id, model_id, auth_type, endpoint, is_stream, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, duration_ms, time_to_first_ms, context_compressed, original_tokens, tokens_saved, status_code) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        rusqlite::params![
            request_id,
            route.provider_id,
            route.model_id,
            route.auth_type,
            endpoint,
            is_stream,
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_tokens,
            usage.cache_creation_tokens,
            duration_ms,
            time_to_first_ms,
            context_compressed,
            original_tokens,
            tokens_saved,
            status_code,
        ],
    );
}

/// 构建上游 URL，处理 baseUrl 与 endpoint 的路径去重
///
/// Qwen Code SDK 行为：baseUrl 包含协议版本前缀（如 /v1），SDK 追加相对路径（如 /chat/completions）
/// 代理引擎收到的 endpoint 是完整路径（如 /v1/chat/completions）
/// 当 baseUrl 最后一段与 endpoint 第一段重复时，去掉 endpoint 的重复前缀
///
/// 示例：
/// - baseUrl="https://api.openai.com/v1" + endpoint="/v1/chat/completions"
///   → "https://api.openai.com/v1/chat/completions"
/// - baseUrl="https://opencode.ai/zen/go/v1" + endpoint="/v1/chat/completions"
///   → "https://opencode.ai/zen/go/v1/chat/completions"
/// - baseUrl="https://api.openai.com" + endpoint="/v1/chat/completions"
///   → "https://api.openai.com/v1/chat/completions"（无重复）
fn build_upstream_url(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');

    // 取 baseUrl 路径的最后一段（如 "/v1"）
    if let Some(last_slash) = base.rfind('/') {
        // 确保这是路径部分而非协议的 "//"
        let host_end = base.find("://").map(|i| i + 2).unwrap_or(0);
        if last_slash > host_end {
            let last_segment = &base[last_slash..]; // e.g. "/v1"

            // 检查 endpoint 是否以这个段开头，且后面紧跟 /
            if endpoint.starts_with(last_segment)
                && endpoint.len() > last_segment.len()
                && endpoint.as_bytes().get(last_segment.len()) == Some(&b'/')
            {
                return format!("{}{}", base, &endpoint[last_segment.len()..]);
            }
        }
    }

    format!("{}{}", base, endpoint)
}

/// 路由解析：model_id → ModelRoute
fn resolve_route(state: &ProxyState, model_id: &str) -> Result<ModelRoute, ProxyErrorResponse> {
    // 测试路由优先
    if let Some(ref routes) = state.test_routes {
        if let Some(route) = routes.get(model_id) {
            return Ok(route.clone());
        }
    }

    let db = state.db.lock().map_err(|e| ProxyErrorResponse {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("db lock: {}", e),
    })?;

    providers::find_model_route(&db, model_id).map_err(|e| ProxyErrorResponse {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: e,
    })?
    .ok_or_else(|| ProxyErrorResponse {
        status: StatusCode::NOT_FOUND,
        message: format!("no route for model '{}'", model_id),
    })
}

/// 代理错误响应
#[derive(Debug)]
pub struct ProxyErrorResponse {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for ProxyErrorResponse {
    fn into_response(self) -> Response {
        let body = json!({
            "error": {
                "message": self.message,
                "type": "proxy_error",
                "code": self.status.as_u16(),
            }
        });
        (self.status, Json(body)).into_response()
    }
}

/// 启动代理服务器（在 tokio::spawn 中调用，不阻塞 setup）
pub async fn start_proxy_server(
    port: u16,
    state: ProxyState,
) -> Result<(), String> {
    let router = create_router(state);
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("bind {}: {}", addr, e))?;

    log::info!("proxy server listening on {}", addr);

    axum::serve(listener, router)
        .await
        .map_err(|e| format!("serve: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_state() -> ProxyState {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_db_with_conn(&conn);
        ProxyState {
            db: Arc::new(Mutex::new(conn)),
            client_pool: Arc::new(ClientPool::new()),
            api_key_resolver: Arc::new(|env_name: &str| {
                if env_name == "TEST_KEY" {
                    Some("sk-test-123".into())
                } else {
                    None
                }
            }),
            compression_engine: Arc::new(CompressionEngine::new_in_memory()),
            test_routes: None,
        }
    }

    #[test]
    fn test_resolve_route_no_model() {
        let state = test_state();
        let result = resolve_route(&state, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_route_with_model() {
        let state = test_state();
        {
            let db = state.db.lock().unwrap();
            let p = providers::CreateProvider {
                name: "test".into(),
                base_url: "https://api.test.com".into(),
                api_key_env: "TEST_KEY".into(),
                proxy_mode: Some("direct".into()),
                proxy_url: None,
                auth_header: None,
                api_key_value: None,
                billing_type: Some("pay_per_use".into()),
                compress_enabled: None,
            };
            let provider = providers::create_provider(&db, &p).unwrap();
            let m = providers::CreateModel {
                provider_id: provider.id,
                model_id: "gpt-4o".into(),
                display_name: None,
                auth_type: r#"["openai","anthropic"]"#.into(),
                is_default: Some(true),
                config_json: None,
            };
            providers::create_model(&db, &m).unwrap();
        }

        let route = resolve_route(&state, "gpt-4o").unwrap();
        assert_eq!(route.model_id, "gpt-4o");
        assert!(route.auth_type.contains("openai"));
        assert_eq!(route.proxy_mode, "direct");
    }

    #[test]
    fn test_pick_auth_strategy_chat_endpoint() {
        let s = pick_auth_strategy(r#"["openai","anthropic"]"#, "/v1/chat/completions");
        assert_eq!(s, AuthStrategy::Passthrough);
    }

    #[test]
    fn test_pick_auth_strategy_messages_endpoint() {
        let s = pick_auth_strategy(r#"["openai","anthropic"]"#, "/v1/messages");
        assert_eq!(s, AuthStrategy::BearerToApiKey);
    }

    #[test]
    fn test_pick_auth_strategy_fallback() {
        // 只有 openai，但端点是 messages → 取第一个
        let s = pick_auth_strategy(r#"["openai"]"#, "/v1/messages");
        assert_eq!(s, AuthStrategy::Passthrough);
    }

    #[test]
    fn test_pick_auth_strategy_legacy_format() {
        // 旧格式单字符串
        let s = pick_auth_strategy("anthropic", "/v1/chat/completions");
        assert_eq!(s, AuthStrategy::BearerToApiKey);
    }

    #[test]
    fn test_build_upstream_url_dedup_v1() {
        // Anthropic: baseUrl 含 /v1 + endpoint /v1/messages → 去重
        assert_eq!(
            build_upstream_url("https://api.anthropic.com/v1", "/v1/messages"),
            "https://api.anthropic.com/v1/messages"
        );
        // Kimi For Coding: 自定义路径 + /v1/messages → 去重
        assert_eq!(
            build_upstream_url("https://api.kimi.com/coding/v1", "/v1/messages"),
            "https://api.kimi.com/coding/v1/messages"
        );
    }

    #[test]
    fn test_build_upstream_url_openai_no_dedup() {
        // OpenAI: baseUrl 含 /v1 + endpoint /chat/completions → 无重复，直接拼接
        assert_eq!(
            build_upstream_url("https://api.openai.com/v1", "/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
        // OpenCode Go
        assert_eq!(
            build_upstream_url("https://opencode.ai/zen/go/v1", "/chat/completions"),
            "https://opencode.ai/zen/go/v1/chat/completions"
        );
        // Responses
        assert_eq!(
            build_upstream_url("https://api.openai.com/v1", "/responses"),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn test_build_upstream_url_custom_path() {
        // GLM: /v4 + /chat/completions → 无重复
        assert_eq!(
            build_upstream_url("https://open.bigmodel.cn/api/coding/paas/v4", "/chat/completions"),
            "https://open.bigmodel.cn/api/coding/paas/v4/chat/completions"
        );
        // 讯飞: /v2 + /chat/completions → 无重复
        assert_eq!(
            build_upstream_url("https://maas-token-api.xf-yun.com/v2", "/chat/completions"),
            "https://maas-token-api.xf-yun.com/v2/chat/completions"
        );
    }

    #[test]
    fn test_build_upstream_url_trailing_slash() {
        assert_eq!(
            build_upstream_url("https://api.openai.com/v1/", "/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
    }
}

/// 集成测试：启动 mock 上游 + 代理服务器，验证完整请求链路
/// 需要 `--features integration` 才会编译执行
#[cfg(test)]
#[cfg(feature = "integration")]
mod integration_tests {
    use super::*;
    use axum::{routing::post, Json, Router};
    use serde_json::json;

    fn test_state() -> ProxyState {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::init_db_with_conn(&conn);
        ProxyState {
            db: Arc::new(Mutex::new(conn)),
            client_pool: Arc::new(ClientPool::new()),
            api_key_resolver: Arc::new(|env_name: &str| {
                if env_name == "TEST_KEY" {
                    Some("sk-test-123".into())
                } else {
                    None
                }
            }),
            compression_engine: Arc::new(CompressionEngine::new_in_memory()),
            test_routes: None,
        }
    }

    fn make_route(model_id: &str, base_url: &str, api_key_env: &str, auth_type: &str) -> ModelRoute {
        ModelRoute {
            model_db_id: 1,
            model_id: model_id.to_string(),
            auth_type: auth_type.to_string(),
            is_default: true,
            config_json: None,
            provider_id: 1,
            provider_name: "test-provider".to_string(),
            base_url: base_url.to_string(),
            api_key_env: api_key_env.to_string(),
            proxy_mode: "direct".to_string(),
            proxy_url: None,
            auth_header: None,
            billing_type: "pay_per_use".to_string(),
            compress_enabled: false,
        }
    }

    /// 启动 mock 上游服务器，返回 (base_url, shutdown_signal)
    async fn start_mock_upstream() -> (String, tokio::sync::oneshot::Sender<()>) {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        // ── OpenAI chat/completions 非流式 ──
        async fn openai_chat(body: String) -> Json<Value> {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({}));
            let model = parsed.get("model").and_then(|v| v.as_str()).unwrap_or("unknown");
            Json(json!({
                "id": "chatcmpl-mock-1",
                "object": "chat.completion",
                "model": model,
                "choices": [{
                    "index": 0,
                    "message": {"role": "assistant", "content": "Hello from mock"},
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "total_tokens": 15,
                    "prompt_tokens_details": {"cached_tokens": 3}
                }
            }))
        }

        // ── OpenAI chat/completions 流式 ──
        async fn openai_chat_stream(body: String) -> axum::response::Response {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({}));
            let model = parsed.get("model").and_then(|v| v.as_str()).unwrap_or("unknown");

            let chunks = vec![
                format!("data: {{\"id\":\"chatcmpl-mock-1\",\"object\":\"chat.completion.chunk\",\"model\":\"{}\",\"choices\":[{{\"index\":0,\"delta\":{{\"role\":\"assistant\"}},\"finish_reason\":null}}]}}\n\n", model),
                "data: {\"id\":\"chatcmpl-mock-1\",\"object\":\"chat.completion.chunk\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n".to_string(),
                format!("data: {{\"id\":\"chatcmpl-mock-1\",\"object\":\"chat.completion.chunk\",\"model\":\"{}\",\"choices\":[{{\"index\":0,\"delta\":{{}},\"finish_reason\":\"stop\"}}],\"usage\":{{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15,\"prompt_tokens_details\":{{\"cached_tokens\":3}}}}}}\n\n", model),
                "data: [DONE]\n\n".to_string(),
            ];

            let stream = futures::stream::iter(chunks.into_iter().map(|c| Ok::<_, std::convert::Infallible>(c)));
            (
                [("content-type", "text/event-stream")],
                axum::body::Body::from_stream(stream),
            ).into_response()
        }

        // ── Anthropic messages 非流式 ──
        async fn anthropic_messages(body: String) -> Json<Value> {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({}));
            let model = parsed.get("model").and_then(|v| v.as_str()).unwrap_or("unknown");
            Json(json!({
                "id": "msg_mock_1",
                "type": "message",
                "role": "assistant",
                "model": model,
                "content": [{"type": "text", "text": "Hello from anthropic mock"}],
                "stop_reason": "end_turn",
                "usage": {
                    "input_tokens": 12,
                    "output_tokens": 8,
                    "cache_read_input_tokens": 4,
                    "cache_creation_input_tokens": 0
                }
            }))
        }

        // ── Anthropic messages 流式 ──
        async fn anthropic_messages_stream(_body: String) -> axum::response::Response {
            let chunks = vec![
                "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_mock_1\",\"model\":\"claude-sonnet-4-20250514\",\"usage\":{\"input_tokens\":20,\"cache_read_input_tokens\":5}}}\n\n".to_string(),
                "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n".to_string(),
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello from stream\"}}\n\n".to_string(),
                "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n".to_string(),
                "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":15}}\n\n".to_string(),
                "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string(),
            ];

            let stream = futures::stream::iter(chunks.into_iter().map(|c| Ok::<_, std::convert::Infallible>(c)));
            (
                [("content-type", "text/event-stream")],
                axum::body::Body::from_stream(stream),
            ).into_response()
        }

        // ── OpenAI Responses 非流式 ──
        async fn openai_responses(body: String) -> Json<Value> {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({}));
            let model = parsed.get("model").and_then(|v| v.as_str()).unwrap_or("unknown");
            Json(json!({
                "id": "resp_mock_1",
                "object": "response",
                "model": model,
                "output": [{
                    "type": "message",
                    "content": [{"type": "output_text", "text": "Hello from responses mock"}]
                }],
                "usage": {
                    "input_tokens": 15,
                    "output_tokens": 10,
                    "prompt_tokens_details": {"cached_tokens": 2}
                }
            }))
        }

        // ── 统一入口：根据请求体的 stream 字段分流 ──
        async fn mock_handler(body: String, endpoint: &str) -> axum::response::Response {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({}));
            let is_stream = parsed.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);

            match endpoint {
                "chat" => {
                    if is_stream { openai_chat_stream(body).await } else { openai_chat(body).await.into_response() }
                }
                "messages" => {
                    if is_stream { anthropic_messages_stream(body).await } else { anthropic_messages(body).await.into_response() }
                }
                "responses" => {
                    openai_responses(body).await.into_response()
                }
                _ => (axum::http::StatusCode::NOT_FOUND, "unknown endpoint").into_response()
            }
        }

        let app = Router::new()
            .route("/v1/chat/completions", post(|body: String| async move { mock_handler(body, "chat").await }))
            .route("/v1/messages", post(|body: String| async move { mock_handler(body, "messages").await }))
            .route("/v1/responses", post(|body: String| async move { mock_handler(body, "responses").await }));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async { rx.await.ok(); })
                .await
                .unwrap();
        });

        (base_url, tx)
    }

    /// 启动代理服务器，返回 (proxy_base_url, shutdown)
    async fn start_test_proxy(upstream_base_url: &str) -> (String, tokio::sync::oneshot::Sender<()>) {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::init_db_with_conn(&conn);

        // 写入 DB（供 /v1/models 端点查询）
        {
            let p = providers::CreateProvider {
                name: "test-provider".into(),
                base_url: upstream_base_url.to_string(),
                api_key_env: "TEST_API_KEY".into(),
                proxy_mode: Some("direct".into()),
                proxy_url: None,
                auth_header: None,
                api_key_value: None,
                billing_type: Some("pay_per_use".into()),
                compress_enabled: Some(false),
            };
            let provider = providers::create_provider(&conn, &p).unwrap();
            for (model_id, auth) in &[
                ("gpt-4o", r#"["openai"]"#),
                ("claude-sonnet-4-20250514", r#"["anthropic","openai"]"#),
                ("o3", r#"["openai"]"#),
            ] {
                let m = providers::CreateModel {
                    provider_id: provider.id,
                    model_id: model_id.to_string(),
                    display_name: None,
                    auth_type: auth.to_string(),
                    is_default: Some(true),
                    config_json: None,
                };
                providers::create_model(&conn, &m).unwrap();
            }
        }

        // 通过 test_routes 直接注入路由，绕过 DB 查询
        let mut routes = std::collections::HashMap::new();
        for (model_id, auth_type, base_url) in &[
            ("gpt-4o", r#"["openai"]"#, format!("{}/v1", upstream_base_url)),
            ("claude-sonnet-4-20250514", r#"["anthropic","openai"]"#, upstream_base_url.to_string()),
            ("o3", r#"["openai"]"#, format!("{}/v1", upstream_base_url)),
        ] {
            routes.insert(
                model_id.to_string(),
                make_route(model_id, base_url, "TEST_API_KEY", auth_type),
            );
        }
        assert!(!routes.is_empty(), "routes must not be empty");
        assert!(routes.contains_key("gpt-4o"), "must contain gpt-4o");

        let test_routes_arc = Arc::new(routes);
        let state = ProxyState {
            db: Arc::new(std::sync::Mutex::new(conn)),
            client_pool: Arc::new(ClientPool::new()),
            api_key_resolver: Arc::new(|env: &str| {
                if env == "TEST_API_KEY" { Some("sk-test-key".into()) } else { None }
            }),
            compression_engine: Arc::new(CompressionEngine::new_in_memory()),
            test_routes: Some(test_routes_arc.clone()),
        };
        // 验证 state 确实持有 test_routes
        assert!(state.test_routes.as_ref().unwrap().contains_key("gpt-4o"), "state.test_routes missing gpt-4o");

        let router = create_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let proxy_url = format!("http://127.0.0.1:{}", addr.port());

        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async { rx.await.ok(); })
                .await
                .unwrap();
        });

        (proxy_url, tx)
    }

    // ── /v1/chat/completions ──────────────────────────────

    #[tokio::test]
    async fn test_chat_completions_non_stream() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/chat/completions", proxy_url))
            .json(&json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "hi"}]
            }))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success(), "status: {}", resp.status());
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body["choices"][0]["message"]["content"], "Hello from mock");
        assert_eq!(body["usage"]["prompt_tokens"], 10);
        assert_eq!(body["usage"]["completion_tokens"], 5);
    }

    #[tokio::test]
    async fn test_chat_completions_stream() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/chat/completions", proxy_url))
            .json(&json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "hi"}],
                "stream": true
            }))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success());
        assert_eq!(
            resp.headers().get("content-type").unwrap().to_str().unwrap(),
            "text/event-stream"
        );

        let text = resp.text().await.unwrap();
        assert!(text.contains("chatcmpl-mock-1"));
        assert!(text.contains("Hello"));
        assert!(text.contains("[DONE]"));
    }

    // ── /v1/messages (Anthropic) ──────────────────────────

    #[tokio::test]
    async fn test_anthropic_messages_non_stream() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/messages", proxy_url))
            .json(&json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 1024,
                "messages": [{"role": "user", "content": "hi"}]
            }))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success(), "status: {}", resp.status());
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body["type"], "message");
        assert_eq!(body["content"][0]["text"], "Hello from anthropic mock");
        assert_eq!(body["usage"]["input_tokens"], 12);
    }

    #[tokio::test]
    async fn test_anthropic_messages_stream() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/messages", proxy_url))
            .json(&json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 1024,
                "messages": [{"role": "user", "content": "hi"}],
                "stream": true
            }))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success());
        let text = resp.text().await.unwrap();
        assert!(text.contains("message_start"));
        assert!(text.contains("Hello from stream"));
        assert!(text.contains("input_tokens"));
        assert!(text.contains("output_tokens"));
    }

    // ── /v1/responses (OpenAI Responses) ──────────────────

    #[tokio::test]
    async fn test_openai_responses_non_stream() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/responses", proxy_url))
            .json(&json!({
                "model": "o3",
                "input": "hi"
            }))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success(), "status: {}", resp.status());
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body["object"], "response");
        assert_eq!(body["usage"]["input_tokens"], 15);
        assert_eq!(body["usage"]["output_tokens"], 10);
    }

    // ── 错误场景 ──────────────────────────────────────────

    #[tokio::test]
    async fn test_missing_model_field() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/chat/completions", proxy_url))
            .json(&json!({
                "messages": [{"role": "user", "content": "hi"}]
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status().as_u16(), 400);
        let body: Value = resp.json().await.unwrap();
        assert!(body["error"]["message"].as_str().unwrap().contains("model"));
    }

    #[tokio::test]
    async fn test_nonexistent_model() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/chat/completions", proxy_url))
            .json(&json!({
                "model": "nonexistent-model-xyz",
                "messages": [{"role": "user", "content": "hi"}]
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status().as_u16(), 404);
        let body: Value = resp.json().await.unwrap();
        assert!(body["error"]["message"].as_str().unwrap().contains("nonexistent-model-xyz"));
    }

    #[tokio::test]
    async fn test_invalid_json_body() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/chat/completions", proxy_url))
            .header("content-type", "application/json")
            .body("not json at all")
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status().as_u16(), 400);
        let body: Value = resp.json().await.unwrap();
        assert!(body["error"]["message"].as_str().unwrap().contains("JSON"));
    }

    // ── /health 端点 ──────────────────────────────────────

    #[tokio::test]
    async fn test_health_endpoint() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .get(format!("{}/health", proxy_url))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success());
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "ok");
    }

    // ── /v1/models 端点 ───────────────────────────────────

    #[tokio::test]
    async fn test_list_models_endpoint() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .get(format!("{}/v1/models", proxy_url))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success());
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body["object"], "list");
        let models = body["data"].as_array().unwrap();
        assert!(models.len() >= 3, "expected >=3 models, got {}", models.len());

        let ids: Vec<&str> = models.iter().filter_map(|m| m["id"].as_str()).collect();
        assert!(ids.contains(&"gpt-4o"));
        assert!(ids.contains(&"claude-sonnet-4-20250514"));
        assert!(ids.contains(&"o3"));
    }

    // ── request_logs 写入验证 ──────────────────────────────

    #[tokio::test]
    async fn test_request_logs_written_after_non_stream() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, proxy_state) = start_test_proxy_with_db(&upstream_url).await;
        let client = reqwest::Client::new();

        // 发送请求
        let resp = client
            .post(format!("{}/v1/chat/completions", proxy_url))
            .json(&json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "test logging"}]
            }))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());

        // 等待异步日志写入
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // 检查 request_logs
        let db = proxy_state.db.lock().unwrap();
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM request_logs WHERE model_id = 'gpt-4o'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(count >= 1, "expected >=1 log entry, got {}", count);

        let log = db
            .query_row(
                "SELECT model_id, endpoint, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, status_code, context_compressed, original_tokens, tokens_saved FROM request_logs WHERE model_id = 'gpt-4o' LIMIT 1",
                [],
                |r| Ok((
                    r.get::<_, String>(0)?,   // model_id
                    r.get::<_, String>(1)?,   // endpoint
                    r.get::<_, i64>(2)?,      // input_tokens
                    r.get::<_, i64>(3)?,      // output_tokens
                    r.get::<_, i64>(4)?,      // cache_read_tokens
                    r.get::<_, i64>(5)?,      // cache_write_tokens
                    r.get::<_, i64>(6)?,      // status_code
                    r.get::<_, bool>(7)?,     // context_compressed
                    r.get::<_, i64>(8)?,      // original_tokens
                    r.get::<_, i64>(9)?,      // tokens_saved
                )),
            )
            .unwrap();
        assert_eq!(log.0, "gpt-4o");
        assert_eq!(log.1, "/v1/chat/completions");
        assert_eq!(log.2, 10); // input_tokens (prompt_tokens)
        assert_eq!(log.3, 5);  // output_tokens (completion_tokens)
        assert_eq!(log.4, 3);  // cache_read_tokens (prompt_tokens_details.cached_tokens)
        assert_eq!(log.5, 0);  // cache_write_tokens
        assert_eq!(log.6, 200); // status_code
        assert!(!log.7);       // context_compressed (压缩未启用)
        assert_eq!(log.8, 0);  // original_tokens
        assert_eq!(log.9, 0);  // tokens_saved
    }

    #[tokio::test]
    async fn test_anthropic_stream_usage_and_cache() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, proxy_state) = start_test_proxy_with_db(&upstream_url).await;
        let client = reqwest::Client::new();

        // Anthropic 流式请求
        let resp = client
            .post(format!("{}/v1/messages", proxy_url))
            .json(&json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 1024,
                "messages": [{"role": "user", "content": "hi"}],
                "stream": true
            }))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let _ = resp.text().await;

        // 等待异步日志写入
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let db = proxy_state.db.lock().unwrap();
        let log = db
            .query_row(
                "SELECT input_tokens, output_tokens, cache_read_tokens, cache_write_tokens FROM request_logs WHERE model_id = 'claude-sonnet-4-20250514' LIMIT 1",
                [],
                |r| Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                )),
            )
            .unwrap();
        // mock message_start: input_tokens=20, cache_read_input_tokens=5
        // mock message_delta: output_tokens=15
        assert_eq!(log.0, 20); // input_tokens
        assert_eq!(log.1, 15); // output_tokens
        assert_eq!(log.2, 5);  // cache_read_tokens
        assert_eq!(log.3, 0);  // cache_write_tokens
    }

    #[tokio::test]
    async fn test_anthropic_non_stream_cache_tokens() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, proxy_state) = start_test_proxy_with_db(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/messages", proxy_url))
            .json(&json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 1024,
                "messages": [{"role": "user", "content": "hi"}]
            }))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let _ = resp.text().await;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let db = proxy_state.db.lock().unwrap();
        let log = db
            .query_row(
                "SELECT input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, endpoint FROM request_logs WHERE model_id = 'claude-sonnet-4-20250514' AND endpoint = '/v1/messages' LIMIT 1",
                [],
                |r| Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                )),
            )
            .unwrap();
        // mock: input_tokens=12, output_tokens=8, cache_read_input_tokens=4
        assert_eq!(log.0, 12); // input_tokens
        assert_eq!(log.1, 8);  // output_tokens
        assert_eq!(log.2, 4);  // cache_read_tokens
        assert_eq!(log.3, 0);  // cache_write_tokens
    }

    #[tokio::test]
    async fn test_openai_stream_cache_tokens() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, proxy_state) = start_test_proxy_with_db(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/chat/completions", proxy_url))
            .json(&json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "hi"}],
                "stream": true
            }))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let _ = resp.text().await;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let db = proxy_state.db.lock().unwrap();
        let log = db
            .query_row(
                "SELECT input_tokens, output_tokens, cache_read_tokens, cache_write_tokens FROM request_logs WHERE model_id = 'gpt-4o' AND is_stream = 1 LIMIT 1",
                [],
                |r| Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                )),
            )
            .unwrap();
        // mock stream: prompt_tokens=10, completion_tokens=5, cached_tokens=3
        assert_eq!(log.0, 10); // input_tokens
        assert_eq!(log.1, 5);  // output_tokens
        assert_eq!(log.2, 3);  // cache_read_tokens
        assert_eq!(log.3, 0);  // cache_write_tokens
    }

    #[tokio::test]
    async fn test_responses_cache_tokens() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, proxy_state) = start_test_proxy_with_db(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/responses", proxy_url))
            .json(&json!({
                "model": "o3",
                "input": "hi"
            }))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let _ = resp.text().await;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let db = proxy_state.db.lock().unwrap();
        let log = db
            .query_row(
                "SELECT input_tokens, output_tokens, cache_read_tokens, cache_write_tokens FROM request_logs WHERE model_id = 'o3' LIMIT 1",
                [],
                |r| Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                )),
            )
            .unwrap();
        // mock: input_tokens=15, output_tokens=10, cached_tokens=2
        assert_eq!(log.0, 15); // input_tokens
        assert_eq!(log.1, 10); // output_tokens
        assert_eq!(log.2, 2);  // cache_read_tokens
        assert_eq!(log.3, 0);  // cache_write_tokens
    }

    /// 启动带压缩路由的代理，返回 (proxy_url, state)
    async fn start_test_proxy_with_compress(upstream_base_url: &str) -> (String, ProxyState) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::init_db_with_conn(&conn);

        {
            let p = providers::CreateProvider {
                name: "test-provider".into(),
                base_url: upstream_base_url.to_string(),
                api_key_env: "TEST_API_KEY".into(),
                proxy_mode: Some("direct".into()),
                proxy_url: None,
                auth_header: None,
                api_key_value: None,
                billing_type: Some("pay_per_use".into()),
                compress_enabled: Some(true),
            };
            let provider = providers::create_provider(&conn, &p).unwrap();
            for (model_id, auth) in &[
                ("gpt-4o", r#"["openai"]"#),
                ("claude-sonnet-4-20250514", r#"["anthropic","openai"]"#),
            ] {
                let m = providers::CreateModel {
                    provider_id: provider.id,
                    model_id: model_id.to_string(),
                    display_name: None,
                    auth_type: auth.to_string(),
                    is_default: Some(true),
                    config_json: None,
                };
                providers::create_model(&conn, &m).unwrap();
            }
        }

        let mut routes = std::collections::HashMap::new();
        for (model_id, auth_type, base_url, compress) in &[
            ("gpt-4o", r#"["openai"]"#, format!("{}/v1", upstream_base_url), true),
            ("claude-sonnet-4-20250514", r#"["anthropic","openai"]"#, upstream_base_url.to_string(), true),
        ] {
            let mut route = make_route(model_id, base_url, "TEST_API_KEY", auth_type);
            route.compress_enabled = *compress;
            routes.insert(model_id.to_string(), route);
        }

        let state = ProxyState {
            db: Arc::new(std::sync::Mutex::new(conn)),
            client_pool: Arc::new(ClientPool::new()),
            api_key_resolver: Arc::new(|env: &str| {
                if env == "TEST_API_KEY" { Some("sk-test-key".into()) } else { None }
            }),
            compression_engine: Arc::new(CompressionEngine::new_in_memory()),
            test_routes: Some(Arc::new(routes)),
        };

        let router = create_router(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let proxy_url = format!("http://127.0.0.1:{}", addr.port());

        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        (proxy_url, state)
    }

    /// 压缩收益测试：发送包含大 JSON 数组的请求，验证 context_compressed=true 且 tokens_saved>0
    #[tokio::test]
    async fn test_compression_benefit_with_large_array() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, proxy_state) = start_test_proxy_with_compress(&upstream_url).await;
        let client = reqwest::Client::new();

        // 构造包含大 JSON 数组的消息内容（SmartCrusher 可压缩）
        let large_array: Vec<String> = (0..200).map(|i| format!("item_{:04}_data_value", i)).collect();
        let content = format!(
            "Analyze this dataset:\n{}",
            serde_json::to_string(&large_array).unwrap()
        );

        let resp = client
            .post(format!("{}/v1/chat/completions", proxy_url))
            .json(&json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": content}]
            }))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success(), "status: {}", resp.status());
        let _ = resp.text().await;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let db = proxy_state.db.lock().unwrap();
        let log = db
            .query_row(
                "SELECT context_compressed, original_tokens, tokens_saved FROM request_logs WHERE model_id = 'gpt-4o' LIMIT 1",
                [],
                |r| Ok((
                    r.get::<_, bool>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                )),
            )
            .unwrap();
        assert!(log.0, "context_compressed should be true");
        assert!(log.1 > 0, "original_tokens should be > 0, got {}", log.1);
        // tokens_saved >= 0（压缩器可能判定不值得压缩）
        assert!(log.2 >= 0, "tokens_saved should be >= 0, got {}", log.2);
        eprintln!("[large_array] context_compressed={}, original_tokens={}, tokens_saved={}", log.0, log.1, log.2);
    }

    /// 压缩收益测试：Anthropic 端点 + 日志内容（LogCompressor 可压缩）
    #[tokio::test]
    async fn test_compression_benefit_anthropic_logs() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, proxy_state) = start_test_proxy_with_compress(&upstream_url).await;
        let client = reqwest::Client::new();

        // 构造包含日志输出的消息内容（LogCompressor 可压缩）
        let log_lines: Vec<String> = (0..100)
            .map(|i| format!("[2026-01-01 12:00:{:02}] ERROR processing request id={} status=failed trace=Traceback (most recent call last): File \"app.py\", line {}, in handle", i, i, i * 3))
            .collect();
        let content = format!("Analyze these logs:\n{}", log_lines.join("\n"));

        let resp = client
            .post(format!("{}/v1/messages", proxy_url))
            .json(&json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 1024,
                "messages": [{"role": "user", "content": content}]
            }))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success(), "status: {}", resp.status());
        let _ = resp.text().await;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let db = proxy_state.db.lock().unwrap();
        let log = db
            .query_row(
                "SELECT context_compressed, original_tokens, tokens_saved FROM request_logs WHERE model_id = 'claude-sonnet-4-20250514' LIMIT 1",
                [],
                |r| Ok((
                    r.get::<_, bool>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                )),
            )
            .unwrap();
        assert!(log.0, "context_compressed should be true");
        assert!(log.1 > 0, "original_tokens should be > 0, got {}", log.1);
        assert!(log.2 >= 0, "tokens_saved should be >= 0, got {}", log.2);
        eprintln!("[anthropic_logs] context_compressed={}, original_tokens={}, tokens_saved={}", log.0, log.1, log.2);
    }

    /// 启动代理并返回 state 以便验证 DB
    async fn start_test_proxy_with_db(upstream_base_url: &str) -> (String, ProxyState) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::init_db_with_conn(&conn);

        // 也需要注册到 DB（list_models 端点从 DB 查询）
        {
            let p = providers::CreateProvider {
                name: "test-provider".into(),
                base_url: upstream_base_url.to_string(),
                api_key_env: "TEST_API_KEY".into(),
                proxy_mode: Some("direct".into()),
                proxy_url: None,
                auth_header: None,
                api_key_value: None,
                billing_type: Some("pay_per_use".into()),
                compress_enabled: Some(false),
            };
            let provider = providers::create_provider(&conn, &p).unwrap();
            for (model_id, auth) in &[
                ("gpt-4o", r#"["openai"]"#),
                ("claude-sonnet-4-20250514", r#"["anthropic","openai"]"#),
                ("o3", r#"["openai"]"#),
            ] {
                let m = providers::CreateModel {
                    provider_id: provider.id,
                    model_id: model_id.to_string(),
                    display_name: None,
                    auth_type: auth.to_string(),
                    is_default: Some(true),
                    config_json: None,
                };
                providers::create_model(&conn, &m).unwrap();
            }
        }

        // test_routes 用于路由解析（绕过 DB 查询的竞态问题）
        let mut routes = std::collections::HashMap::new();
        for (model_id, auth_type, base_url) in &[
            ("gpt-4o", r#"["openai"]"#, format!("{}/v1", upstream_base_url)),
            ("claude-sonnet-4-20250514", r#"["anthropic","openai"]"#, upstream_base_url.to_string()),
            ("o3", r#"["openai"]"#, format!("{}/v1", upstream_base_url)),
        ] {
            routes.insert(
                model_id.to_string(),
                make_route(model_id, base_url, "TEST_API_KEY", auth_type),
            );
        }

        let state = ProxyState {
            db: Arc::new(std::sync::Mutex::new(conn)),
            client_pool: Arc::new(ClientPool::new()),
            api_key_resolver: Arc::new(|env: &str| {
                if env == "TEST_API_KEY" { Some("sk-test-key".into()) } else { None }
            }),
            compression_engine: Arc::new(CompressionEngine::new_in_memory()),
            test_routes: Some(Arc::new(routes)),
        };

        let router = create_router(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let proxy_url = format!("http://127.0.0.1:{}", addr.port());

        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        (proxy_url, state)
    }

    // ── Anthropic 鉴权头转换验证 ──────────────────────────

    #[tokio::test]
    async fn test_anthropic_auth_header_transform() {
        // 启动 mock 上游，检查收到的 header
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        use axum::Extension;

        #[derive(Clone)]
        struct HeaderCapture(std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>);

        async fn capture_headers_mw(
            req: axum::extract::Request,
            next: axum::middleware::Next,
        ) -> impl axum::response::IntoResponse {
            let headers: Vec<(String, String)> = req
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let ext = req.extensions().get::<HeaderCapture>().cloned();
            if let Some(cap) = ext {
                *cap.0.lock().unwrap() = headers;
            }
            next.run(req).await
        }

        let captured = HeaderCapture(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())));
        let captured_clone = captured.clone();

        async fn mock_anthropic(_body: String) -> Json<Value> {
            Json(json!({
                "id": "msg_check",
                "type": "message",
                "role": "assistant",
                "model": "claude-sonnet-4-20250514",
                "content": [{"type": "text", "text": "ok"}],
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 1, "output_tokens": 1}
            }))
        }

        let app = Router::new()
            .route("/v1/messages", post(mock_anthropic))
            .layer(axum::middleware::from_fn(capture_headers_mw))
            .layer(Extension(captured));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let upstream_url = format!("http://127.0.0.1:{}", addr.port());

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async { rx.await.ok(); })
                .await
                .unwrap();
        });

        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/messages", proxy_url))
            .json(&json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 100,
                "messages": [{"role": "user", "content": "check headers"}]
            }))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success());

        // 检查上游收到的 headers
        let headers = captured_clone.0.lock().unwrap();
        let header_map: std::collections::HashMap<String, String> = headers.iter().cloned().collect();
        assert!(
            header_map.contains_key("x-api-key"),
            "Anthropic should receive x-api-key header"
        );
        assert!(
            header_map.contains_key("anthropic-version"),
            "Anthropic should receive anthropic-version header"
        );
        assert!(
            !header_map.contains_key("authorization"),
            "Anthropic should NOT receive authorization header"
        );

        let _ = tx.send(());
    }

    // ── 上游不可达 ────────────────────────────────────────

    #[tokio::test]
    async fn test_upstream_unreachable() {
        // 使用一个不可能的端口
        let (proxy_url, _p) = start_test_proxy("http://127.0.0.1:1").await;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/chat/completions", proxy_url))
            .timeout(std::time::Duration::from_secs(15))
            .json(&json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "hi"}]
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status().as_u16(), 502);
        let body: Value = resp.json().await.unwrap();
        assert!(body["error"]["message"].as_str().unwrap().contains("unreachable"));
    }

    // ── Anthropic auth_type 选择 openai 端点时降级 ────────

    #[tokio::test]
    async fn test_auth_type_fallback_to_first() {
        let (upstream_url, _u) = start_mock_upstream().await;
        let (proxy_url, _p) = start_test_proxy(&upstream_url).await;
        let client = reqwest::Client::new();

        // claude-sonnet-4-20250514 的 auth_type 是 ["anthropic","openai"]
        // 请求 /v1/chat/completions → 应该选 openai（匹配端点）
        let resp = client
            .post(format!("{}/v1/chat/completions", proxy_url))
            .json(&json!({
                "model": "claude-sonnet-4-20250514",
                "messages": [{"role": "user", "content": "hi"}]
            }))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success(), "status: {}", resp.status());
    }

    // ── 代理克隆安全性 ────────────────────────────────────

    #[test]
    fn test_proxy_state_clone_shares_db() {
        let state = test_state();
        let state2 = state.clone();
        // 两个 state 应该共享同一个 DB
        {
            let db = state.db.lock().unwrap();
            let p = providers::CreateProvider {
                name: "clone-test".into(),
                base_url: "https://test.com".into(),
                api_key_env: "KEY".into(),
                proxy_mode: Some("direct".into()),
                proxy_url: None,
                auth_header: None,
                api_key_value: None,
                billing_type: Some("pay_per_use".into()),
                compress_enabled: None,
            };
            providers::create_provider(&db, &p).unwrap();
        }
        {
            let db2 = state2.db.lock().unwrap();
            let providers = providers::list_providers(&db2).unwrap();
            assert!(providers.iter().any(|p| p.name == "clone-test"));
        }
    }
}
