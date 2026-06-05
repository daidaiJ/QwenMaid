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
    // OpenAI: baseUrl 含 /v1，上游只加 /chat/completions
    handle_openai_endpoint(&state, "/chat/completions", &body).await
}

async fn handle_messages(
    AxumState(state): AxumState<ProxyState>,
    body: String,
) -> Result<Response, ProxyErrorResponse> {
    // Anthropic: baseUrl 含 /v1，上游加 /v1/messages
    handle_anthropic_endpoint(&state, "/v1/messages", &body).await
}

async fn handle_responses(
    AxumState(state): AxumState<ProxyState>,
    body: String,
) -> Result<Response, ProxyErrorResponse> {
    // OpenAI Responses: baseUrl 含 /v1，上游只加 /responses
    handle_openai_endpoint(&state, "/responses", &body).await
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
