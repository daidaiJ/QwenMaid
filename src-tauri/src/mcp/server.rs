use std::sync::{Arc, Mutex};

use axum::extract::State as AxumState;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use rusqlite::Connection;
use serde_json::{json, Value};

use super::engines;
use super::protocol::{JsonRpcRequest, JsonRpcResponse};

#[derive(Clone)]
pub struct McpState {
    pub db: Arc<Mutex<Connection>>,
    pub http: reqwest::Client,
}

pub async fn start_server(
    port: u16,
    db: Arc<Mutex<Connection>>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|e| e.to_string())?;

    let state = McpState { db, http };

    let app = Router::new()
        .route("/mcp", post(handle_mcp))
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind MCP port {}: {}", port, e))?;

    log::info!("MCP server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            while !*shutdown_rx.borrow_and_update() {
                if shutdown_rx.changed().await.is_err() {
                    break;
                }
            }
            log::info!("MCP server shutting down");
        })
        .await
        .map_err(|e| format!("MCP server error: {}", e))
}

async fn handle_mcp(
    AxumState(state): AxumState<McpState>,
    body: String,
) -> impl IntoResponse {
    let request: JsonRpcRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            let resp = JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
            return (StatusCode::OK, axum::Json(resp)).into_response();
        }
    };

    // Notifications (no id) don't need a response
    if request.id.is_none() && request.method.starts_with("notifications/") {
        return StatusCode::ACCEPTED.into_response();
    }

    let response = match request.method.as_str() {
        "initialize" => handle_initialize(&request),
        "tools/list" => handle_tools_list(&request, &state),
        "tools/call" => handle_tools_call(&request, &state).await,
        "ping" => JsonRpcResponse::success(request.id.clone(), json!({})),
        _ => JsonRpcResponse::error(
            request.id.clone(),
            -32601,
            format!("Method not found: {}", request.method),
        ),
    };

    (StatusCode::OK, axum::Json(response)).into_response()
}

fn handle_initialize(req: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        req.id.clone(),
        json!({
            "protocolVersion": "2025-03-26",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "agentbox-mcp",
                "version": "0.1.0"
            }
        }),
    )
}

fn read_config(db: &Connection) -> Value {
    db.query_row(
        "SELECT port, auto_inject, smartsearch_enabled, academicsearch_enabled, \
         cleanfetch_enabled, search_mode, tavily_api_key, jina_api_key, proxy_url \
         FROM mcp_config WHERE id = 1",
        [],
        |row| {
            Ok(json!({
                "port": row.get::<_, u16>(0)?,
                "auto_inject": row.get::<_, bool>(1)?,
                "smartsearch_enabled": row.get::<_, bool>(2)?,
                "academicsearch_enabled": row.get::<_, bool>(3)?,
                "cleanfetch_enabled": row.get::<_, bool>(4)?,
                "search_mode": row.get::<_, String>(5)?,
                "tavily_api_key": row.get::<_, Option<String>>(6)?,
                "jina_api_key": row.get::<_, Option<String>>(7)?,
                "proxy_url": row.get::<_, Option<String>>(8)?,
            }))
        },
    )
    .unwrap_or_else(|_| json!({}))
}

fn handle_tools_list(req: &JsonRpcRequest, state: &McpState) -> JsonRpcResponse {
    let config = {
        let db = state.db.lock().unwrap();
        read_config(&db)
    };

    let mut tools = Vec::new();

    if config["smartsearch_enabled"].as_bool().unwrap_or(true) {
        tools.push(json!({
            "name": "smartsearch",
            "description": "应当优先使用的网络检索工具，搜索互联网获取最新信息。当需要查询实时数据、最新新闻、技术文档、产品信息、或其他需要联网获取的知识时使用此工具。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "搜索关键词" }
                },
                "required": ["query"]
            }
        }));
    }

    if config["academicsearch_enabled"].as_bool().unwrap_or(false) {
        tools.push(json!({
            "name": "academicsearch",
            "description": "学术论文检索工具，从多个学术数据库（arXiv、Crossref、OpenAlex）并行搜索论文、期刊文章和学术资源。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "学术搜索关键词" },
                    "engines": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "指定引擎子集，如 [\"arxiv\", \"crossref\"]，为空则全部使用"
                    },
                    "time_range": {
                        "type": "string",
                        "enum": ["year", "month", "week"],
                        "description": "时间范围过滤"
                    }
                },
                "required": ["query"]
            }
        }));
    }

    if config["cleanfetch_enabled"].as_bool().unwrap_or(true) {
        tools.push(json!({
            "name": "cleanfetch",
            "description": "网页内容抓取工具，获取指定 URL 的干净网页内容，减小被网站防爬机制阻断的风险。适用于需要阅读某篇文章、获取网页正文、或提取特定页面信息的场景。返回 Markdown 格式的清理后内容。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "要抓取的网页 URL" }
                },
                "required": ["url"]
            }
        }));
    }

    JsonRpcResponse::success(req.id.clone(), json!({ "tools": tools }))
}

async fn handle_tools_call(req: &JsonRpcRequest, state: &McpState) -> JsonRpcResponse {
    let params = &req.params;
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let config = {
        let db = state.db.lock().unwrap();
        read_config(&db)
    };

    let result = match tool_name {
        "smartsearch" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            if query.is_empty() {
                Err("query parameter is required".to_string())
            } else {
                exec_smartsearch(&state.http, query, &config, &state.db).await
            }
        }
        "academicsearch" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            if query.is_empty() {
                Err("query parameter is required".to_string())
            } else {
                let engines_filter = args.get("engines").and_then(|e| e.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect::<Vec<_>>()
                });
                exec_academicsearch(&state.http, query, engines_filter, &state.db).await
            }
        }
        "cleanfetch" => {
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if url.is_empty() {
                Err("url parameter is required".to_string())
            } else {
                exec_cleanfetch(&state.http, url, &config, &state.db).await
            }
        }
        _ => Err(format!("Unknown tool: {}", tool_name)),
    };

    match result {
        Ok(text) => {
            JsonRpcResponse::success(
                req.id.clone(),
                json!({ "content": [{ "type": "text", "text": text }] }),
            )
        }
        Err(e) => {
            JsonRpcResponse::success(
                req.id.clone(),
                json!({
                    "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                    "isError": true
                }),
            )
        }
    }
}

fn record_stat(db: &Arc<Mutex<Connection>>, tool_name: &str, api_name: &str, success: bool) {
    if let Ok(db) = db.lock() {
        let _ = db.execute(
            "INSERT INTO mcp_api_stats (tool_name, api_name, success, called_at) \
             VALUES (?1, ?2, ?3, datetime('now'))",
            rusqlite::params![tool_name, api_name, success],
        );
    }
}

// ── Tool Execution ───────────────────────────────────────

async fn exec_smartsearch(
    client: &reqwest::Client,
    query: &str,
    config: &Value,
    db: &Arc<Mutex<Connection>>,
) -> Result<String, String> {
    let mode = config["search_mode"].as_str().unwrap_or("engine");

    let results = match mode {
        "tavily" => {
            let key = config["tavily_api_key"].as_str().unwrap_or("");
            if key.is_empty() {
                return Err("Tavily API key not configured".to_string());
            }
            let r = engines::search_tavily(client, query, key).await;
            record_stat(db, "smartsearch", "tavily", r.is_ok());
            r?
        }
        "bing" => {
            let r = engines::search_bing(client, query).await;
            record_stat(db, "smartsearch", "bing", r.is_ok());
            r?
        }
        "baidu" => {
            let r = engines::search_baidu(client, query).await;
            record_stat(db, "smartsearch", "baidu", r.is_ok());
            r?
        }
        _ => {
            // engine mode: bing primary, baidu fallback
            match engines::search_bing(client, query).await {
                Ok(r) if !r.is_empty() => {
                    record_stat(db, "smartsearch", "bing", true);
                    r
                }
                Ok(_) => {
                    record_stat(db, "smartsearch", "bing", true);
                    let r = engines::search_baidu(client, query).await;
                    record_stat(db, "smartsearch", "baidu", r.is_ok());
                    r?
                }
                Err(_) => {
                    record_stat(db, "smartsearch", "bing", false);
                    let r = engines::search_baidu(client, query).await;
                    record_stat(db, "smartsearch", "baidu", r.is_ok());
                    r?
                }
            }
        }
    };

    Ok(format_search_results(query, &results))
}

fn format_search_results(query: &str, results: &[engines::SearchResult]) -> String {
    if results.is_empty() {
        return format!("No results found for \"{}\"", query);
    }
    let mut out = format!("## Search Results for \"{}\"\n\n", query);
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!(
            "{}. **{}**\n   URL: {}\n   {}\n\n",
            i + 1,
            r.title,
            r.url,
            if r.snippet.is_empty() {
                "(no description)"
            } else {
                &r.snippet
            }
        ));
    }
    out
}

async fn exec_academicsearch(
    client: &reqwest::Client,
    query: &str,
    engines_filter: Option<Vec<String>>,
    db: &Arc<Mutex<Connection>>,
) -> Result<String, String> {
    let filter = engines_filter.unwrap_or_else(|| vec!["arxiv".into(), "crossref".into(), "openalex".into()]);

    let mut all_results = Vec::new();

    for engine_name in &filter {
        let result = match engine_name.as_str() {
            "arxiv" => engines::search_arxiv(client, query).await,
            "crossref" => engines::search_crossref(client, query).await,
            "openalex" => engines::search_openalex(client, query).await,
            _ => continue,
        };
        match result {
            Ok(results) => {
                record_stat(db, "academicsearch", engine_name, true);
                all_results.extend(results);
            }
            Err(_) => {
                record_stat(db, "academicsearch", engine_name, false);
            }
        }
    }

    Ok(format_academic_results(query, &all_results))
}

fn format_academic_results(query: &str, results: &[engines::AcademicResult]) -> String {
    if results.is_empty() {
        return format!("No academic results found for \"{}\"", query);
    }
    let mut out = format!("## Academic Results for \"{}\"\n\n", query);
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!(
            "{}. **{}**\n   Source: {} | Published: {}\n   Authors: {}\n   URL: {}\n",
            i + 1,
            r.title,
            r.source,
            if r.published.is_empty() {
                "N/A"
            } else {
                &r.published
            },
            if r.authors.is_empty() {
                "N/A"
            } else {
                &r.authors
            },
            r.url,
        ));
        if !r.abstract_text.is_empty() {
            let preview = if r.abstract_text.len() > 300 {
                format!("{}...", &r.abstract_text[..300])
            } else {
                r.abstract_text.clone()
            };
            out.push_str(&format!("   Abstract: {}\n", preview));
        }
        out.push('\n');
    }
    out
}

async fn exec_cleanfetch(
    client: &reqwest::Client,
    url: &str,
    config: &Value,
    db: &Arc<Mutex<Connection>>,
) -> Result<String, String> {
    match engines::fetch_direct(client, url).await {
        Ok(content) if !content.is_empty() && content.len() > 100 => {
            record_stat(db, "cleanfetch", "direct", true);
            Ok(content)
        }
        Ok(_) | Err(_) => {
            record_stat(db, "cleanfetch", "direct", false);
            let jina_key = config["jina_api_key"].as_str().unwrap_or("");
            if !jina_key.is_empty() {
                let r = engines::fetch_jina(client, url, jina_key).await;
                record_stat(db, "cleanfetch", "jina", r.is_ok());
                r
            } else {
                Err("Fetch failed and no Jina API key configured".to_string())
            }
        }
    }
}
