# 状态行用量追踪方案（简化版）

## 定位

**未启用代理时的兜底数据源。** AgentBox 不重复实现采集逻辑，只做：

1. 管理 `qwen-code-usage` 的安装/更新
2. 管理其自启动和状态行脚本注入到 Qwen Code settings.json
3. 从 `~/.qwen/usage/usage.db` 读取数据，展示在分析仪表盘

## 架构

```
┌─────────────────────────────────────┐
│            Qwen Code                 │
│  settings.json:                      │
│    ui.statusLine → qwen-usage record │
└──────────────┬──────────────────────┘
               │ stdin JSON
               ▼
┌──────────────────────────────────────┐
│  qwen-code-usage (独立进程，自启动)    │
│  - record: 增量计算 → SQLite          │
│  - server: HTTP :9527 (可选)          │
│  存储: ~/.qwen/usage/usage.db        │
└──────────────┬───────────────────────┘
               │ 读取 SQLite (只读)
               ▼
┌──────────────────────────────────────┐
│  AgentBox                            │
│                                      │
│  安装管理:                            │
│    InstallPanel → 检测/安装/更新       │
│    配置注入 → settings.json           │
│                                      │
│  数据展示:                            │
│    AnalyticsPanel → 读 usage.db      │
│    ├─ 30天热力图                      │
│    ├─ Token 用量趋势                  │
│    ├─ 模型分布                        │
│    ├─ 汇总统计                        │
│    └─ 会话列表                        │
└──────────────────────────────────────┘
```

## AgentBox 实现

### 1. 安装管理（InstallPanel 扩展）

```
检测: qwen-usage version
├─ 已安装 → 显示版本，检查更新
└─ 未安装 → 提示安装（go install / 下载预编译）
```

配置注入（一键写入 `~/.qwen/settings.json`）：

```json
{
  "ui": {
    "statusLine": {
      "type": "command",
      "command": "input=$(cat); qwen-usage record <<< \"$input\""
    }
  }
}
```

自启动由 qwen-usage 自身管理，AgentBox 只需确认状态。

### 2. 后端：metrics.rs（Tauri 命令，只读 SQLite）

```rust
use rusqlite::Connection;

fn usage_db() -> PathBuf {
    dirs::home_dir().unwrap().join(".qwen/usage/usage.db")
}

#[tauri::command]
pub fn check_usage_db() -> Result<bool, String>;

/// 热力图：按天聚合
#[tauri::command]
pub fn get_usage_heatmap(days: u32, metric: String) -> Result<Vec<HeatmapEntry>, String>;
// SELECT DATE(recorded_at), SUM(prompt_tokens/completion_tokens/total_tokens/1)
// FROM call_records WHERE recorded_at >= datetime('now', '-? days')
// GROUP BY DATE(recorded_at)

/// 汇总统计
#[tauri::command]
pub fn get_usage_summary(days: u32) -> Result<UsageSummary, String>;
// COUNT(*), SUM(prompt/completion/cached/thoughts/total), AVG(latency),
// COUNT(DISTINCT session_id), COUNT(DISTINCT model_name)

/// 模型分布
#[tauri::command]
pub fn get_model_breakdown(days: u32) -> Result<Vec<ModelUsage>, String>;
// GROUP BY model_name ORDER BY SUM(total_tokens) DESC

/// Token 趋势（按天）
#[tauri::command]
pub fn get_token_trend(days: u32) -> Result<Vec<DailyTokenUsage>, String>;
// SELECT DATE, SUM(prompt), SUM(completion), SUM(cached), SUM(thoughts)

/// 会话列表
#[tauri::command]
pub fn get_usage_sessions(days: u32, limit: u32) -> Result<Vec<SessionRow>, String>;
// GROUP BY session_id, ORDER BY MAX(recorded_at) DESC
```

### 3. 前端：AnalyticsPanel

新增顶级面板 "用量分析"：

- **热力图**：30 天 GitHub-style，metric 切换（请求/Token/输入/输出）
- **汇总卡片**：总请求、总 Token、缓存率、平均延迟、活跃天数
- **Token 趋势**：按天的 stacked bar（prompt + completion + cached）
- **模型分布**：饼图/柱状图
- **会话排行**：按 Token 总量排序

### 4. lib/tauri.ts 新增接口

```typescript
interface HeatmapEntry { date: string; value: number; level: number }
interface UsageSummary {
  total_requests: number; total_tokens: number;
  prompt_tokens: number; completion_tokens: number;
  cached_tokens: number; thoughts_tokens: number;
  cache_hit_rate: number; avg_latency_ms: number;
  session_count: number; model_count: number; active_days: number;
}
interface ModelUsage {
  model: string; requests: number;
  prompt_tokens: number; completion_tokens: number;
  cached_tokens: number; total_tokens: number; avg_latency_ms: number;
}
interface DailyTokenUsage {
  date: string; prompt: number; completion: number;
  cached: number; thoughts: number;
}

export const checkUsageDb = () => invoke<boolean>("check_usage_db");
export const getUsageHeatmap = (days: number, metric: string) =>
  invoke<HeatmapEntry[]>("get_usage_heatmap", { days, metric });
export const getUsageSummary = (days: number) =>
  invoke<UsageSummary>("get_usage_summary", { days });
export const getModelBreakdown = (days: number) =>
  invoke<ModelUsage[]>("get_model_breakdown", { days });
export const getTokenTrend = (days: number) =>
  invoke<DailyTokenUsage[]>("get_token_trend", { days });
export const getUsageSessions = (days: number, limit: number) =>
  invoke<SessionRow[]>("get_usage_sessions", { days, limit });
```

## 数据源优先级

```
代理层数据（精确，逐条请求）  >  状态行数据（兜底，session 维度）
```

- 同一 session 两种数据都有时，以代理数据为准
- 状态行覆盖未启用代理的场景
- 仪表盘标注数据来源

## 依赖

- **qwen-code-usage**：安装 + 自启动（由 qwen-usage 自身管理）
- **rusqlite**：已有的 Cargo 依赖，直接读 SQLite
- **无需 HTTP 通信**：AgentBox 直接读 DB 文件

## 实施顺序

| 步骤 | 内容 |
|------|------|
| 1 | `metrics.rs`：6 个 Tauri 命令读 `~/.qwen/usage/usage.db` |
| 2 | `lib/tauri.ts`：新增 API 函数和类型 |
| 3 | `AnalyticsPanel.tsx`：热力图 + 汇总 + 趋势 + 模型分布 |
| 4 | `InstallPanel`：检测 qwen-usage 安装状态 + 配置注入 |
