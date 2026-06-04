# 01 — 代理引擎 (Proxy Engine)

> axum HTTP 代理服务器，嵌入 Tauri 进程，拦截 Qwen Code 的所有 API 请求。

## 职责

- 监听本地端口（默认 18900），接收 Qwen Code 的 API 请求
- 根据请求的 endpoint + model 路由到对应的 Provider
- 执行鉴权转换和 API 格式转换
- 流式透传响应（SSE）
- 提取用量信息并记录到 SQLite

## 服务器生命周期

```
AgentBox 启动
  │
  ├─ 初始化 SQLite 数据库
  ├─ 加载 Provider/Model 配置
  ├─ 启动 axum HTTP 服务器 (localhost:18900)
  │     └─ 注册路由: /v1/chat/completions, /v1/messages, /v1/responses, /v1/models
  └─ 前端就绪

AgentBox 关闭
  │
  ├─ 停止接受新连接
  ├─ 等待进行中的请求完成（超时 30s）
  └─ 关闭 SQLite 连接
```

## 路由表

| 入站端点 | 说明 | 路由逻辑 |
|---|---|---|
| `/v1/chat/completions` | OpenAI Chat Completions | 查 model → 确定 provider/auth_type → 转发 |
| `/v1/messages` | Anthropic Messages | 查 model → 确定 provider → 鉴权转换 → 转发 |
| `/v1/responses` | OpenAI Responses | 查 model → 确定 provider → 转发 |
| `/v1/models` | 模型列表 | 合并所有 provider 的模型列表返回 |
| `/health` | 健康检查 | 返回代理状态 |

## 路由匹配算法

### 基础路由

```
1. 解析请求体中的 model 字段
2. 在 models 表中查找 model_id 匹配的所有记录
3. 筛选 is_active=1 的 provider
4. 使用加权选择算法选出最佳路由（见下方）
5. 根据 model 的 auth_type 选择处理管道
```

### API 模型池（加权路由）

同一 model_id 可注册在多个供应商（或同一供应商多个 SK），形成模型池。新会话按历史时延 + 当前负载加权路由。

```rust
struct RouteCandidate {
    model_db_id: i64,
    provider_id: i64,
    base_url: String,
    avg_latency_ms: f64,     // 最近 N 次请求的平均时延（从 request_logs 统计）
    p95_latency_ms: f64,     // P95 时延
    success_rate: f64,       // 最近成功率
    in_flight: u32,          // 当前进行中的请求数
    last_success_at: Option<String>,  // 上次成功时间
}
```

**选择算法：**

```
score = success_rate × (1.0 / (avg_latency_ms + 1)) × (1.0 / (in_flight + 1))

权重 = score^2  // 放大差异，让优质路由更集中

选中 = 加权随机（weighted_random）
```

**指标来源：**
- `avg_latency_ms` / `p95_latency_ms` / `success_rate`：从 `request_logs` 表按 provider_id + model_id 聚合，缓存在内存中，定时刷新（每 60s）
- `in_flight`：内存计数器，请求开始 +1，响应完成 -1
- `last_success_at`：models 表字段，请求成功后更新

**冷启动：** 新增供应商或历史数据不足时，使用默认权重（等概率），避免冷启动偏差。

**故障排除：** 连续失败 3 次的路由临时降权 60s，期间仍可被选中但概率大幅降低。

## 处理管道

```rust
// 伪代码
async fn handle_request(req: Request) -> Response {
    // 1. 解析请求
    let body = parse_body(req);
    let model_id = body.model;

    // 2. 路由匹配
    let route = db.find_model_route(&model_id)?;

    // 3. 鉴权转换
    let headers = auth::transform_headers(req.headers(), &route.auth_type, &route.api_key);

    // 4. API 格式转换（如需要）
    let transformed = transform::apply(body, route.auth_type)?;

    // 5. 上下文压缩（如启用）
    let compressed = compress::maybe_compress(transformed, config)?;

    // 6. 转发请求
    let upstream_req = build_upstream_request(route.base_url, route.endpoint, headers, compressed);
    let response = client.send(upstream_req).await?;

    // 7. 流式透传 + 用量提取
    if response.is_stream() {
        proxy_stream(response, |chunk| {
            usage::extract_from_chunk(chunk);  // 提取 usage 信息
            forward_to_client(chunk);          // 透传
        })
    } else {
        let usage = usage::extract_from_response(&response);
        db.record_request(usage);
        forward_to_client(response)
    }
}
```

## 错误处理

| 场景 | 处理方式 |
|---|---|
| Provider 不可达 | 返回 502 + 错误信息 |
| 鉴权失败 (401/403) | 透传 Provider 错误，记录日志 |
| 模型未找到 | 返回 404 + 可用模型列表提示 |
| 上游超时 | 返回 504 + 超时信息 |
| 请求格式错误 | 返回 400 + 格式说明 |

## HTTP 客户端池（代理热切换）

代理软件（Clash/V2Ray 等）可能在 AgentBox 运行期间启动或关闭，需要无感切换代理路由。

### 性能取舍

| 策略 | 重建频率 | 代价 | 适用场景 |
|---|---|---|---|
| 固定 TTL | 每 30s | TLS 握手 + 连接池重置 | 简单但浪费 |
| **变更检测（采用）** | 仅环境变量/注册表变化时 | 变化时一次性代价 | 精确、零浪费 |
| 请求级重试 | 仅连接失败时 | 失败请求额外延迟 | 作为兜底 |

### 设计

**变更检测 + 失败重试**：只在代理配置实际变化时重建 Client，连接失败时自动切换代理模式重试。

```rust
struct ClientPool {
    clients: Mutex<HashMap<i64, CachedClient>>,
}

struct CachedClient {
    client: reqwest::Client,
    proxy_mode: String,
    proxy_url: Option<String>,
    env_snapshot: ProxySnapshot,  // 创建时的环境变量快照
}

/// 记录代理相关环境变量，用于检测变化
#[derive(Clone, PartialEq)]
struct ProxySnapshot {
    http_proxy: Option<String>,
    https_proxy: Option<String>,
    no_proxy: Option<String>,
}

impl ProxySnapshot {
    fn current() -> Self {
        Self {
            http_proxy: std::env::var("HTTP_PROXY").ok()
                .or_else(|| std::env::var("http_proxy").ok()),
            https_proxy: std::env::var("HTTPS_PROXY").ok()
                .or_else(|| std::env::var("https_proxy").ok()),
            no_proxy: std::env::var("NO_PROXY").ok()
                .or_else(|| std::env::var("no_proxy").ok()),
        }
    }
}

impl ClientPool {
    /// 获取 Client，仅在代理配置变化时重建
    fn get(&self, provider_id: i64, proxy_mode: &str, proxy_url: Option<&str>) -> reqwest::Client {
        let mut clients = self.clients.lock().unwrap();
        let current_snap = ProxySnapshot::current();

        if let Some(cached) = clients.get(&provider_id) {
            let config_changed = cached.proxy_mode != proxy_mode
                || cached.proxy_url.as_deref() != proxy_url;
            let env_changed = cached.proxy_mode == "system"
                && cached.env_snapshot != current_snap;

            if !config_changed && !env_changed {
                return cached.client.clone();  // clone 是廉价的（共享连接池）
            }
        }

        let client = build_client(proxy_mode, proxy_url);
        clients.insert(provider_id, CachedClient {
            client: client.clone(),
            proxy_mode: proxy_mode.to_string(),
            proxy_url: proxy_url.map(|s| s.to_string()),
            env_snapshot: current_snap,
        });
        client
    }

    /// 连接失败时，用 direct 模式重试
    fn get_fallback(&self, provider_id: i64) -> reqwest::Client {
        self.get(provider_id, "direct", None)
    }
}

fn build_client(proxy_mode: &str, proxy_url: Option<&str>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120));

    match proxy_mode {
        "direct" => {
            builder = builder.no_proxy();
        }
        "custom" => {
            if let Some(url) = proxy_url {
                if let Ok(proxy) = reqwest::Proxy::all(url) {
                    builder = builder.proxy(proxy);
                }
            }
        }
        "system" | _ => {
            // 默认行为：读取 HTTP_PROXY/HTTPS_PROXY + Windows 系统代理
            // reqwest 内部调用 winreg 读 HKCU\...\Internet Settings
        }
    }
    builder.build().expect("failed to build reqwest client")
}
```

### 行为

| 场景 | proxy_mode | 效果 |
|---|---|---|
| Qwen/DashScope（国内） | `direct` | 始终直连，Client 永不重建 |
| OpenAI/Anthropic（需翻墙） | `system` | 环境变量变化时自动重建，Clash 启动后下个请求即生效 |
| 自建代理服务 | `custom` | 固定走指定地址，Client 永不重建 |
| 代理不稳定 | 自动 | `system` 模式连接失败 → 自动用 `direct` 重试一次 |

用户在前端切换供应商的 proxy_mode 后，下次请求即生效（无需重启）。

## 测试策略

### 编译时间优化

| 策略 | 说明 |
|---|---|
| 单元测试放源文件内 | `#[cfg(test)] mod tests {}` 在每个 `.rs` 文件底部，不单独编译 |
| 集成测试最小化 | `tests/` 目录只放端到端场景，用 `cargo test --lib` 跑单元测试 |
| mock 用 trait 抽象 | 不引入 `mockall` 重型 crate，用 trait + 手写 fake 实现 |
| test 依赖精简 | dev-dependencies 只加 `tempfile`（临时 DB），不加重型测试框架 |
| `cargo-nextest` | 推荐安装，测试并行执行，比 `cargo test` 快 2-3x |

### Mock 设计

HTTP 客户端和数据库通过 trait 抽象，测试时注入 fake 实现：

```rust
/// 代理转发的核心行为，生产用 reqwest，测试用 Fake
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn send(&self, req: ProxyRequest) -> Result<ProxyResponse, ProxyError>;
}

/// 生产实现
pub struct ReqwestClient { /* client_pool */ }

/// 测试用 fake
pub struct FakeClient {
    pub response: ProxyResponse,
    pub captured_requests: Mutex<Vec<ProxyRequest>>,
}
```

数据库层直接用内存 SQLite（`:memory:`），无需 mock：

```rust
#[cfg(test)]
fn test_db() -> Connection {
    Connection::open_in_memory().unwrap()
}
```

## 配置

代理服务器本身通过 AgentBox 的配置管理：

```json
{
  "proxy": {
    "port": 18900,
    "host": "127.0.0.1",
    "logLevel": "info",
    "maxRetries": 3,
    "timeoutMs": 120000,
    "contextCompression": {
      "enabled": false
    }
  }
}
```
