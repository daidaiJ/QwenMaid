# 用量分析详情页 + 面板交互修复 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。

**目标：** AnalyticsPanel 拆分总览/详情 Tab，详情页展示模型时间轴指标（合并代理层+usage.db），修复 GenericPanel 子项 ID bug，总览页布局压缩为双栏

**架构：** 后端新增 `metrics.rs` 读 usage.db，前端 AnalyticsPanel 内 Tab 切换，详情页左栏多选模型 + 右侧双图表

**技术栈：** React 19 + Tailwind 4 + SVG 图表（无第三方库）| Rust + rusqlite + chrono

---

### 任务 1：GenericPanel Bug 修复 + 动画

**文件：**
- 修改：`src/components/layout/GenericPanel.tsx:89-95`（handleSelect）
- 修改：`src/components/layout/GenericPanel.tsx:193-220`（列表渲染）

- [ ] **步骤 1：修复 handleSelect ID 前缀剥离**

`GenericPanel.tsx` 第 89-95 行，`handleSelect` 调用 `onSelect` 前剥离 `groupId::` 前缀：

```typescript
const handleSelect = (id: string) => {
  setSelected(id);
  const rawId = id.includes("::") ? id.split("::").slice(1).join("::") : id;
  onSelect?.(rawId);
};
```

- [ ] **步骤 2：搜索模式隐藏展开箭头**

列表渲染中（约第 193 行），将 `isGroup && loadGroupChildren` 的展开箭头渲染条件改为：

```typescript
const showExpand = isGroup && loadGroupChildren && !searchQuery;
```

箭头 icon 改为带旋转动画的 `▶`：

```tsx
{showExpand && (
  <span className={`shrink-0 text-[var(--text-muted)] transition-transform duration-150 ${isExpanded ? "rotate-90" : ""}`}>
    ▶
  </span>
)}
```

搜索模式下分组标题点击不触发 `toggleGroup`：

```tsx
onClick={() => {
  if (isGroup && loadGroupChildren && !searchQuery) {
    toggleGroup(item.id);
  } else {
    handleSelect(item.id);
  }
}}
```

- [ ] **步骤 3：构建验证**

```bash
cd /d/ai/AgentBox && npm run build
```

预期：无 TypeScript 错误

- [ ] **步骤 4：Commit**

```bash
git add src/components/layout/GenericPanel.tsx
git commit -m "fix: GenericPanel 子项 ID 前缀剥离 + 搜索模式展开修复 + 箭头旋转动画"
```

---

### 任务 2：SessionsPanel 加载更多

**文件：**
- 修改：`src/components/sessions/SessionsPanel.tsx:40-55`（loadGroupChildren）

- [ ] **步骤 1：添加分页状态和加载更多逻辑**

在 `SessionsPanel` 组件内新增状态：

```typescript
const [groupSessionCounts, setGroupSessionCounts] = useState<Record<string, number>>({});
const [loadedCounts, setLoadedCounts] = useState<Record<string, number>>({});
```

修改 `loadGroupChildren`，接收 `offset` 参数（通过闭包读取 `loadedCounts`）：

```typescript
const loadGroupChildren = useCallback(
  async (groupId: string): Promise<ListItem[]> => {
    const project = groupId.replace("project:", "");
    const sessions = await listSessions(project);
    setGroupSessionCounts((prev) => ({ ...prev, [groupId]: sessions.length }));
    const offset = loadedCounts[groupId] ?? 0;
    const batch = sessions.slice(offset, offset + 30);
    setLoadedCounts((prev) => ({ ...prev, [groupId]: offset + batch.length }));

    const items: ListItem[] = batch.map((s) => ({
      id: `session:${project}:${s.id}`,
      label: s.title || s.id.slice(0, 8),
      description: formatRelativeTime(s.started_at),
      badge: `~${s.message_count}`,
      badgeColor: "var(--text-muted)",
    }));

    // 追加"加载更多"伪项
    const remaining = sessions.length - offset - batch.length;
    if (remaining > 0) {
      items.push({
        id: `__load_more_${groupId}`,
        label: `加载更多（剩余 ${remaining} 条）`,
        badge: "…",
        badgeColor: "var(--text-muted)",
      });
    }
    return items;
  },
  [loadedCounts],
);
```

- [ ] **步骤 2：构建验证 + Commit**

```bash
cd /d/ai/AgentBox && npm run build
git add src/components/sessions/SessionsPanel.tsx
git commit -m "feat: 会话列表分页加载，每次 30 条 + 加载更多按钮"
```

---

### 任务 3：总览页双栏布局压缩

**文件：**
- 修改：`src/components/analytics/AnalyticsPanel.tsx`（布局重构 + 热力图参数 + 项目统计）

- [ ] **步骤 1：热力图参数缩小**

`HeatmapChart` 组件内，`cellSize` 改为 11，`gap` 改为 2。

- [ ] **步骤 2：重构为双栏 grid 布局**

将 `AnalyticsPanel` 的内容区域从 `space-y-5` 垂直堆叠改为：

```tsx
<div className="flex-1 p-5 space-y-4 overflow-auto">
  {/* Row 1: 汇总卡片（不变） */}
  <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
    <StatCard ... /> {/* x5 */}
  </div>

  {/* Row 2: Token明细(左) + 热力图&项目统计(右) */}
  <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
    <div className="grid grid-cols-3 gap-3">
      <TokenCard ... /> {/* x3，保持原有横向布局 */}
    </div>
    <div className="space-y-4">
      {data.daily.length > 0 && (
        <Section title="使用热力图" icon={...}>
          <HeatmapChart daily={data.daily} />
        </Section>
      )}
      {data.project_stats.length > 0 && (
        <Section title="项目统计" icon={...}>
          {/* 紧凑列表，见步骤 3 */}
        </Section>
      )}
    </div>
  </div>

  {/* Row 3: 模型排名+工具排行(左) + 模型趋势(右) */}
  <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
    <div className="space-y-4">
      {data.top_models.length > 0 && (
        <Section title="模型用量排名" icon={...}>
          <ModelRankingTable models={data.top_models} />
        </Section>
      )}
      {data.top_tools.length > 0 && (
        <Section title="工具调用排行" icon={...}>
          <BarList items={data.top_tools} ... />
        </Section>
      )}
    </div>
    {data.model_daily.length > 0 && (
      <Section title="模型 Token 用量趋势" icon={...}>
        <ModelLineChart rows={data.model_daily} />
      </Section>
    )}
  </div>
</div>
```

- [ ] **步骤 3：项目统计 Top 5 + 紧凑排列**

后端 `analytics.rs` 的 project_stats 查询加 `LIMIT 5`（约第 215 行）。

前端项目统计列表改为紧凑样式：

```tsx
<div className="space-y-0.5">
  {data.project_stats.map((p) => (
    <div key={p.project} className="flex items-center gap-2 h-6 px-1 rounded hover:bg-[var(--bg-hover)]">
      <span className="text-[11px] text-[var(--text-primary)] flex-1 truncate font-mono">{decodeProject(p.project)}</span>
      <span className="text-[10px] text-[var(--text-muted)] w-12 text-right">{p.session_count}会话</span>
      <span className="text-[10px] font-mono text-[var(--text-muted)] w-14 text-right">{fmtTok(p.total_tokens)}</span>
    </div>
  ))}
</div>
```

- [ ] **步骤 4：构建验证**

```bash
cd /d/ai/AgentBox && npm run build
```

- [ ] **步骤 5：Commit**

```bash
git add src/components/analytics/AnalyticsPanel.tsx src-tauri/src/commands/analytics.rs
git commit -m "refactor: 总览页双栏网格布局 + 热力图缩小 + 项目统计 Top 5 紧凑排列"
```

---

### 任务 4：后端 metrics.rs — usage.db 读取

**文件：**
- 新增：`src-tauri/src/commands/metrics.rs`
- 修改：`src-tauri/src/commands/mod.rs`（注册模块和命令）
- 修改：`src-tauri/src/lib.rs`（invoke_handler 注册新命令）

- [ ] **步骤 1：创建 metrics.rs**

```rust
// src-tauri/src/commands/metrics.rs
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

fn usage_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".qwen/usage/usage.db")
}

#[tauri::command]
pub fn check_usage_db() -> bool {
    usage_db_path().exists()
}

#[derive(Debug, Serialize, Clone)]
pub struct ModelMeta {
    pub model: String,
    pub total_requests: i64,
    pub total_input: i64,
    pub total_output: i64,
    pub total_cache: i64,
    pub avg_tps: f64,
    pub p50_latency: f64,
    pub p95_latency: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ModelDailyDetail {
    pub date: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read: i64,
    pub uncached_input: i64,
    pub avg_tps: f64,
    pub avg_latency: f64,
    pub p50_latency: f64,
    pub p95_latency: f64,
    pub request_count: i64,
}

#[derive(Debug, Serialize)]
pub struct ModelDetailData {
    pub models: Vec<ModelMeta>,
    pub daily: Vec<ModelDailyDetail>,
}

/// 从 request_logs 读取代理层 token 数据
fn query_request_logs(conn: &Connection, days: u32) -> Result<Vec<(String, String, i64, i64, i64, i64)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT date(timestamp), model_id,
                    SUM(input_tokens), SUM(output_tokens),
                    SUM(cache_read_tokens), COUNT(*)
             FROM request_logs
             WHERE timestamp >= datetime('now', '-' || ?1 || ' days')
               AND model_id IS NOT NULL AND model_id != ''
             GROUP BY date(timestamp), model_id",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![days], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// 从 usage.db 的 call_records 读取性能数据
fn query_call_records(days: u32) -> Result<HashMap<(String, String), (Vec<f64>, f64)>, String> {
    let path = usage_db_path();
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let uconn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("open usage.db: {}", e))?;

    // 探测表结构：先查 call_records 是否存在
    let table_exists: bool = uconn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='call_records'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !table_exists {
        return Ok(HashMap::new());
    }

    // 读取原始延迟值用于 p50/p95 计算，同时取 avg(tps)
    let mut stmt = uconn
        .prepare(
            "SELECT DATE(recorded_at), model_name, latency_ms, tokens_per_sec
             FROM call_records
             WHERE recorded_at >= datetime('now', '-' || ?1 || ' days')
               AND model_name IS NOT NULL AND model_name != ''",
        )
        .map_err(|e| e.to_string())?;

    let mut map: HashMap<(String, String), (Vec<f64>, f64)> = HashMap::new();
    let rows = stmt
        .query_map(rusqlite::params![days], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2).unwrap_or(0.0),
                row.get::<_, f64>(3).unwrap_or(0.0),
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut count_map: HashMap<(String, String), (f64, i64)> = HashMap::new();
    for row in rows.flatten() {
        let (date, model, latency, tps) = row;
        let key = (date, model);
        map.entry(key.clone())
            .or_default()
            .0
            .push(latency);
        let entry = count_map.entry(key).or_insert((0.0, 0));
        entry.0 += tps;
        entry.1 += 1;
    }

    // 计算 avg_tps 写入 map 的 .1
    for (key, (tps_sum, count)) in count_map {
        if let Some(entry) = map.get_mut(&key) {
            entry.1 = if count > 0 { tps_sum / count as f64 } else { 0.0 };
        }
    }

    Ok(map)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() { return 0.0; }
    let idx = ((sorted.len() as f64) * p / 100.0).floor() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[tauri::command]
pub fn get_model_detail_stats(
    state: tauri::State<'_, super::AppState>,
    days: u32,
) -> Result<ModelDetailData, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // 1. request_logs 数据
    let rl_data = query_request_logs(&db, days)?;

    // 2. usage.db 性能数据
    let perf_data = query_call_records(days)?;

    // 3. 合并：按 (date, model) 构建 daily 列表
    let mut daily_map: HashMap<(String, String), ModelDailyDetail> = HashMap::new();

    for (date, model, inp, out, cache, count) in &rl_data {
        let key = (date.clone(), model.clone());
        let entry = daily_map.entry(key.clone()).or_insert(ModelDailyDetail {
            date: date.clone(),
            model: model.clone(),
            input_tokens: 0, output_tokens: 0, cache_read: 0, uncached_input: 0,
            avg_tps: 0.0, avg_latency: 0.0, p50_latency: 0.0, p95_latency: 0.0,
            request_count: 0,
        });
        entry.input_tokens = *inp;
        entry.output_tokens = *out;
        entry.cache_read = *cache;
        entry.uncached_input = (inp - cache).max(0);
        entry.request_count = *count;
    }

    // 填充性能数据
    for ((date, model), (latencies, avg_tps)) in &perf_data {
        let key = (date.clone(), model.clone());
        let entry = daily_map.entry(key.clone()).or_insert(ModelDailyDetail {
            date: date.clone(),
            model: model.clone(),
            input_tokens: 0, output_tokens: 0, cache_read: 0, uncached_input: 0,
            avg_tps: 0.0, avg_latency: 0.0, p50_latency: 0.0, p95_latency: 0.0,
            request_count: 0,
        });
        entry.avg_tps = *avg_tps;
        if !latencies.is_empty() {
            let mut sorted = latencies.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            entry.avg_latency = sorted.iter().sum::<f64>() / sorted.len() as f64;
            entry.p50_latency = percentile(&sorted, 50.0);
            entry.p95_latency = percentile(&sorted, 95.0);
        }
    }

    // 4. 聚合 ModelMeta
    let mut meta_map: HashMap<String, ModelMeta> = HashMap::new();
    for d in daily_map.values() {
        let entry = meta_map.entry(d.model.clone()).or_insert(ModelMeta {
            model: d.model.clone(),
            total_requests: 0, total_input: 0, total_output: 0, total_cache: 0,
            avg_tps: 0.0, p50_latency: 0.0, p95_latency: 0.0,
        });
        entry.total_requests += d.request_count;
        entry.total_input += d.input_tokens;
        entry.total_output += d.output_tokens;
        entry.total_cache += d.cache_read;
    }

    // 性能汇总：对每个模型的所有天的延迟值取分位数
    let mut model_latencies: HashMap<String, Vec<f64>> = HashMap::new();
    let mut model_tps: HashMap<String, Vec<f64>> = HashMap::new();
    for d in daily_map.values() {
        if d.p50_latency > 0.0 {
            model_latencies.entry(d.model.clone()).or_default().push(d.avg_latency);
        }
        if d.avg_tps > 0.0 {
            model_tps.entry(d.model.clone()).or_default().push(d.avg_tps);
        }
    }
    for (model, meta) in meta_map.iter_mut() {
        if let Some(lats) = model_latencies.get(model) {
            let mut sorted = lats.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            meta.p50_latency = percentile(&sorted, 50.0);
            meta.p95_latency = percentile(&sorted, 95.0);
        }
        if let Some(tps_vals) = model_tps.get(model) {
            meta.avg_tps = tps_vals.iter().sum::<f64>() / tps_vals.len() as f64;
        }
    }

    let mut models: Vec<ModelMeta> = meta_map.into_values().collect();
    models.sort_by(|a, b| (b.total_input + b.total_output).cmp(&(a.total_input + a.total_output)));

    let mut daily: Vec<ModelDailyDetail> = daily_map.into_values().collect();
    daily.sort_by(|a, b| b.date.cmp(&a.date).then(a.model.cmp(&b.model)));

    Ok(ModelDetailData { models, daily })
}
```

- [ ] **步骤 2：注册模块和命令**

`src-tauri/src/commands/mod.rs` 顶部添加：

```rust
pub mod metrics;
```

`src-tauri/src/lib.rs` 的 `invoke_handler` 中添加：

```rust
commands::metrics::check_usage_db,
commands::metrics::get_model_detail_stats,
```

- [ ] **步骤 3：构建验证**

```bash
cd /d/ai/AgentBox/src-tauri && cargo build
```

预期：编译通过，无错误

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/commands/metrics.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: metrics.rs — usage.db 读取 + 模型详情查询（合并代理层+call_records）"
```

---

### 任务 5：前端类型 + IPC 绑定

**文件：**
- 修改：`src/lib/tauri.ts`（末尾追加类型和函数）

- [ ] **步骤 1：添加类型和 IPC 函数**

在 `tauri.ts` 末尾 `// Agents` 之前追加：

```typescript
// ── Model Detail (usage.db + proxy) ──────────────────────

export interface ModelMeta {
  model: string;
  total_requests: number;
  total_input: number;
  total_output: number;
  total_cache: number;
  avg_tps: number;
  p50_latency: number;
  p95_latency: number;
}

export interface ModelDailyDetail {
  date: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  cache_read: number;
  uncached_input: number;
  avg_tps: number;
  avg_latency: number;
  p50_latency: number;
  p95_latency: number;
  request_count: number;
}

export interface ModelDetailData {
  models: ModelMeta[];
  daily: ModelDailyDetail[];
}

export const checkUsageDb = () => invoke<boolean>("check_usage_db");
export const getModelDetailStats = (days: number) =>
  invoke<ModelDetailData>("get_model_detail_stats", { days });
```

- [ ] **步骤 2：构建验证 + Commit**

```bash
cd /d/ai/AgentBox && npm run build
git add src/lib/tauri.ts
git commit -m "feat: 前端类型 + IPC 绑定 — ModelDetailData / checkUsageDb / getModelDetailStats"
```

---

### 任务 6：AnalyticsPanel Tab 切换

**文件：**
- 修改：`src/components/analytics/AnalyticsPanel.tsx`（头部 Tab 栏 + 条件渲染）

- [ ] **步骤 1：添加 Tab 状态和 usage.db 检测**

在 `AnalyticsPanel` 组件内添加：

```typescript
const [tab, setTab] = useState<"overview" | "detail">("overview");
const [hasUsageDb, setHasUsageDb] = useState(false);

useEffect(() => {
  checkUsageDb().then(setHasUsageDb).catch(() => setHasUsageDb(false));
}, []);
```

导入 `checkUsageDb`。

- [ ] **步骤 2：修改标题栏为 Tab 切换**

将原有的标题栏改为包含 Tab 按钮：

```tsx
<div className="flex items-center justify-between px-5 h-11 border-b border-[var(--border)] shrink-0">
  <div className="flex items-center gap-1">
    <button
      onClick={() => setTab("overview")}
      className={`px-3 h-7 text-[12px] rounded-md transition-colors ${
        tab === "overview"
          ? "bg-[var(--accent)]/10 text-[var(--accent)] font-medium"
          : "text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
      }`}
    >
      总览
    </button>
    {hasUsageDb && (
      <button
        onClick={() => setTab("detail")}
        className={`px-3 h-7 text-[12px] rounded-md transition-colors ${
          tab === "detail"
            ? "bg-[var(--accent)]/10 text-[var(--accent)] font-medium"
            : "text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
        }`}
      >
        详情
      </button>
    )}
  </div>
  {/* 同步按钮保留 */}
  <button onClick={handleSync} ...>...</button>
</div>
```

- [ ] **步骤 3：条件渲染 Tab 内容**

```tsx
{tab === "overview" ? (
  <div className="flex-1 p-5 space-y-4 overflow-auto">
    {/* 现有总览内容（任务 3 已重构为双栏） */}
  </div>
) : (
  <AnalyticsDetail />
)}
```

`AnalyticsDetail` 先用占位组件（任务 7 实现）：

```tsx
import { AnalyticsDetail } from "./AnalyticsDetail";
```

- [ ] **步骤 4：构建验证 + Commit**

```bash
cd /d/ai/AgentBox && npm run build
git add src/components/analytics/AnalyticsPanel.tsx
git commit -m "feat: AnalyticsPanel Tab 切换 — 总览/详情，详情 Tab 仅在 usage.db 存在时显示"
```

---

### 任务 7：AnalyticsDetail 组件 — 左栏

**文件：**
- 新增：`src/components/analytics/AnalyticsDetail.tsx`

- [ ] **步骤 1：创建组件骨架 + 左栏模型列表**

```tsx
import { useState, useEffect, useCallback, useMemo } from "react";
import { getModelDetailStats } from "@/lib/tauri";
import type { ModelDetailData, ModelMeta, ModelDailyDetail } from "@/lib/tauri";

export function AnalyticsDetail() {
  const [data, setData] = useState<ModelDetailData | null>(null);
  const [loading, setLoading] = useState(true);
  const [selectedModels, setSelectedModels] = useState<Set<string>>(new Set());
  const [days, setDays] = useState(30);
  const [granularity, setGranularity] = useState<"day" | "week">("day");

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const d = await getModelDetailStats(days);
      setData(d);
      setSelectedModels(new Set(d.models.map((m) => m.model)));
    } catch {
      setData(null);
    } finally {
      setLoading(false);
    }
  }, [days]);

  useEffect(() => { load(); }, [load]);

  const toggleModel = (model: string) => {
    setSelectedModels((prev) => {
      const next = new Set(prev);
      if (next.has(model)) next.delete(model);
      else next.add(model);
      return next;
    });
  };

  const toggleAll = () => {
    if (!data) return;
    if (selectedModels.size === data.models.length) {
      setSelectedModels(new Set());
    } else {
      setSelectedModels(new Set(data.models.map((m) => m.model)));
    }
  };

  // 过滤选中模型的 daily 数据
  const filteredDaily = useMemo(() => {
    if (!data) return [];
    return data.daily.filter((d) => selectedModels.has(d.model));
  }, [data, selectedModels]);

  if (loading) {
    return <div className="flex items-center justify-center h-full"><Loader2 size={20} className="animate-spin text-[var(--text-muted)]" /></div>;
  }
  if (!data || data.models.length === 0) {
    return <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">暂无数据</div>;
  }

  return (
    <div className="flex h-full overflow-hidden">
      {/* 左栏 */}
      <div className="w-[220px] shrink-0 border-r border-[var(--border)] bg-[var(--bg-sidebar)] flex flex-col overflow-auto">
        {/* 模型选择标题 */}
        <div className="flex items-center justify-between px-3 h-9 border-b border-[var(--border)]">
          <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">模型选择</span>
          <button onClick={toggleAll} className="text-[10px] text-[var(--accent)] hover:underline">
            {selectedModels.size === data.models.length ? "全不选" : "全选"}
          </button>
        </div>

        {/* 模型列表 */}
        <div className="flex-1 overflow-auto py-1">
          {data.models.map((m) => (
            <div
              key={m.model}
              onClick={() => toggleModel(m.model)}
              className="flex items-start gap-2 px-3 py-1.5 cursor-pointer hover:bg-[var(--bg-hover)] transition-colors"
            >
              <input
                type="checkbox"
                checked={selectedModels.has(m.model)}
                onChange={() => toggleModel(m.model)}
                className="mt-0.5 accent-[var(--accent)]"
              />
              <div className="flex-1 min-w-0">
                <div className="text-[11px] font-mono text-[var(--text-primary)] truncate">{shortModel(m.model)}</div>
                <div className="text-[9px] text-[var(--text-muted)]">
                  {m.total_requests.toLocaleString()} req · {fmtTok(m.total_input + m.total_output)}
                </div>
              </div>
            </div>
          ))}
        </div>

        {/* 时间范围 + 粒度 */}
        <div className="border-t border-[var(--border)] p-2 space-y-2">
          <div className="text-[10px] text-[var(--text-muted)]">时间范围</div>
          <select
            value={days}
            onChange={(e) => setDays(Number(e.target.value))}
            className="w-full h-7 text-[11px] bg-[var(--bg-input)] border border-[var(--border)] rounded px-2 text-[var(--text-primary)]"
          >
            <option value={7}>最近 7 天</option>
            <option value={14}>最近 14 天</option>
            <option value={30}>最近 30 天</option>
            <option value={60}>最近 60 天</option>
            <option value={90}>最近 90 天</option>
          </select>
          <div className="flex gap-2 text-[10px]">
            <label className="flex items-center gap-1 cursor-pointer text-[var(--text-muted)]">
              <input type="radio" name="gran" checked={granularity === "day"} onChange={() => setGranularity("day")} className="accent-[var(--accent)]" /> 日
            </label>
            <label className="flex items-center gap-1 cursor-pointer text-[var(--text-muted)]">
              <input type="radio" name="gran" checked={granularity === "week"} onChange={() => setGranularity("week")} className="accent-[var(--accent)]" /> 周
            </label>
          </div>
        </div>
      </div>

      {/* 图表区 */}
      <div className="flex-1 overflow-auto p-4 space-y-4">
        <div className="text-[var(--text-muted)] text-sm text-center py-8">图表区域（任务 8-9 实现）</div>
      </div>
    </div>
  );
}

// 工具函数复用
function shortModel(model: string): string {
  const parts = model.split("/");
  return parts[parts.length - 1].replace(/-\d{8}$/, "").slice(-24);
}
function fmtTok(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}
```

- [ ] **步骤 2：构建验证 + Commit**

```bash
cd /d/ai/AgentBox && npm run build
git add src/components/analytics/AnalyticsDetail.tsx
git commit -m "feat: AnalyticsDetail 左栏 — 模型多选列表 + 时间范围 + 粒度切换"
```

---

### 任务 8：Token 堆叠面积图

**文件：**
- 新增：`src/components/analytics/TokenAreaChart.tsx`

- [ ] **步骤 1：创建 TokenAreaChart 组件**

SVG 堆叠面积图，复用 `MODEL_COLORS` 配色。每个选中模型 4 层面积（输出→缓存→输入未命中→总量边框线），不同模型用透明度区分。

```tsx
import { useMemo } from "react";
import type { ModelDailyDetail } from "@/lib/tauri";

const AREA_COLORS = {
  output: "#3fb950",
  cache: "#bc8cff",
  uncached: "#58a6ff",
};

interface Props {
  daily: ModelDailyDetail[];
  selectedModels: Set<string>;
  granularity: "day" | "week";
}

export function TokenAreaChart({ daily, selectedModels, granularity }: Props) {
  // 按粒度聚合
  const aggregated = useMemo(() => {
    const filtered = daily.filter((d) => selectedModels.has(d.model));
    if (granularity === "week") {
      // 按 ISO 周聚合
      const map = new Map<string, ModelDailyDetail>();
      for (const d of filtered) {
        const weekStart = getWeekStart(d.date);
        const key = `${weekStart}::${d.model}`;
        const existing = map.get(key);
        if (existing) {
          existing.output_tokens += d.output_tokens;
          existing.cache_read += d.cache_read;
          existing.uncached_input += d.uncached_input;
          existing.input_tokens += d.input_tokens;
        } else {
          map.set(key, { ...d, date: weekStart });
        }
      }
      return [...map.values()];
    }
    return filtered;
  }, [daily, selectedModels, granularity]);

  // 提取日期轴
  const dates = useMemo(() => {
    const set = new Set(aggregated.map((d) => d.date));
    return [...set].sort();
  }, [aggregated]);

  if (dates.length < 2) {
    return <div className="text-[11px] text-[var(--text-muted)] text-center py-4">数据不足</div>;
  }

  const models = [...selectedModels].filter((m) => aggregated.some((d) => d.model === m));
  const W = 600, H = 220, PL = 50, PR = 10, PT = 10, PB = 25;
  const plotW = W - PL - PR, plotH = H - PT - PB;

  // 计算每个日期每个模型的堆叠值
  const stacked = useMemo(() => {
    return dates.map((date) => {
      let total = 0;
      const modelData: Record<string, { output: number; cache: number; uncached: number; total: number }> = {};
      for (const model of models) {
        const d = aggregated.find((a) => a.date === date && a.model === model);
        const output = d?.output_tokens ?? 0;
        const cache = d?.cache_read ?? 0;
        const uncached = d?.uncached_input ?? 0;
        modelData[model] = { output, cache, uncached, total: output + cache + uncached };
        total += output + cache + uncached;
      }
      return { date, total, modelData };
    });
  }, [dates, models, aggregated]);

  const maxY = Math.max(...stacked.map((s) => s.total), 1);
  const xStep = plotW / Math.max(dates.length - 1, 1);
  const xPos = (i: number) => PL + i * xStep;
  const yPos = (v: number) => PT + plotH - (v / maxY) * plotH;

  // 构建每个模型每层的面积路径
  const areas: { model: string; layer: "output" | "cache" | "uncached"; path: string; color: string }[] = [];
  for (const model of models) {
    const layers: ("output" | "cache" | "uncached")[] = ["output", "cache", "uncached"];
    let cumulative = new Array(dates.length).fill(0);
    for (const layer of layers) {
      const topY = stacked.map((s, i) => {
        const val = s.modelData[model]?.[layer] ?? 0;
        return yPos(cumulative[i] + val);
      });
      const bottomY = cumulative.map((v) => yPos(v));
      // 构建面积路径
      const topPoints = topY.map((y, i) => `${xPos(i)},${y}`).join(" L");
      const bottomPoints = bottomY.map((y, i) => `${xPos(dates.length - 1 - i)},${y}`).join(" L");
      const path = `M${xPos(0)},${topY[0]} L${topPoints} L${bottomPoints} Z`;
      areas.push({ model, layer, path, color: AREA_COLORS[layer] });
      cumulative = cumulative.map((v, i) => v + (stacked[i].modelData[model]?.[layer] ?? 0));
    }
  }

  const labelEvery = Math.max(1, Math.floor(dates.length / 8));

  return (
    <div className="space-y-2">
      <svg viewBox={`0 0 ${W} ${H}`} className="w-full h-auto">
        {/* 网格线 */}
        {[0, 0.25, 0.5, 0.75, 1].map((r) => {
          const y = yPos(r * maxY);
          return <g key={r}>
            <line x1={PL} y1={y} x2={W - PR} y2={y} stroke="var(--border)" strokeWidth={0.3} strokeDasharray="2,2" />
            <text x={PL - 4} y={y + 3} textAnchor="end" fill="var(--text-muted)" fontSize={8}>{fmtTok(r * maxY)}</text>
          </g>;
        })}
        {/* X 轴标签 */}
        {dates.map((d, i) => i % labelEvery === 0 ? (
          <text key={d} x={xPos(i)} y={H - PB + 12} textAnchor="middle" fill="var(--text-muted)" fontSize={7}>{d.slice(5)}</text>
        ) : null)}
        {/* 面积 */}
        {areas.map((a, i) => (
          <path key={`${a.model}-${a.layer}`} d={a.path} fill={a.color} opacity={0.3 + (models.indexOf(a.model) * 0.1)} />
        ))}
      </svg>
      {/* 图例 */}
      <div className="flex flex-wrap gap-3">
        {models.map((model) => (
          <div key={model} className="flex items-center gap-1">
            <div className="w-2.5 h-2.5 rounded-full bg-[#58a6ff]" style={{ opacity: 0.3 + models.indexOf(model) * 0.1 }} />
            <span className="text-[10px] text-[var(--text-primary)] font-mono">{shortModel(model)}</span>
          </div>
        ))}
        <span className="text-[10px] text-[var(--text-muted)]">|</span>
        {Object.entries(AREA_COLORS).map(([name, color]) => (
          <div key={name} className="flex items-center gap-1">
            <div className="w-2.5 h-2.5 rounded-sm" style={{ backgroundColor: color, opacity: 0.5 }} />
            <span className="text-[10px] text-[var(--text-muted)]">{name === "output" ? "输出" : name === "cache" ? "缓存" : "输入未命中"}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function getWeekStart(dateStr: string): string {
  const d = new Date(dateStr);
  const day = d.getDay();
  d.setDate(d.getDate() - (day === 0 ? 6 : day - 1));
  return d.toISOString().slice(0, 10);
}

function shortModel(model: string): string {
  return model.split("/").pop()?.replace(/-\d{8}$/, "").slice(-24) ?? model;
}

function fmtTok(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}
```

- [ ] **步骤 2：构建验证 + Commit**

```bash
cd /d/ai/AgentBox && npm run build
git add src/components/analytics/TokenAreaChart.tsx
git commit -m "feat: TokenAreaChart 堆叠面积图 — 输出/缓存/输入未命中三层 + 模型透明度区分"
```

---

### 任务 9：性能折线图 + 汇总卡片

**文件：**
- 新增：`src/components/analytics/PerfLineChart.tsx`

- [ ] **步骤 1：创建 PerfLineChart 组件**

双 Y 轴折线图（左 TPS，右延迟），4 条线可 toggle。

```tsx
import { useState, useMemo } from "react";
import type { ModelDailyDetail } from "@/lib/tauri";

const PERF_LINES = [
  { key: "avg_tps", label: "TPS", color: "#d29922", axis: "left" as const },
  { key: "avg_latency", label: "平均延迟", color: "#58a6ff", axis: "right" as const },
  { key: "p50_latency", label: "P50", color: "#3fb950", axis: "right" as const },
  { key: "p95_latency", label: "P95", color: "#f48771", axis: "right" as const },
];

interface Props {
  daily: ModelDailyDetail[];
  selectedModels: Set<string>;
  granularity: "day" | "week";
  hasPerfData: boolean;
}

export function PerfLineChart({ daily, selectedModels, granularity, hasPerfData }: Props) {
  const [visibleLines, setVisibleLines] = useState<Set<string>>(
    new Set(PERF_LINES.map((l) => l.key))
  );

  const toggleLine = (key: string) => {
    setVisibleLines((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  // 聚合（同 TokenAreaChart 逻辑，按选中模型 + 粒度过滤）
  const aggregated = useMemo(() => {
    const filtered = daily.filter((d) => selectedModels.has(d.model));
    if (granularity === "week") {
      const map = new Map<string, ModelDailyDetail>();
      for (const d of filtered) {
        const weekStart = getWeekStart(d.date);
        const key = `${weekStart}::${d.model}`;
        const existing = map.get(key);
        if (existing) {
          existing.avg_tps = (existing.avg_tps + d.avg_tps) / 2;
          existing.avg_latency = (existing.avg_latency + d.avg_latency) / 2;
          existing.p50_latency = Math.max(existing.p50_latency, d.p50_latency);
          existing.p95_latency = Math.max(existing.p95_latency, d.p95_latency);
        } else {
          map.set(key, { ...d, date: weekStart });
        }
      }
      return [...map.values()];
    }
    return filtered;
  }, [daily, selectedModels, granularity]);

  const dates = useMemo(() => [...new Set(aggregated.map((d) => d.date))].sort(), [aggregated]);
  const models = [...selectedModels].filter((m) => aggregated.some((d) => d.model === m));

  if (!hasPerfData) {
    return (
      <div className="flex flex-col items-center justify-center h-[200px] text-[var(--text-muted)] text-sm gap-1">
        <span>性能数据不可用</span>
        <span className="text-[10px]">未安装 qwen-code-usage 或无 call_records 数据</span>
      </div>
    );
  }

  if (dates.length < 2) {
    return <div className="text-[11px] text-[var(--text-muted)] text-center py-4">数据不足</div>;
  }

  const W = 600, H = 200, PL = 50, PR = 50, PT = 10, PB = 25;
  const plotW = W - PL - PR, plotH = H - PT - PB;

  // 计算双 Y 轴范围
  let maxLeft = 0, maxRight = 0;
  for (const d of aggregated) {
    if (visibleLines.has("avg_tps") && d.avg_tps > maxLeft) maxLeft = d.avg_tps;
    for (const key of ["avg_latency", "p50_latency", "p95_latency"]) {
      if (visibleLines.has(key) && (d as any)[key] > maxRight) maxRight = (d as any)[key];
    }
  }
  if (maxLeft === 0) maxLeft = 1;
  if (maxRight === 0) maxRight = 1;

  const xStep = plotW / Math.max(dates.length - 1, 1);
  const xPos = (i: number) => PL + i * xStep;
  const yLeft = (v: number) => PT + plotH - (v / maxLeft) * plotH;
  const yRight = (v: number) => PT + plotH - (v / maxRight) * plotH;

  const labelEvery = Math.max(1, Math.floor(dates.length / 8));

  return (
    <div className="space-y-2">
      <svg viewBox={`0 0 ${W} ${H}`} className="w-full h-auto">
        {/* 左 Y 轴网格 */}
        {[0, 0.25, 0.5, 0.75, 1].map((r) => {
          const y = yLeft(r * maxLeft);
          return <g key={`l${r}`}>
            <line x1={PL} y1={y} x2={W - PR} y2={y} stroke="var(--border)" strokeWidth={0.3} strokeDasharray="2,2" />
            <text x={PL - 4} y={y + 3} textAnchor="end" fill="#d29922" fontSize={7}>{r === 0 ? "TPS" : (r * maxLeft).toFixed(0)}</text>
          </g>;
        })}
        {/* 右 Y 轴标签 */}
        {[0, 0.5, 1].map((r) => (
          <text key={`r${r}`} x={W - PR + 4} y={yRight(r * maxRight) + 3} fill="#58a6ff" fontSize={7}>{(r * maxRight).toFixed(0)}ms</text>
        ))}
        {/* X 轴标签 */}
        {dates.map((d, i) => i % labelEvery === 0 ? (
          <text key={d} x={xPos(i)} y={H - PB + 12} textAnchor="middle" fill="var(--text-muted)" fontSize={7}>{d.slice(5)}</text>
        ) : null)}
        {/* 折线 */}
        {PERF_LINES.filter((l) => visibleLines.has(l.key)).map((line) => {
          const yFn = line.axis === "left" ? yLeft : yRight;
          return models.map((model) => {
            const points = dates.map((d, i) => {
              const val = aggregated.find((a) => a.date === d && a.model === model)?.[line.key as keyof ModelDailyDetail] as number ?? 0;
              return `${xPos(i)},${yFn(val)}`;
            });
            return (
              <polyline
                key={`${line.key}-${model}`}
                points={points.join(" ")}
                fill="none"
                stroke={line.color}
                strokeWidth={1.5}
                strokeLinejoin="round"
                opacity={models.length > 1 ? 0.5 + 0.3 * models.indexOf(model) : 1}
              />
            );
          });
        })}
      </svg>
      {/* 图例（可点击 toggle） */}
      <div className="flex flex-wrap gap-2">
        {PERF_LINES.map((line) => (
          <button
            key={line.key}
            onClick={() => toggleLine(line.key)}
            className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] transition-opacity ${
              visibleLines.has(line.key) ? "opacity-100" : "opacity-30"
            }`}
          >
            <div className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: line.color }} />
            <span className="text-[var(--text-primary)]">{line.label}</span>
            <span className="text-[var(--text-muted)]">({line.axis === "left" ? "TPS" : "ms"})</span>
          </button>
        ))}
      </div>
    </div>
  );
}

function getWeekStart(dateStr: string): string {
  const d = new Date(dateStr);
  const day = d.getDay();
  d.setDate(d.getDate() - (day === 0 ? 6 : day - 1));
  return d.toISOString().slice(0, 10);
}
```

- [ ] **步骤 2：构建验证 + Commit**

```bash
cd /d/ai/AgentBox && npm run build
git add src/components/analytics/PerfLineChart.tsx
git commit -m "feat: PerfLineChart 双 Y 轴性能折线图 — TPS/延迟(p50/p95) 可 toggle"
```

---

### 任务 10：集成 — AnalyticsDetail 接入图表 + 汇总卡片

**文件：**
- 修改：`src/components/analytics/AnalyticsDetail.tsx`（图表区占位替换为实际组件）

- [ ] **步骤 1：替换图表区占位为 TokenAreaChart + PerfLineChart + 汇总卡片**

在 `AnalyticsDetail.tsx` 的图表区替换占位内容：

```tsx
import { TokenAreaChart } from "./TokenAreaChart";
import { PerfLineChart } from "./PerfLineChart";

// ... 在图表区 div 内：

{/* Token 堆叠面积图 */}
<div className="border border-[var(--border)] rounded-lg overflow-hidden">
  <div className="flex items-center gap-2 px-4 h-8 bg-[var(--bg-sidebar)]">
    <Cpu size={13} className="text-[var(--text-muted)]" />
    <span className="text-[12px] font-medium text-[var(--text-primary)]">Token 用量趋势</span>
  </div>
  <div className="px-4 py-3">
    <TokenAreaChart daily={data!.daily} selectedModels={selectedModels} granularity={granularity} />
  </div>
</div>

{/* 性能折线图 */}
<div className="border border-[var(--border)] rounded-lg overflow-hidden">
  <div className="flex items-center gap-2 px-4 h-8 bg-[var(--bg-sidebar)]">
    <Zap size={13} className="text-[var(--text-muted)]" />
    <span className="text-[12px] font-medium text-[var(--text-primary)]">性能指标</span>
  </div>
  <div className="px-4 py-3">
    <PerfLineChart
      daily={data!.daily}
      selectedModels={selectedModels}
      granularity={granularity}
      hasPerfData={data!.models.some((m) => m.avg_tps > 0)}
    />
  </div>
</div>

{/* 汇总统计卡片 */}
<div className="grid grid-cols-3 md:grid-cols-6 gap-3">
  {/* 从 data.models 中按 selectedModels 过滤后汇总 */}
  <StatCard ... />
</div>
```

汇总卡片的数值从 `data.models` 按 `selectedModels` 过滤后求和计算。

- [ ] **步骤 2：构建验证**

```bash
cd /d/ai/AgentBox && npm run build
```

- [ ] **步骤 3：Commit**

```bash
git add src/components/analytics/AnalyticsDetail.tsx
git commit -m "feat: AnalyticsDetail 集成 — TokenAreaChart + PerfLineChart + 汇总卡片"
```

---

### 任务 11：全量构建验证

- [ ] **步骤 1：前端构建**

```bash
cd /d/ai/AgentBox && npm run build
```

预期：无错误

- [ ] **步骤 2：后端构建**

```bash
cd /d/ai/AgentBox/src-tauri && cargo build
```

预期：无错误

- [ ] **步骤 3：功能验证清单**

手动验证：
1. 总览页：双栏布局正常，热力图缩小，项目统计只显示 5 个
2. 详情 Tab：仅在 usage.db 存在时显示
3. 详情页：左栏模型列表可多选，图表随选择更新
4. 记忆面板：展开项目分组后点击子项，中栏正确显示内容
5. 会话面板：展开项目后点击会话，中栏显示消息；列表超过 30 条时显示"加载更多"

- [ ] **步骤 4：最终 Commit**

```bash
git add -A
git commit -m "feat: 用量分析详情页 + 总览双栏布局 + GenericPanel 交互修复"
```
