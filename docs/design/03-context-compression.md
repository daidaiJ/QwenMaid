# 03 — 上下文压缩 (Context Compression)

> 集成 headroom 压缩算法，在代理层透明压缩上下文，节省 token。纯本地实现，无网络调用。

## 设计目标

- 在请求发送到 Provider 之前，对 messages 中的历史上下文进行压缩
- 减少 input_tokens，降低 API 成本
- 保持语义完整性，不丢失关键信息
- 纯本地算法，不产生额外 API 调用
- 可通过配置开关控制

## 处理流程

```
Qwen Code 请求进入
  │
  ▼
上下文压缩是否启用？
  ├─ 否 → 直接透传
  └─ 是 ↓
┌─────────────────────────────┐
│ 1. 解析 messages 数组        │
│ 2. 识别 system / 最新 N 轮   │
│    (这些不压缩)              │
│ 3. 对中间历史消息应用压缩    │
│ 4. 替换原始 messages         │
│ 5. 记录压缩统计              │
└─────────────────────────────┘
  │
  ▼
转发到 Provider
```

## 压缩策略

### 分区保护

```
messages:
  [0] system prompt        ← 不压缩
  [1] user                 ┐
  [2] assistant            │ 历史消息（可压缩）
  [3] user                 │
  [4] assistant            │
  ...                      ┘
  [N-2] user               ┐
  [N-1] assistant           │ 最近 K 轮（不压缩）
  [N] user                 ┘
```

- **system prompt**：永远不压缩
- **最近 K 轮**：不压缩（保持对话连贯性），K 可配置（默认 3）
- **中间历史**：应用 headroom 压缩算法

### headroom 压缩算法集成

headroom 作为 Rust crate 引入（或通过 FFI 调用），核心能力：

1. **消息摘要**：将长消息压缩为保留关键信息的短摘要
2. **工具结果裁剪**：对 tool_result 类型的消息进行智能裁剪
3. **重复内容去重**：检测并合并重复的上下文信息
4. **Token 计数**：精确计算压缩前后的 token 差异

### 压缩粒度配置

```json
{
  "proxy": {
    "contextCompression": {
      "enabled": false,
      "preserveRecentTurns": 3,
      "compressionRatio": 0.5,
      "strategies": {
        "toolResults": "truncate",
        "longMessages": "summarize",
        "duplicates": "deduplicate"
      }
    }
  }
}
```

| 策略 | 说明 |
|---|---|
| `toolResults: truncate` | 工具输出超过阈值时截断 |
| `longMessages: summarize` | 长消息压缩为摘要 |
| `duplicates: deduplicate` | 检测并合并重复内容 |

## 统计记录

每次压缩后记录到 `request_logs` 表：

```rust
struct CompressionStats {
    original_tokens: u32,       // 压缩前 token 数
    compressed_tokens: u32,     // 压缩后 token 数
    tokens_saved: u32,          // 节省的 token 数
    messages_compressed: u32,   // 压缩的消息条数
}
```

## 开关控制

- 默认关闭（`enabled: false`）
- 通过 AgentBox 前端设置页面可切换
- 每个请求独立判断，不影响进行中的请求
- 压缩失败时静默回退到原始请求（不阻塞）

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 压缩导致信息丢失 | 保护 system prompt 和最近 K 轮 |
| 压缩算法耗时过长 | 设置超时（默认 500ms），超时跳过 |
| 压缩后 token 数反而增加 | 对比前后 token 数，增加则回退 |
| 与流式响应冲突 | 压缩仅作用于请求侧，响应侧无影响 |
