# 06 — 会话分析 (Session Analysis)

> 解析 Qwen Code 的 JSONL 会话文件，与代理日志 join 做联合成本分析。

## 数据来源

### Qwen Code 会话文件

```
~/.qwen/projects/<project>/chats/
├── <session-id>.jsonl          ← 逐行 JSON，每行一条消息
└── <session-id>.runtime.json   ← 会话元数据（版本、PID、启动时间）
```

### 代理请求日志

AgentBox SQLite 的 `request_logs` 表，通过 `session_id` 关联。

## JSONL 解析

### 消息类型

| type | subtype | 说明 | 成本相关 |
|---|---|---|---|
| `user` | — | 用户输入 | — |
| `assistant` | — | 模型响应 | 包含 usageMetadata |
| `system` | `ui_telemetry` | UI 遥测 | 包含 token 用量 |
| `system` | `attribution_snapshot` | 署名快照 | — |
| `system` | `custom_title` | 自定义标题 | — |
| `tool_result` | — | 工具调用结果 | — |

### usageMetadata 提取

```json
{
  "usageMetadata": {
    "inputTokenCount": 1500,
    "outputTokenCount": 200,
    "totalTokenCount": 1700,
    "cachedContentTokenCount": 500
  }
}
```

### systemPayload 中的工具信息

```json
{
  "systemPayload": {
    "toolCalls": [
      { "name": "read_file", "duration_ms": 120, "success": true },
      { "name": "edit", "duration_ms": 350, "success": true }
    ]
  }
}
```

## 分析维度

### 会话级统计

```rust
struct SessionStats {
    session_id: String,
    project_path: String,
    qwen_version: String,
    git_branch: String,
    started_at: DateTime,
    ended_at: DateTime,
    message_count: u32,
    user_messages: u32,
    assistant_messages: u32,
    tool_calls_total: u32,
    tool_calls_by_type: HashMap<String, u32>,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: u64,
    estimated_cost_usd: f64,
    // 代理层数据（join 后）
    proxy_requests: u32,
    proxy_errors: u32,
    actual_cost_usd: Option<f64>,  // 代理层精确计算
    tokens_saved_by_compression: u64,
}
```

### 项目级聚合

```rust
struct ProjectStats {
    project_path: String,
    session_count: u32,
    total_messages: u32,
    total_tokens: u64,
    total_cost_usd: f64,
    top_models: Vec<(String, u32)>,       // (model, count)
    top_tools: Vec<(String, u32)>,        // (tool, count)
    daily_usage: Vec<DailyUsage>,         // 每日用量趋势
}
```

## 会话与代理日志 Join

```
会话数据 (JSONL)              代理日志 (SQLite)
  session_id ────────────────── session_id
  usageMetadata.input_tokens ←→ request_logs.input_tokens (对比校准)
  message.type = "assistant" ←→ request_logs.endpoint
  timestamp                  ←→ request_logs.timestamp (时间窗口匹配)
```

Join 逻辑：
1. 通过 `session_id` 直接关联（如果代理能提取到）
2. 如果 session_id 不可用，通过时间窗口 + model 匹配
3. 对比 JSONL 中的 usageMetadata 和代理日志中的 usage，做数据校准

## session_id 提取

Qwen Code 的 API 请求中不直接携带 session_id。提取策略：

1. **首选**：从 Qwen Code 的 runtime.json 中读取 session_id，代理启动时注册
2. **备选**：代理层维护活跃会话映射（通过监控 chats/ 目录的文件创建事件）
3. **兜底**：通过时间窗口 + 工作目录匹配

## Tauri IPC 命令

| 命令 | 说明 |
|---|---|
| `list_projects()` | 列出所有项目目录 |
| `list_sessions(project)` | 列出项目的会话列表 |
| `get_session_stats(session_id)` | 获取会话详细统计 |
| `get_project_stats(project)` | 获取项目级聚合 |
| `get_session_messages(session_id, offset, limit)` | 分页获取会话消息 |
| `get_cost_breakdown(filters)` | 成本分解查询（支持按日期/模型/provider 过滤） |
| `export_cost_report(format, filters)` | 导出成本报告（CSV/JSON） |
