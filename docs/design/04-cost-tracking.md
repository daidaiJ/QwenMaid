# 04 — 成本追踪与数据库 (Cost Tracking)

> SQLite 数据库 schema、token 用量提取、成本聚合查询。

## ORM 选型：SeaORM

使用 SeaORM 作为 Rust ORM，迁移优势：
- `sea-orm-cli migrate generate <name>` 生成迁移文件
- 迁移文件是纯 Rust 代码（`up()` / `down()`），易读易维护
- `sea-orm-cli migrate up/down` 命令行执行迁移
- 支持 `MigratorTrait` 在应用启动时自动检查并执行待迁移
- Entity 自动生成：从数据库 schema 生成 Rust struct

### 迁移目录结构

```
src-tauri/migration/
├── src/
│   ├── lib.rs
│   ├── m20240101_000001_create_providers.rs
│   ├── m20240101_000002_create_models.rs
│   ├── m20240101_000003_create_request_logs.rs
│   ├── m20240101_000004_create_tool_call_stats.rs
│   └── m20240101_000005_create_scan_progress.rs
└── Cargo.toml
```

### 启动时自动迁移

```rust
// 应用启动时
Migrator::up(&db_connection, None).await?;
```

新增表或字段变更时：`sea-orm-cli migrate generate add_xxx` → 写 up/down → 自动执行。

## 数据留存策略

用户可配置不同类型数据的保留天数，应用启动后延迟拉起异步清理任务。

### 配置

```json
{
  "database": {
    "retention": {
      "requestLogs": 30,
      "toolCallStats": 90,
      "scanProgress": 365
    }
  }
}
```

| 数据类型 | 默认保留天数 | 说明 |
|---|---|---|
| `requestLogs` | 30 天 | API 请求日志（数据量最大） |
| `toolCallStats` | 90 天 | 工具调用聚合统计（已压缩） |
| `scanProgress` | 365 天 | 扫描进度记录（轻量，保留以避免重扫） |

### 清理任务

```rust
// 应用启动后延迟 30s 执行，避免阻塞初始化
tokio::spawn(async move {
    tokio::time::sleep(Duration::from_secs(30)).await;
    cleanup_expired_data(&db, &retention_config).await;
});

async fn cleanup_expired_data(db: &DatabaseConnection, config: &RetentionConfig) {
    let cutoff = Utc::now() - Duration::days(config.request_logs);
    request_logs::Entity::delete_many()
        .filter(request_logs::Column::Timestamp.lt(cutoff))
        .exec(db).await;

    // tool_call_stats 和 scan_progress 同理
}
```

- 启动后延迟 30s 执行，不阻塞 UI 初始化和代理启动
- 清理完成后记录日志（删除了多少条、释放了多少空间）
- 可通过前端手动触发立即清理

## 数据库 Schema

### providers — 供应商配置

```sql
CREATE TABLE providers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,
    base_url    TEXT NOT NULL,
    api_key_env TEXT NOT NULL,           -- 环境变量名（不存实际 key）
    is_active   BOOLEAN DEFAULT 1,
    created_at  TEXT DEFAULT (datetime('now')),
    updated_at  TEXT DEFAULT (datetime('now'))
);
```

### models — 模型配置（auth_type 在模型级）

```sql
CREATE TABLE models (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_id    TEXT NOT NULL,           -- 后端实际模型 ID
    display_name TEXT,
    auth_type   TEXT NOT NULL CHECK(auth_type IN ('openai', 'anthropic', 'gemini')),
    is_default  BOOLEAN DEFAULT 0,
    config_json TEXT,                     -- generationConfig JSON
    created_at  TEXT DEFAULT (datetime('now')),
    UNIQUE(provider_id, model_id)
);
```

### request_logs — API 请求日志

```sql
CREATE TABLE request_logs (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id           TEXT UNIQUE NOT NULL,  -- UUID
    session_id           TEXT,                  -- Qwen Code 会话 ID（如能提取）
    timestamp            TEXT DEFAULT (datetime('now')),

    -- 路由信息
    provider_id          INTEGER REFERENCES providers(id),
    model_id             TEXT,
    auth_type            TEXT,
    endpoint             TEXT,                  -- 入站端点

    -- Token 用量
    input_tokens         INTEGER DEFAULT 0,
    output_tokens        INTEGER DEFAULT 0,
    cache_read_tokens    INTEGER DEFAULT 0,
    cache_write_tokens   INTEGER DEFAULT 0,
    reasoning_tokens     INTEGER DEFAULT 0,

    -- 性能
    duration_ms          INTEGER,
    time_to_first_ms     INTEGER,               -- 首 token 延迟
    status_code          INTEGER,
    is_stream            BOOLEAN DEFAULT 0,

    -- 上下文压缩
    context_compressed   BOOLEAN DEFAULT 0,
    original_tokens      INTEGER DEFAULT 0,      -- 压缩前总 token
    tokens_saved         INTEGER DEFAULT 0,      -- 压缩节省

    -- 错误
    error_message        TEXT
);

CREATE INDEX idx_request_logs_session ON request_logs(session_id);
CREATE INDEX idx_request_logs_timestamp ON request_logs(timestamp);
CREATE INDEX idx_request_logs_provider ON request_logs(provider_id, model_id);
```

### cost_daily — 每日成本聚合（物化视图或定时任务）

> session_id 保留在 request_logs 中作为关联键，会话级聚合由 AgentBox 会话分析模块读取 JSONL 时 join 完成。

```sql
CREATE VIEW cost_daily AS
SELECT
    date(timestamp)     AS date,
    provider_id,
    model_id,
    COUNT(*)            AS request_count,
    SUM(input_tokens)   AS total_input,
    SUM(output_tokens)  AS total_output,
    SUM(cache_read_tokens)  AS total_cache_read,
    SUM(reasoning_tokens)   AS total_reasoning,
    SUM(duration_ms)        AS total_duration_ms,
    SUM(CASE WHEN error_message IS NOT NULL THEN 1 ELSE 0 END) AS error_count,
    SUM(tokens_saved)       AS total_tokens_saved
FROM request_logs
GROUP BY date, provider_id, model_id;
```

## Token 用量提取

从 Provider 响应中提取 usage 信息，不同协议格式不同：

### OpenAI 格式

```json
{
  "usage": {
    "prompt_tokens": 100,
    "completion_tokens": 50,
    "total_tokens": 150,
    "prompt_tokens_details": {
      "cached_tokens": 20
    },
    "completion_tokens_details": {
      "reasoning_tokens": 10
    }
  }
}
```

映射：
- `input_tokens` = `prompt_tokens`
- `output_tokens` = `completion_tokens`
- `cache_read_tokens` = `prompt_tokens_details.cached_tokens`
- `reasoning_tokens` = `completion_tokens_details.reasoning_tokens`

### Anthropic 格式

```json
{
  "usage": {
    "input_tokens": 100,
    "output_tokens": 50,
    "cache_creation_input_tokens": 10,
    "cache_read_input_tokens": 20
  }
}
```

映射：
- `input_tokens` = `input_tokens`
- `output_tokens` = `output_tokens`
- `cache_read_tokens` = `cache_read_input_tokens`
- `cache_write_tokens` = `cache_creation_input_tokens`

### 流式响应用量提取

流式（SSE）响应中，usage 信息在最后一个 chunk 中：

```
data: {"choices":[{"delta":{"content":"!"}}]}
data: [DONE]
```

OpenAI 流式可请求 `stream_options: { include_usage: true }`，最后一个 chunk 包含 usage。
Anthropic 流式在 `message_delta` 事件中包含 `usage`。

代理层在流式转发时缓存最后一个 chunk，提取用量后写入数据库。

## 成本计算

成本计算需要每个模型的定价信息，存储在 models.config_json 中：

```json
{
  "pricing": {
    "input_per_mtok": 2.50,
    "output_per_mtok": 10.00,
    "cache_read_per_mtok": 0.25,
    "cache_write_per_mtok": 2.50,
    "reasoning_per_mtok": 10.00,
    "currency": "USD"
  }
}
```

成本公式：
```
cost = (input_tokens / 1_000_000) * input_per_mtok
     + (output_tokens / 1_000_000) * output_per_mtok
     + (cache_read_tokens / 1_000_000) * cache_read_per_mtok
     + (cache_write_tokens / 1_000_000) * cache_write_per_mtok
```

## 成本可视化

### 每日热力图

按天显示成本热力图，颜色深浅代表花费高低：
- X 轴：日期（最近 30/60/90 天可选）
- Y 轴：可选维度（按 model / provider / session）
- 颜色：从浅绿（低花费）→ 深红（高花费）
- 悬停显示：具体金额、请求次数、token 用量

### 细粒度折线图（5h 滚动窗口）

在选定的 5 小时时间窗口内，按分钟级粒度显示 4 条折线：

| 折线 | 颜色 | 计算 |
|---|---|---|
| 总量 | 白色/灰色 | input + output |
| 缓存命中 | 绿色 | cache_read_tokens |
| 未缓存输入 | 蓝色 | input_tokens - cache_read_tokens |
| 输出 | 橙色 | output_tokens |

交互：
- 拖拽选择时间窗口
- 滚轮缩放时间粒度
- 点击数据点查看该时刻的请求详情
- 支持切换成本 ($) / token 数两种 Y 轴

### 官方 API 用量同步

从 Qwen 官方 API 同步用量数据，与代理层本地记录做交叉校验：

```rust
// Qwen/DashScope 用量查询 API
async fn sync_official_usage(
    api_key: &str,
    date_from: &str,
    date_to: &str,
) -> Result<Vec<OfficialUsageRecord>> {
    // 调用官方用量查询接口
    // 将结果与本地 request_logs 做 matching
    // 差异标记为 "unmatched"
}
```

| 数据源 | 说明 | 精度 |
|---|---|---|
| 代理层本地记录 | AgentBox 自己记录的每个请求 | 请求级，实时 |
| 官方 API 同步 | 从 Qwen/DashScope 拉取的账单 | 天级，延迟 |

同步流程：
1. 定时（每天）或手动触发同步
2. 拉取官方用量数据
3. 按日期 + model 聚合，与本地记录对比
4. 差异标记（本地有官方无 = 可能是非 Qwen provider；官方有本地无 = 同步前的请求）
5. 以官方数据为准做成本校准

## 模型工具调用统计

按 model_id 维度统计工具调用表现，用于评估不同模型的工具使用能力。

### 统计指标

| 指标 | 计算 | 说明 |
|---|---|---|
| 请求次数 | COUNT(request_logs) WHERE model_id = X | 该模型的总请求次数 |
| 工具调用次数 | SUM 工具调用相关字段 | 模型触发工具调用的总次数 |
| 工具调用失败次数 | SUM 失败的工具调用 | 工具执行报错的次数 |
| 工具调用率 | tool_calls / requests | 每次请求平均触发多少次工具调用 |
| 工具调用成功率 | (total - failed) / total | 工具调用的通过率 |
| 按工具类型分布 | GROUP BY tool_name | 各工具的调用次数和失败率 |

### 数据来源

基于会话文件定时统计，不从代理层实时提取：

1. 定时扫描 `~/.qwen/projects/*/chats/*.jsonl`
2. 解析每条消息的 `systemPayload.toolCalls` 字段
3. 按 model_id 聚合写入统计表
4. 会话删除（chat 文件夹被清理）→ 对应统计自然过期，无需级联更新

```sql
CREATE TABLE tool_call_stats (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    model_id    TEXT NOT NULL,
    tool_name   TEXT NOT NULL,
    call_count  INTEGER DEFAULT 0,
    fail_count  INTEGER DEFAULT 0,
    avg_duration_ms INTEGER DEFAULT 0,
    sample_date TEXT NOT NULL,           -- 统计日期
    session_count INTEGER DEFAULT 0,     -- 参与统计的会话数
    UNIQUE(model_id, tool_name, sample_date)
);
```

定时任务逻辑（增量扫描）：
```
1. 查询 scan_progress 表，获取每个 session 的上次扫描 offset
2. 遍历所有项目 chats/ 目录
   → 跳过已完全扫描且文件大小未变化的 session
   → 对有新数据的 session，从上次 offset 开始读取
   → 提取新增的 toolCalls 记录
   → 按 model_id + tool_name 增量累加到 tool_call_stats
   → 更新 scan_progress 的 offset 和文件 mtime
3. 清理：删除 scan_progress 中 session_id 已不存在的记录
```

```sql
-- 增量扫描进度追踪
CREATE TABLE scan_progress (
    session_id      TEXT PRIMARY KEY,
    project_path    TEXT NOT NULL,
    file_path       TEXT NOT NULL,
    last_offset     INTEGER NOT NULL DEFAULT 0,  -- 上次扫描到的字节偏移
    last_mtime      TEXT NOT NULL,                -- 上次文件修改时间
    last_scan_at    TEXT DEFAULT (datetime('now')),
    total_lines     INTEGER DEFAULT 0             -- 已解析的消息行数
);
```

### 展示

- **表格视图**：每个 model_id 一行，显示调用次数/失败次数/调用率/成功率
- **柱状图**：按模型对比工具调用率
- **饼图**：单个模型的工具类型分布
- 支持按时间范围筛选

## 请求性能指标

### TTFT (Time To First Token)

从请求发出到收到第一个流式 chunk 的延迟，从 SSE 流中测量。

```sql
-- request_logs 表已有字段
time_to_first_ms    INTEGER    -- 首 token 延迟 (ms)
```

### 吞吐量 (Throughput)

```
throughput_tps = output_tokens / (duration_ms - time_to_first_ms) * 1000
```

单位：tokens/second，仅对流式响应且成功请求计算。

### 聚合与展示

按 provider_id + model_id 聚合 TTFT 和吞吐量的 P50/P95/P99。时间维度展示使用插值折线图，缺失数据点线性填充。

## 供应商错误监控

### 错误分类

**排除（用户自身问题）：**
- 用量耗尽（用户配额用完）
- 余额不足
- 上下文超出限制

**记录并展示（供应商问题）：**
- 5xx 服务端错误（供应商服务崩溃/过载）
- 连接超时 / 读取超时
- 供应商侧限流（429 但非用户配额问题）
- 连接拒绝 / DNS 解析失败
- 响应格式异常（供应商返回了非预期格式）

### 错误识别逻辑

```rust
fn classify_error(status_code: u16, error_body: &str) -> ErrorCategory {
    match status_code {
        429 => {
            if error_body.contains("quota") || error_body.contains("insufficient") {
                ErrorCategory::UserQuotaExhausted  // 排除
            } else {
                ErrorCategory::ProviderRateLimit    // 记录
            }
        }
        402 => ErrorCategory::UserBalanceInsufficient,  // 排除
        400 if error_body.contains("context") => ErrorCategory::ContextLimitExceeded,  // 排除
        500..=599 => ErrorCategory::ProviderServerError,  // 记录
        _ if error_body.contains("timeout") => ErrorCategory::ProviderTimeout,  // 记录
        _ if error_body.contains("ECONNREFUSED") => ErrorCategory::ProviderUnavailable,  // 记录
        _ => ErrorCategory::Unknown,
    }
}
```

### 可用性时间轴

在性能折线图下方叠加：
- 绿色区间：正常运行
- 红色标记：供应商错误事件（悬停显示错误类型、状态码、错误信息）
- 支持点击跳转到对应请求日志

按 provider + model 聚合的可用性百分比：
```
availability = successful_requests / (successful_requests + provider_errors) * 100%
```

## 查询接口（Tauri IPC）

> 代理层聚焦请求级日志和成本聚合，会话级分析由 AgentBox 会话分析模块完成。session_id 保留在 request_logs 中供 join 使用。

| 命令 | 说明 |
|---|---|
| `get_cost_summary(date_from, date_to, group_by)` | 按日期/provider/model 聚合成本 |
| `get_request_logs(session_id, limit, offset)` | 查询请求日志（可按 session_id 过滤） |
| `get_model_usage_stats(model_id)` | 模型使用统计 |
| `get_daily_cost_trend(days)` | 最近 N 天成本趋势 |
| `get_heatmap_data(days, dimension)` | 热力图数据（按天 × 维度） |
| `get_timeseries_data(date_from, date_to, granularity)` | 时序数据（折线图，支持分钟级） |
| `get_performance_stats(provider_id, model_id)` | TTFT/吞吐量聚合统计 |
| `get_availability_timeline(date_from, date_to)` | 可用性时间轴数据 |
| `sync_official_usage(date_from, date_to)` | 触发官方 API 用量同步 |
| `get_sync_status()` | 获取同步状态和最后同步时间 |
