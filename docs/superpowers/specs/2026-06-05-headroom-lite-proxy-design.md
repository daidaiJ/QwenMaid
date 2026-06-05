# Headroom Lite 代理压缩集成设计

> 日期: 2026-06-05
> 分支: feat/headroom-lite-proxy
> 状态: 待实现

## 概述

在 AgentBox 代理引擎中集成 [only-cc-lite](https://github.com/daidaiJ/only-cc-lite)（Headroom Lite），为经过代理转发的 LLM 请求提供上下文压缩能力。压缩在请求发送到上游 API 之前执行，可节省 40-90% 的 input tokens。

## 需求

- 压缩作为**独立开关**叠加在现有代理模式（direct/system/custom）上，不互斥
- **Provider 级别**的 `compress_enabled` 布尔开关
- 启用 **CCR（Context Compression Registry）** 无损压缩，SQLite 存储
- 覆盖全部三个端点：`/v1/chat/completions`、`/v1/messages`、`/v1/responses`
- **后端优先**，压缩统计 UI 后续迭代

## 方案选型

| 方案 | 描述 | 选择 |
|------|------|------|
| A: 内联调用 | 新建 `compression.rs`，在 `forward_request` 中条件性调用 | ✅ 推荐 |
| B: axum 中间件 | 路由层拦截请求体压缩 | ❌ 需重构路由解析，过度设计 |

## 数据模型

### providers 表扩展

```sql
ALTER TABLE providers ADD COLUMN compress_enabled BOOLEAN NOT NULL DEFAULT 0;
```

### 新增 ccr_blocks 表

CCR 无损压缩的原始内容存储，BLAKE3 哈希索引。

```sql
CREATE TABLE IF NOT EXISTS ccr_blocks (
    hash        TEXT PRIMARY KEY,   -- BLAKE3 hex (64 chars)
    content     TEXT NOT NULL,      -- 被压缩的原始内容
    provider_id INTEGER,            -- 来源供应商（用于清理）
    created_at  DATETIME DEFAULT (datetime('now'))
);
```

### request_logs 表（已有字段，无需修改）

`request_logs` 表已包含压缩相关字段：

| 字段 | 类型 | 说明 |
|---|---|---|
| `context_compressed` | BOOLEAN DEFAULT 0 | 本次请求是否启用了压缩 |
| `original_tokens` | INTEGER DEFAULT 0 | 压缩前估算 token 数 |
| `tokens_saved` | INTEGER DEFAULT 0 | 节省的 token 数 |

不需要新建 `compression_logs` 表，直接复用这些现有字段记录每个请求的压缩状态。

## 模块设计

### 新增 `proxy/compression.rs`

**职责：** 封装 only-cc-lite，提供与代理引擎的干净接口。

**核心类型：**

```rust
/// 压缩结果
pub struct CompressResult {
    pub body: String,              // 压缩后的请求体（JSON 字符串）
    pub tokens_saved: u64,         // 节省的 token 数
    pub bytes_saved: u64,          // 节省的字节数
    pub strategies: Vec<String>,   // 使用的压缩策略
}

/// 压缩引擎
pub struct CompressionEngine {
    ccr_store: Arc<Mutex<CcrStore>>,  // SQLite-backed CCR
}
```

**核心方法：**

```rust
impl CompressionEngine {
    /// 初始化，传入 SQLite 连接用于 CCR 存储
    pub fn new(db: Connection) -> Self;

    /// 压缩请求体
    /// - body: 原始请求 JSON
    /// - endpoint: 路径，用于推断 provider 格式
    /// - model: 模型 ID
    /// 返回 CompressResult，失败时返回原始 body
    pub fn compress(&self, body: &str, endpoint: &str, model: &str) -> CompressResult;
}
```

**端点 → Provider 类型映射：**

| 代理端点 | only-cc-lite Provider |
|---|---|
| `/v1/messages` | `Provider::Anthropic` |
| `/v1/chat/completions` | `Provider::OpenAI` |
| `/v1/responses` | `Provider::OpenAIResponses` |

**容错策略：** 压缩失败（JSON 解析错误、库 panic catch、内部错误）时：
1. 记录 `log::warn!` 日志
2. 返回原始 body（`CompressResult { body: original, tokens_saved: 0, ... }`）
3. 不阻断请求转发

### 修改 `proxy/engine.rs`

**ProxyState 扩展：**

```rust
pub struct ProxyState {
    pub db: Arc<Mutex<Connection>>,
    pub client_pool: Arc<ClientPool>,
    pub api_key_resolver: Arc<dyn Fn(&str) -> Option<String> + Send + Sync>,
    pub compression_engine: Arc<CompressionEngine>,  // 新增
}
```

**ModelRoute 扩展：**

```rust
pub struct ModelRoute {
    // ... 现有字段 ...
    pub compress_enabled: bool,  // 从 providers 表读取
}
```

**forward_request 修改：**

在鉴权转换之后、构建请求之前插入压缩步骤：

```rust
// 条件性压缩
let (body_to_send, compress_metrics) = if route.compress_enabled {
    let result = state.compression_engine.compress(body, endpoint, &route.model_id);
    (result.body, Some(result))
} else {
    (body.to_string(), None)
};

// 用 body_to_send 发送请求（替代原来的 body）

// 请求完成后，将压缩指标写入 request_logs
// 复用已有字段：context_compressed, original_tokens, tokens_saved
```

### 修改 `db/providers.rs`

- `find_model_route` SQL 查询增加 `p.compress_enabled` 字段
- `ModelRoute` 结构体增加 `compress_enabled: bool` 字段
- `CreateProvider` / `UpdateProvider` 增加 `compress_enabled` 可选字段
- `create_provider` / `update_provider` SQL 语句适配新字段

### 修改 `commands/` (Tauri IPC)

- Provider CRUD 命令透传 `compress_enabled` 字段
- 新增 `get_compression_stats` 命令（可选，后续 UI 用）

## 依赖

```toml
# Cargo.toml 新增
only-cc-lite = { git = "https://github.com/daidaiJ/only-cc-lite" }
```

> 实现时需确认 only-cc-lite 的实际 crate 名和依赖方式（git / path / crates.io）。

## 请求流变更

```
Qwen Code → POST localhost:18900/v1/messages
  ├─ 解析请求体，提取 model_id
  ├─ resolve_route() → ModelRoute（含 compress_enabled）
  ├─ 鉴权转换
  ├─ [NEW] if compress_enabled:
  │    compression_engine.compress(body, endpoint, model)
  │    → 压缩 body，记录 metrics
  ├─ 发送 compressed body 到上游
  ├─ 提取 usage 数据
  ├─ [NEW] 写入 request_logs（context_compressed + original_tokens + tokens_saved）
  └─ 返回响应
```

## 不做的事

- 前端压缩开关 UI（后续迭代）
- 压缩统计面板（后续迭代）
- CCR 过期清理策略（后续迭代，可用定时任务清理过期 blocks）
- 流式请求体的分块压缩（only-cc-lite 处理完整请求体）

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| only-cc-lite 压缩后请求体格式不兼容某些供应商 | 容错降级：压缩失败时用原始 body |
| CCR SQLite 表膨胀 | 后续加 TTL 清理 + provider 级联删除 |
| 压缩增加请求延迟 | only-cc-lite 是纯正则，延迟应 <10ms；记录 duration 供监控 |
| BLAKE3 哈希冲突 | 64 字符 hex 空间，实际碰撞概率可忽略 |
