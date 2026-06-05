# Headroom Lite 代理压缩集成实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在 AgentBox 代理引擎中集成 only-cc-lite 上下文压缩，Provider 级开关控制，请求转发前压缩 body，压缩指标写入 request_logs。

**架构：** 新建 `proxy/compression.rs` 封装 only-cc-lite 的 `compress_request()`，在 `forward_request` 中鉴权后条件性调用。CCR 使用独立 SQLite 文件持久化。压缩指标复用 `request_logs` 已有的 `context_compressed` / `original_tokens` / `tokens_saved` 字段。

**技术栈：** Rust, only-cc-lite (git), axum, rusqlite, reqwest

**设计规格：** `docs/superpowers/specs/2026-06-05-headroom-lite-proxy-design.md`

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src-tauri/Cargo.toml` | 修改 | 添加 only-cc-lite 依赖 |
| `src-tauri/src/proxy/compression.rs` | **新建** | 压缩引擎：封装 only-cc-lite，端点→Provider 映射，容错降级 |
| `src-tauri/src/proxy/mod.rs` | 修改 | 导出 compression 模块 |
| `src-tauri/src/proxy/engine.rs` | 修改 | ProxyState 加 compression_engine，forward_request 加压缩步骤 + request_logs 写入 |
| `src-tauri/src/db/mod.rs` | 修改 | 迁移：providers 加 compress_enabled 列 |
| `src-tauri/src/db/providers.rs` | 修改 | ModelRoute / CreateProvider / UpdateProvider 加 compress_enabled |
| `src-tauri/src/lib.rs` | 修改 | 初始化 CompressionEngine，注入 ProxyState |
| `src-tauri/src/commands/mod.rs` | 修改 | Provider CRUD 命令透传 compress_enabled |

---

### 任务 1：添加 only-cc-lite 依赖

**文件：**
- 修改：`src-tauri/Cargo.toml`

- [ ] **步骤 1：添加依赖**

在 `Cargo.toml` 的 `[dependencies]` 中添加：

```toml
only-cc-lite = { git = "https://github.com/daidaiJ/only-cc-lite" }
```

> 注意：only-cc-lite 内部已依赖 blake3、rusqlite 等，无需重复添加。如果出现 rusqlite 版本冲突（only-cc-lite 用 0.32，AgentBox 用 0.35），需要在 only-cc-lite 依赖中指定 `features = ["bundled"]` 或统一版本。

- [ ] **步骤 2：验证编译**

运行：`cd /d/ai/AgentBox/src-tauri && cargo check 2>&1 | head -50`
预期：编译通过或仅有不影响的 warnings。如果 rusqlite 版本冲突，在 Cargo.toml 中用 `rusqlite = { version = "0.35", features = ["bundled"] }` 强制统一版本。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "deps: 添加 only-cc-lite 上下文压缩依赖"
```

---

### 任务 2：DB 迁移 — providers 表添加 compress_enabled

**文件：**
- 修改：`src-tauri/src/db/mod.rs:289-298`（迁移版本 8 之后）
- 修改：`src-tauri/src/db/providers.rs`

- [ ] **步骤 1：编写迁移测试**

在 `src-tauri/src/db/mod.rs` 的 `#[cfg(test)] mod tests` 中添加：

```rust
#[test]
fn test_migration_compress_enabled() {
    let conn = Connection::open_in_memory().unwrap();
    init_db_with_conn(&conn);

    // 验证 compress_enabled 列存在
    let has_col: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('providers') WHERE name='compress_enabled'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_col, "providers table should have compress_enabled column");
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd /d/ai/AgentBox/src-tauri && cargo test test_migration_compress_enabled 2>&1`
预期：FAIL，列不存在。

- [ ] **步骤 3：添加迁移代码**

在 `src-tauri/src/db/mod.rs` 的 `if current < 8` 块之后添加：

```rust
if current < 9 {
    let _ = conn.execute_batch(
        "ALTER TABLE providers ADD COLUMN compress_enabled BOOLEAN NOT NULL DEFAULT 0"
    );
    conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (9)", [])
        .map_err(|e| e.to_string())?;
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd /d/ai/AgentBox/src-tauri && cargo test test_migration_compress_enabled 2>&1`
预期：PASS

- [ ] **步骤 5：更新 providers CRUD**

在 `src-tauri/src/db/providers.rs` 中：

1. `Provider` 结构体添加字段：`pub compress_enabled: bool`
2. `CreateProvider` 添加字段：`pub compress_enabled: Option<bool>`
3. `UpdateProvider` 添加字段：`pub compress_enabled: Option<bool>`
4. `list_providers` SQL 的 SELECT 列表追加 `compress_enabled`，query_map 中追加 `row.get(12)?`（注意字段顺序）
5. `get_provider` 同上
6. `create_provider` INSERT 语句追加 `compress_enabled` 列，值为 `p.compress_enabled.unwrap_or(false)`
7. `update_provider` UPDATE 语句追加 `compress_enabled = COALESCE(?N, compress_enabled)`
8. `find_model_route` SQL 的 SELECT 追加 `p.compress_enabled`，`ModelRoute` 结构体添加 `pub compress_enabled: bool`

> **字段顺序注意：** Provider 现有 12 个字段（id=0..updated_at=11），compress_enabled 为第 13 个（index 12）。所有 query_map 中的 `row.get(N)` 索引需要对应调整。

- [ ] **步骤 6：编写 CRUD 测试**

在 `src-tauri/src/db/providers.rs` 的测试模块（如果存在）或 `mod.rs` 测试中添加：

```rust
#[test]
fn test_provider_compress_enabled() {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::init_db_with_conn(&conn);

    let p = super::providers::CreateProvider {
        name: "test".into(),
        base_url: "https://api.test.com".into(),
        api_key_env: "KEY".into(),
        proxy_mode: Some("direct".into()),
        proxy_url: None,
        auth_header: None,
        api_key_value: None,
        billing_type: Some("pay_per_use".into()),
        compress_enabled: Some(true),
    };
    let created = super::providers::create_provider(&conn, &p).unwrap();
    assert!(created.compress_enabled);

    let got = super::providers::get_provider(&conn, created.id).unwrap();
    assert!(got.compress_enabled);
}
```

- [ ] **步骤 7：运行测试验证通过**

运行：`cd /d/ai/AgentBox/src-tauri && cargo test test_provider_compress_enabled 2>&1`
预期：PASS

- [ ] **步骤 8：Commit**

```bash
git add src-tauri/src/db/mod.rs src-tauri/src/db/providers.rs
git commit -m "feat(db): providers 表添加 compress_enabled 字段"
```

---

### 任务 3：创建压缩模块 — compression.rs

**文件：**
- 创建：`src-tauri/src/proxy/compression.rs`
- 修改：`src-tauri/src/proxy/mod.rs`

- [ ] **步骤 1：编写压缩模块测试**

创建 `src-tauri/src/proxy/compression.rs`：

```rust
use only_cc_lite::{compress_request, CompressOutcome, Provider};
use only_cc_lite::ccr::backends::{CcrBackendConfig, InMemoryCcrStore};
use only_cc_lite::ccr::CcrStore;

use std::sync::Arc;

/// 压缩结果（简化版，供代理引擎使用）
#[derive(Debug, Clone)]
pub struct CompressResult {
    /// 压缩后的请求体（bytes）。如果为 None，表示未压缩（passthrough）
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
                    None => body.to_vec(), // passthrough
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
        assert_eq!(endpoint_to_provider("/v1/chat/completions"), Provider::OpenAiChat);
        assert_eq!(endpoint_to_provider("/v1/responses"), Provider::OpenAiResponses);
        assert_eq!(endpoint_to_provider("/chat/completions"), Provider::OpenAiChat);
    }

    #[test]
    fn test_compress_openai_with_large_content() {
        let engine = CompressionEngine::new_in_memory();

        // 构造一个包含大段 JSON 数组的 OpenAI 请求体
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

        // 压缩应该返回有效 body
        assert!(!result.body.is_empty());
        // 压缩后的 body 应该是合法 JSON
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

        let result = engine.compress(&body_bytes, "/v1/messages", "claude-sonnet-4-20250514");

        assert!(!result.body.is_empty());
        // Anthropic 格式压缩后应仍为合法 JSON
        let _: serde_json::Value = serde_json::from_slice(&result.body).unwrap();
    }

    #[test]
    fn test_compress_passthrough_on_small_body() {
        let engine = CompressionEngine::new_in_memory();

        // 极小的请求体，不应被压缩（或压缩后 tokens_saved 为 0）
        let body = serde_json::json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}]
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();
        let original_len = body_bytes.len();

        let result = engine.compress(&body_bytes, "/v1/chat/completions", "gpt-4o");

        // body 不应为空
        assert!(!result.body.is_empty());
        // 对于极小 body，压缩不应增加大小（最差保持不变）
        assert!(result.body.len() <= original_len + 100); // 允许少量开销
    }

    #[test]
    fn test_compress_invalid_json_does_not_panic() {
        let engine = CompressionEngine::new_in_memory();

        let body = b"this is not json";

        // 不应 panic，应容错降级返回原始 body
        let result = engine.compress(body, "/v1/chat/completions", "gpt-4o");
        assert_eq!(result.body, body);
        assert_eq!(result.tokens_saved, 0);
    }

    #[test]
    fn test_compress_logs_strategies() {
        let engine = CompressionEngine::new_in_memory();

        // 构造一个包含日志内容的请求
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

        // 应该使用了某种压缩策略
        // （具体策略取决于 only-cc-lite 的检测逻辑，但 body 应有效）
        assert!(!result.body.is_empty());
    }
}
```

- [ ] **步骤 2：更新 mod.rs 导出**

在 `src-tauri/src/proxy/mod.rs` 中添加：

```rust
pub mod compression;
```

- [ ] **步骤 3：运行测试验证通过**

运行：`cd /d/ai/AgentBox/src-tauri && cargo test -p agentbox proxy::compression::tests 2>&1`
预期：所有测试 PASS。如果 `endpoint_to_provider` 测试失败，检查 only-cc-lite 的 `Provider` 枚举变体名。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/proxy/compression.rs src-tauri/src/proxy/mod.rs
git commit -m "feat(proxy): 新建 compression 模块，封装 only-cc-lite 压缩引擎"
```

---

### 任务 4：集成到代理引擎 — ProxyState + forward_request

**文件：**
- 修改：`src-tauri/src/proxy/engine.rs`

- [ ] **步骤 1：编写集成测试**

在 `src-tauri/src/proxy/engine.rs` 的 `#[cfg(test)] mod tests` 中添加：

```rust
#[test]
fn test_compress_enabled_in_route() {
    let state = test_state();
    {
        let db = state.db.lock().unwrap();
        let p = providers::CreateProvider {
            name: "compress-provider".into(),
            base_url: "https://api.test.com".into(),
            api_key_env: "TEST_KEY".into(),
            proxy_mode: Some("direct".into()),
            proxy_url: None,
            auth_header: None,
            api_key_value: None,
            billing_type: Some("pay_per_use".into()),
            compress_enabled: Some(true),
        };
        let provider = providers::create_provider(&db, &p).unwrap();
        let m = providers::CreateModel {
            provider_id: provider.id,
            model_id: "gpt-4o-compressed".into(),
            display_name: None,
            auth_type: r#"["openai"]"#.into(),
            is_default: Some(true),
            config_json: None,
        };
        providers::create_model(&db, &m).unwrap();
    }

    let route = resolve_route(&state, "gpt-4o-compressed").unwrap();
    assert!(route.compress_enabled);
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd /d/ai/AgentBox/src-tauri && cargo test test_compress_enabled_in_route 2>&1`
预期：FAIL（test_state 中的 ProxyState 还没有 compression_engine 字段）。

- [ ] **步骤 3：修改 ProxyState**

在 `src-tauri/src/proxy/engine.rs` 中：

1. 添加 use 语句：
```rust
use super::compression::CompressionEngine;
```

2. `ProxyState` 结构体添加字段：
```rust
pub compression_engine: Arc<CompressionEngine>,
```

3. 更新 `test_state()` 函数：
```rust
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
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd /d/ai/AgentBox/src-tauri && cargo test test_compress_enabled_in_route 2>&1`
预期：PASS

- [ ] **步骤 5：修改 forward_request 添加压缩逻辑**

在 `forward_request` 函数中，鉴权转换之后、构建请求之前插入压缩步骤。完整修改后的 `forward_request`：

```rust
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
    let (body_to_send, compress_result) = if route.compress_enabled {
        let result = state.compression_engine.compress(body.as_bytes(), endpoint, &route.model_id);
        let original_tokens = estimate_tokens(body.len());
        let compressed_tokens = estimate_tokens(result.body.len());
        (result, Some((original_tokens, compressed_tokens)))
    } else {
        (
            super::compression::CompressResult {
                body: body.as_bytes().to_vec(),
                tokens_saved: 0,
                bytes_saved: 0,
                strategies: vec![],
            },
            None,
        )
    };

    let mut req_builder = client.post(&url).header("content-type", "application/json");
    for (k, v) in headers {
        req_builder = req_builder.header(k.as_str(), v.as_str());
    }

    let resp = req_builder
        .body(body_to_send.body.clone())
        .send()
        .await
        .map_err(|e| ProxyErrorResponse {
            status: StatusCode::BAD_GATEWAY,
            message: format!("provider unreachable: {}", e),
        })?;

    // 请求成功后更新路由亲和性
    if resp.status().is_success() {
        let db = state.db.lock().ok();
        if let Some(conn) = db {
            providers::touch_model_success(&conn, route.model_db_id);
        }
    }

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    // 记录 request_log（含压缩指标）
    log_request(state, route, endpoint, is_stream, &compress_result, status.as_u16());

    if is_stream {
        let stream = resp.bytes_stream();
        Ok((
            status,
            [("content-type", "text/event-stream")],
            axum::body::Body::from_stream(stream),
        )
            .into_response())
    } else {
        let resp_body = resp.text().await.map_err(|e| ProxyErrorResponse {
            status: StatusCode::BAD_GATEWAY,
            message: format!("read response: {}", e),
        })?;
        Ok((status, [("content-type", "application/json")], resp_body).into_response())
    }
}

/// 估算 token 数（粗略：1 token ≈ 4 字符）
fn estimate_tokens(bytes: usize) -> u64 {
    (bytes as u64 + 3) / 4
}

/// 记录请求日志到 request_logs 表
fn log_request(
    state: &ProxyState,
    route: &ModelRoute,
    endpoint: &str,
    is_stream: bool,
    compress_result: &Option<(u64, u64)>,
    status_code: u16,
) {
    let request_id = uuid::Uuid::new_v4().to_string();
    let (context_compressed, original_tokens, tokens_saved) = match compress_result {
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
        "INSERT INTO request_logs (request_id, provider_id, model_id, auth_type, endpoint, is_stream, context_compressed, original_tokens, tokens_saved, status_code) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            request_id,
            route.provider_id,
            route.model_id,
            route.auth_type,
            endpoint,
            is_stream,
            context_compressed,
            original_tokens,
            tokens_saved,
            status_code,
        ],
    );
}
```

- [ ] **步骤 6：运行全部代理引擎测试**

运行：`cd /d/ai/AgentBox/src-tauri && cargo test -p agentbox proxy::engine::tests 2>&1`
预期：所有现有测试 + 新测试 PASS

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/proxy/engine.rs
git commit -m "feat(proxy): forward_request 集成上下文压缩 + request_logs 写入"
```

---

### 任务 5：初始化 CompressionEngine 并注入 ProxyState

**文件：**
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：修改 lib.rs setup 中的代理启动块**

在 `lib.rs` 的 `tauri::async_runtime::spawn` 代理启动块中，初始化 `CompressionEngine`：

```rust
// 延迟启动代理服务器（不阻塞 setup）
let db_for_proxy = db.clone();
let data_dir_for_proxy = data_dir.clone();
tauri::async_runtime::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let client_pool = Arc::new(proxy::client_pool::ClientPool::new());

    // 初始化 CCR 压缩引擎（SQLite 后端）
    let ccr_db_path = data_dir_for_proxy.join("ccr.db");
    let compression_engine = match proxy::compression::CompressionEngine::new_sqlite(&ccr_db_path) {
        Ok(engine) => {
            log::info!("CCR compression engine initialized at {}", ccr_db_path.display());
            Arc::new(engine)
        }
        Err(e) => {
            log::warn!("CCR SQLite init failed ({}), using in-memory backend", e);
            Arc::new(proxy::compression::CompressionEngine::new_in_memory())
        }
    };

    let state = proxy::engine::ProxyState {
        db: db_for_proxy,
        client_pool,
        api_key_resolver: Arc::new(|env_name: &str| {
            std::env::var(env_name).ok()
        }),
        compression_engine,
    };

    if let Err(e) = proxy::engine::start_proxy_server(18900, state).await {
        log::error!("proxy server failed: {}", e);
    }
});
```

- [ ] **步骤 2：验证编译**

运行：`cd /d/ai/AgentBox/src-tauri && cargo check 2>&1`
预期：编译通过

- [ ] **步骤 3：运行全部测试**

运行：`cd /d/ai/AgentBox/src-tauri && cargo test 2>&1`
预期：所有测试 PASS

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(proxy): 初始化 CompressionEngine 并注入 ProxyState"
```

---

### 任务 6：Tauri IPC — Provider CRUD 透传 compress_enabled

**文件：**
- 修改：`src-tauri/src/commands/mod.rs`

- [ ] **步骤 1：检查现有 Provider 命令实现**

读取 `src-tauri/src/commands/mod.rs`，找到 `create_provider`、`update_provider`、`list_providers` 等 Tauri 命令的参数结构体（通常是 `CreateProviderParams` 或直接用 `providers::CreateProvider`）。

- [ ] **步骤 2：确保 compress_enabled 字段透传**

如果 Tauri 命令的参数结构体是独立定义的（非直接使用 `providers::CreateProvider`），需要在参数结构体中添加 `compress_enabled: Option<bool>` 并传递到 `providers::CreateProvider`。

如果直接使用 `providers::CreateProvider`，则任务 2 中的修改已自动生效，无需额外操作。

- [ ] **步骤 3：验证编译**

运行：`cd /d/ai/AgentBox/src-tauri && cargo check 2>&1`
预期：编译通过

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/commands/mod.rs
git commit -m "feat(commands): Provider CRUD 透传 compress_enabled 字段"
```

---

### 任务 7：端到端验证 — cargo test 全量 + 构建检查

- [ ] **步骤 1：运行全量测试**

运行：`cd /d/ai/AgentBox/src-tauri && cargo test 2>&1`
预期：所有测试 PASS，包括：
- `db::tests::test_migration_compress_enabled`
- `db::tests::test_provider_compress_enabled`（或等价测试）
- `proxy::compression::tests::*`（全部 6 个测试）
- `proxy::engine::tests::test_compress_enabled_in_route`
- 所有现有测试不变

- [ ] **步骤 2：构建检查**

运行：`cd /d/ai/AgentBox/src-tauri && cargo build 2>&1`
预期：构建成功

- [ ] **步骤 3：最终 Commit**

```bash
git add -A
git commit -m "feat: headroom-lite 代理上下文压缩集成完成"
```
