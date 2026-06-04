# 用量分析详情页 + 记忆/会话交互修复

> 日期：2026-06-04
> 状态：已批准，待实现

## 1. 背景与目标

当前 `AnalyticsPanel` 是单页全量展示，信息密度过高，无法聚焦到单个模型的性能细节（TPS、延迟分位数等）。同时记忆/会话面板的折叠展开存在 ID 格式 bug，子项点击后中栏不显示内容。

**目标：**
- 将 `AnalyticsPanel` 拆分为「总览」+「详情」两层 Tab
- 详情页展示模型在时间轴上的 Token 明细和性能指标
- 修复 GenericPanel 子项 ID 格式 bug，改进记忆/会话交互

## 2. 整体结构

### 2.1 Tab 切换

```
┌─────────────────────────────────────────────────────┐
│  [总览]  [详情]         用量分析        [同步数据]   │  ← Tab 栏
├─────────────────────────────────────────────────────┤
│  总览 Tab（现有内容不变）                             │
│  或                                                  │
│  详情 Tab（新增，仅在 usage.db 存在时显示）            │
└─────────────────────────────────────────────────────┘
```

- `AnalyticsPanel` 内部用 `useState<"overview" | "detail">` 管理 Tab 状态
- "详情" Tab 仅在 `checkUsageDb()` 返回 `true` 时渲染 Tab 按钮
- Tab 切换不触发路由跳转，不新增 ActivityBar 图标

### 2.2 数据源

详情页数据来自两个源，按 `(date, model)` join：

| 数据源 | 提供的字段 | 可用性 |
|--------|-----------|--------|
| `request_logs` 表（代理层） | input_tokens, output_tokens, cache_read_tokens, request_count | 代理启用时有数据 |
| `usage.db` 的 `call_records` 表 | avg_tps, avg_latency_ms, p50_latency_ms, p95_latency_ms | qwen-code-usage 安装时有数据 |

- Token 数据优先用 `request_logs`（更精确）
- TPS/延迟数据用 `call_records`（原生字段）
- 只有 `call_records` 的模型也保留（纯 usage.db 场景）
- 两个源都无数据时，详情 Tab 不显示

## 3. 详情页布局

### 3.1 左栏（固定 220px）

```
┌────────────────────┐
│ 模型选择         ☰ │  ← 标题 + 全选/全不选按钮
├────────────────────┤
│ ☑ claude-sonnet-4  │  ← checkbox + 模型短名
│   12.3k req · 2.1M │  ← 请求次数 + 总 token
│ ☑ gemini-2.5-pro   │
│   8.7k req · 1.8M  │
│ ☐ deepseek-r1      │
│   1.2k req · 340k  │
│ ...                │
├────────────────────┤
│ 时间范围            │
│ ┌────────────────┐ │
│ │  [BrushRange]  │ │  ← 复用现有 BrushRange 组件
│ └────────────────┘ │
│ 2025-03 ~ 2025-06  │
├────────────────────┤
│ 粒度: ○ 日  ○ 周   │  ← 聚合粒度切换
└────────────────────┘
```

- 模型列表按总 token 降序排列
- 点击行切换 checkbox，选中变化后图表实时更新
- 默认全选
- BrushRange 控制时间范围，两个图表共享同一时间窗口
- 粒度切换影响 X 轴聚合

### 3.2 图表区（flex-1，上下堆叠）

**Token 堆叠面积图（高度 220px）：**
- X 轴：日期（受 BrushRange 和粒度控制）
- Y 轴：Token 数量
- 每个选中模型 4 条面积带，从下到上堆叠：`输出` → `缓存` → `输入未命中` → `总量`
- 颜色：输出 #3fb950、缓存 #bc8cff、输入未命中 #58a6ff、总量用边框线
- 不同模型用不同透明度/虚线区分
- Hover tooltip 显示：日期、模型名、各指标精确值

**性能折线图（高度 200px）：**
- X 轴：同上（共享时间范围）
- 左 Y 轴：TPS（tokens/sec）
- 右 Y 轴：延迟（ms）
- 4 条折线：TPS、avg_latency、p50_latency、p95_latency
- 颜色：TPS #d29922、avg #58a6ff、p50 #3fb950、p95 #f48771
- 每条线可独立 toggle（点击图例）
- 无 usage.db 数据时显示"性能数据不可用（未安装 qwen-code-usage）"

**汇总统计卡片（图表下方）：**
- 6 个 StatCard：总请求、总 Token、缓存率、平均 TPS、p50 延迟、p95 延迟
- 仅统计当前选中模型
- 风格与总览页 StatCard 一致

## 4. 后端设计

### 4.1 新增文件：`src-tauri/src/commands/metrics.rs`

```rust
// 检测 usage.db 是否存在
pub fn check_usage_db() -> Result<bool, String>;

// 模型详情：合并 request_logs + call_records
pub fn get_model_detail_stats(conn: &Connection, days: u32) -> Result<ModelDetailData, String>;
```

### 4.2 usage.db 读取策略

- `~/.qwen/usage/usage.db` 用独立 `rusqlite::Connection` 打开（只读）
- 不 `ATTACH` 到主 DB，避免 schema 冲突
- 文件不存在时 `check_usage_db` 返回 `false`，`get_model_detail_stats` 跳过 usage.db 部分
- call_records 表结构待确认（需探测实际表名和列名）

### 4.3 p50/p95 计算

- 从 `call_records` 读取指定日期+模型的所有 `latency_ms` 值到 Rust 内存
- 排序后取索引 `len * 50 / 100` 和 `len * 95 / 100` 的值作为 p50/p95
- 每日粒度聚合：每天的 p50/p95 是当天所有请求的分位数

### 4.4 返回类型

```typescript
interface ModelDetailData {
  models: ModelMeta[];
  daily: ModelDailyDetail[];
}

interface ModelMeta {
  model: string;
  total_requests: number;
  total_input: number;
  total_output: number;
  total_cache: number;
  avg_tps: number;
  p50_latency: number;
  p95_latency: number;
}

interface ModelDailyDetail {
  date: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  cache_read: number;
  uncached_input: number;  // = input - cache_read
  avg_tps: number;
  avg_latency: number;
  p50_latency: number;
  p95_latency: number;
  request_count: number;
}
```

### 4.5 SQL 查询

```sql
-- request_logs 侧
SELECT date(timestamp) AS date, model_id,
       SUM(input_tokens), SUM(output_tokens),
       SUM(cache_read_tokens), COUNT(*)
FROM request_logs
WHERE timestamp >= datetime('now', '-' || ? || ' days')
GROUP BY date, model_id;

-- call_records 侧（在 usage.db 连接上）
SELECT DATE(recorded_at), model_name,
       AVG(tokens_per_sec), AVG(latency_ms)
FROM call_records
WHERE recorded_at >= datetime('now', '-' || ? || ' days')
GROUP BY DATE(recorded_at), model_name;
```

p50/p95 在 Rust 侧按 (date, model) 分组后对 latency 值排序计算。

## 5. GenericPanel Bug 修复

### 5.1 Bug 1：子项 ID 格式导致 handleSelect 不触发

**根因：** `toggleGroup` 将子项 ID 重写为 `{groupId}::childId`，`onSelect` 传递重写后的 ID。SessionsPanel 的 `handleSelect` 判断 `id.startsWith("session:")`，实际收到的是 `project:xxx::session:yyy:zzz`，不匹配。

**修复：** `GenericThreeColumnPanel` 的 `handleSelect` 在调用 `onSelect` 前剥离 `groupId::` 前缀：

```typescript
const handleSelect = (id: string) => {
  setSelected(id);
  const rawId = id.includes("::") ? id.split("::").slice(1).join("::") : id;
  onSelect?.(rawId);
};
```

MemoryPanel 和 SessionsPanel 的 `handleSelect` 无需改动。

### 5.2 Bug 2：搜索后折叠状态不同步

**修复：** 搜索模式下隐藏展开/折叠箭头，分组标题不可点击展开，仅作视觉分隔。搜索清除后恢复原有展开状态。

```typescript
const showExpand = loadGroupChildren && !searchQuery;
```

### 5.3 改进：会话列表"加载更多"

SessionsPanel 的 `loadGroupChildren` 去掉 `.slice(0, 30)` 硬限制：
- 首次加载 30 条
- 列表底部渲染"加载更多（剩余 N 条）"按钮
- 点击后追加下一批 30 条
- 分页状态由 SessionsPanel 自行管理（`loadedCount` state）

### 5.4 改进：展开/折叠微动画

- 箭头 ▶/▼ 改为 `▶` + CSS `transition-transform rotate-90` 实现旋转动画
- 子项出现/消失加 `opacity` 过渡（可选，如果性能允许）

## 6. 文件变更清单

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `src-tauri/src/commands/metrics.rs` | 新增 | usage.db 读取 + 模型详情查询 |
| `src-tauri/src/commands/mod.rs` | 修改 | 注册新命令 |
| `src-tauri/src/lib.rs` | 修改 | 挂载 metrics 模块 |
| `src/lib/tauri.ts` | 修改 | 新增类型和 IPC 函数 |
| `src/components/analytics/AnalyticsPanel.tsx` | 修改 | Tab 切换 + 热力图缩小 + 项目统计 Top 5 |
| `src/components/analytics/AnalyticsDetail.tsx` | 新增 | 详情页组件（左栏 + 图表） |
| `src/components/analytics/TokenAreaChart.tsx` | 新增 | Token 堆叠面积图 |
| `src/components/analytics/PerfLineChart.tsx` | 新增 | 性能折线图 |
| `src/components/layout/GenericPanel.tsx` | 修改 | ID 前缀剥离 + 搜索模式修复 + 动画 |
| `src/components/sessions/SessionsPanel.tsx` | 修改 | 加载更多分页 |

## 7. 总览页微调

### 7.1 双栏网格布局

当前总览页从上到下 7 个 Section 垂直堆叠，需要滚动多屏。改为双栏网格，从 7 行压缩到 3 行：

```
┌──────────────────────────────────────────────────┐
│ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐        │  Row 1: 汇总卡片（不变，5 个一行）
│ │会话 │ │消息 │ │Token│ │缓存 │ │活跃 │        │
│ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘        │
├────────────────────┬─────────────────────────────┤
│ ┌────────────────┐ │ ┌─────────────────────────┐ │  Row 2: Token明细 + 热力图&项目统计
│ │ 输入 / 输出 /  │ │ │  热力图（缩小）          │ │
│ │ 缓存 TokenCard │ │ ├─────────────────────────┤ │
│ │                │ │ │  项目统计 Top 5          │ │
│ └────────────────┘ │ └─────────────────────────┘ │
├────────────────────┼─────────────────────────────┤
│ ┌────────────────┐ │ ┌─────────────────────────┐ │  Row 3: 模型排名 + 模型趋势
│ │ 模型用量排名    │ │ │  模型 Token 用量趋势     │ │
│ │ + 工具调用排行  │ │ │                         │ │
│ └────────────────┘ │ └─────────────────────────┘ │
└────────────────────┴─────────────────────────────┘
```

关键变化：
- 外层 `space-y-4`，Row 2-3 用 `grid grid-cols-2 gap-4`
- 小屏幕（< md）退回单栏
- 热力图和项目统计合并为一个 Section（上下排列在同一列）
- 模型排名和工具调用排行合并为一个 Section（上下排列在同一列）
- **项目统计列表**：去掉两端对齐（`justify-between`），改为紧凑排列——项目名 + 数值右对齐，行间距缩小，去掉多余 padding

### 7.2 热力图尺寸缩小

- `cellSize` 从 14 缩小到 11
- `gap` 从 3 缩小到 2
- 保持 GitHub 风格 7 行 × 周列布局不变

### 7.3 项目统计 Top 5 + 紧凑排列

- 后端 `get_analytics_summary` 的 project_stats 查询加 `LIMIT 5`（已按 `COUNT(*) DESC` 排序）
- 列表行高从 h-8 缩小到 h-6，去掉 `justify-between`，数值用固定宽度右对齐
- 去掉多余 padding，整体更紧凑

## 8. 不做的事

- 不新增 ActivityBar 图标，详情页在 AnalyticsPanel 内部 Tab 切换
- 不实现 request_logs 表的新写入逻辑（已有代理层写入）
- 不做数据源优先级合并的复杂逻辑（简单 LEFT JOIN，字段互斥取值）
