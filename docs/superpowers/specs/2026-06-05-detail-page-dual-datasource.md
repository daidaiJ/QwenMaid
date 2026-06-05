# 详情页双数据源设计

> 日期: 2026-06-05
> 分支: feat/headroom-lite-proxy
> 状态: 待实现

## 概述

在代理引擎的流式管道中接入 `UsageExtractor`，让 `request_logs` 表拥有真实的 token 和延迟数据。在分析详情页添加双 Tab 数据源切换，用户可选择查看"状态行 usage"（usage.db）或"本地路由代理"（request_logs）的统计数据。

## 需求

- `request_logs` 表的 token 和延迟字段需要被真实填充
- 详情页支持双 Tab 切换数据源
- 两个 Tab 共用相同的图表组件

## 后端：UsageExtractor 接入流式管道

### forward_request 改造

当前 `forward_request` 直接将上游 SSE 流透传给客户端，不解析内容。需要在流式转发过程中：

1. **记录请求开始时间** `start_time = Instant::now()`
2. **流式响应时**，将每个 chunk 同时发送给客户端和 `UsageExtractor`
3. **记录首字节时间** `first_byte_time`（收到第一个 chunk 时）
4. **请求结束时**，从 `UsageExtractor.snapshot` 提取 usage 数据

```rust
// 伪代码
let start = Instant::now();
let mut extractor = UsageExtractor::new(endpoint);
let mut first_byte: Option<Instant> = None;

// 流式转发：每个 chunk 同时喂给 extractor
while let Some(chunk) = stream.next().await {
    if first_byte.is_none() {
        first_byte = Some(Instant::now());
    }
    extractor.process_chunk(&chunk?);
    yield chunk;
}

let duration_ms = start.elapsed().as_millis() as i64;
let ttft_ms = first_byte.map(|t| (t - start).as_millis() as i64).unwrap_or(0);

// 从 extractor.snapshot 提取：
// input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens
```

### log_request 扩展

当前 `log_request` 只写 10 列。扩展为写入完整的 usage 和延迟数据：

| 字段 | 来源 |
|------|------|
| `input_tokens` | `UsageExtractor.snapshot.input_tokens` |
| `output_tokens` | `UsageExtractor.snapshot.output_tokens` |
| `cache_read_tokens` | `UsageExtractor.snapshot.cache_read_tokens` |
| `duration_ms` | `start.elapsed().as_millis()` |
| `time_to_first_ms` | `first_byte_time - start_time` |
| `context_compressed` | 压缩引擎结果 |
| `original_tokens` | 压缩前估算 |
| `tokens_saved` | 压缩节省 |

### 非流式响应

非流式响应的 usage 从响应 JSON body 中直接解析（与 UsageExtractor 的 OpenAI/Anthropic 解析逻辑类似），写入相同字段。

### 新增 Tauri 命令：get_proxy_detail_stats

```rust
#[tauri::command]
pub fn get_proxy_detail_stats(
    state: tauri::State<'_, AppState>,
    days: u32,
) -> Result<ModelDetailData, String>
```

查询 `request_logs` 表，按 (date, model) 聚合：
- `input_tokens = SUM(input_tokens)`
- `output_tokens = SUM(output_tokens)`
- `cache_read = SUM(cache_read_tokens)`
- `uncached_input = input_tokens - cache_read`
- `request_count = COUNT(*)`
- `avg_tps = SUM(output_tokens) / SUM(duration_ms) * 1000`（仅 duration_ms > 0 时）
- `avg_latency = AVG(duration_ms)`
- `p50_latency` / `p95_latency`：使用 subquery 或应用层计算

返回 `ModelDetailData` 结构，与 `get_model_detail_stats` 完全一致。

## 前端：AnalyticsDetail 双 Tab

### UI 结构

在 `AnalyticsDetail` 左栏顶部加双 Tab：

```
┌─────────────────────────────┐
│ [状态行 usage] [本地路由代理] │  ← Tab 切换
├─────────────────────────────┤
│ ☑ gpt-4o                    │
│ ☑ claude-sonnet-4           │
│ ☐ deepseek-v4               │
├─────────────────────────────┤
│ 时间范围: [7] [14] [30] [90]│
│ 粒度:   [日] [周]            │
└─────────────────────────────┘
```

### 数据流

- Tab 1 "状态行 usage"：调用 `getModelDetailStats(days)`（现有逻辑）
- Tab 2 "本地路由代理"：调用 `getProxyDetailStats(days)`（新命令）
- 图表组件（TokenAreaChart、PerfLineChart、统计卡片）完全复用，只是数据源不同

### 空状态

- Tab 1 无数据：显示"未安装 qwen-code-usage"（现有逻辑）
- Tab 2 无数据：显示"无代理请求记录，请先开启本地路由代理"

## 文件修改清单

| 文件 | 变更 |
|------|------|
| `src-tauri/src/proxy/engine.rs` | forward_request 流式管道接入 UsageExtractor + 计时 + log_request 扩展 |
| `src-tauri/src/commands/metrics.rs` | 新增 get_proxy_detail_stats 命令 |
| `src-tauri/src/commands/mod.rs` | 注册新命令 |
| `src/lib/tauri.ts` | 新增 getProxyDetailStats 函数 |
| `src/components/analytics/AnalyticsDetail.tsx` | 双 Tab 数据源切换 |

## 不做的事

- 合并模式（同时展示两个数据源）— 用户选择双 Tab
- 实时流式更新图表 — 数据在请求结束后一次性写入
- TPS 列持久化 — 从 output_tokens/duration_ms 实时计算
