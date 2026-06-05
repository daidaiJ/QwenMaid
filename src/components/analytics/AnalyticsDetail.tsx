import { useState, useEffect, useCallback, useMemo } from "react";
import { getModelDetailStats, getProxyDetailStats } from "@/lib/tauri";
import type { ModelDetailData } from "@/lib/tauri";
import { Loader2, Cpu, Zap, Server } from "lucide-react";

type DataSource = "usage" | "proxy";

export function AnalyticsDetail() {
  const [data, setData] = useState<ModelDetailData | null>(null);
  const [loading, setLoading] = useState(true);
  const [selectedModels, setSelectedModels] = useState<Set<string>>(new Set());
  const [days, setDays] = useState(30);
  const [granularity, setGranularity] = useState<"day" | "week">("day");
  const [source, setSource] = useState<DataSource>("usage");
  const [isInitialLoad, setIsInitialLoad] = useState(true);

  const load = useCallback(async () => {
    // 仅首次加载显示全屏 spinner，Tab 切换时保留旧数据
    if (isInitialLoad) setLoading(true);
    try {
      const d = await (source === "proxy" ? getProxyDetailStats(days) : getModelDetailStats(days));
      setData(d);
      setSelectedModels(new Set(d.models.map((m) => m.model)));
    } catch {
      // 查询失败时 data 保持旧值，右侧面板显示空状态占位
    } finally {
      setLoading(false);
      setIsInitialLoad(false);
    }
  }, [days, source, isInitialLoad]);

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

  const filteredDaily = useMemo(() => {
    if (!data) return [];
    return data.daily.filter((d) => selectedModels.has(d.model));
  }, [data, selectedModels]);

  if (loading && !data) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 size={20} className="animate-spin text-[var(--text-muted)]" />
      </div>
    );
  }

  const hasData = data && data.models.length > 0;
  const summaryStats = hasData ? computeSummary(data.models.filter((m) => selectedModels.has(m.model))) : null;

  return (
    <div className="flex h-full overflow-hidden">
      {/* 左栏：数据源切换 + 模型选择 + 控制（始终渲染，保持 Tab 可切换） */}
      <div className="w-[220px] shrink-0 border-r border-[var(--border)] bg-[var(--bg-sidebar)] flex flex-col">
        {/* 数据源切换 Tab */}
        <div className="flex border-b border-[var(--border)]">
          <button
            onClick={() => setSource("usage")}
            className={`flex-1 flex items-center justify-center gap-1.5 h-8 text-[11px] font-medium transition-colors ${
              source === "usage"
                ? "text-[var(--accent)] border-b-2 border-[var(--accent)] bg-[var(--accent-light)]"
                : "text-[var(--text-muted)] hover:text-[var(--text-secondary)]"
            }`}
          >
            <Zap size={12} />
            状态行 usage
          </button>
          <button
            onClick={() => setSource("proxy")}
            className={`flex-1 flex items-center justify-center gap-1.5 h-8 text-[11px] font-medium transition-colors ${
              source === "proxy"
                ? "text-[var(--accent)] border-b-2 border-[var(--accent)] bg-[var(--accent-light)]"
                : "text-[var(--text-muted)] hover:text-[var(--text-secondary)]"
            }`}
          >
            <Server size={12} />
            本地路由代理
          </button>
        </div>

        {hasData && (
          <>
            <div className="flex items-center justify-between px-3 h-9 border-b border-[var(--border)]">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">模型选择</span>
              <button onClick={toggleAll} className="text-[10px] text-[var(--accent)] hover:underline">
                {selectedModels.size === data.models.length ? "全不选" : "全选"}
              </button>
            </div>

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
          </>
        )}

        <div className="border-t border-[var(--border)] p-2 space-y-2">
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
          <div className="flex gap-3 text-[10px]">
            <label className="flex items-center gap-1 cursor-pointer text-[var(--text-muted)]">
              <input type="radio" name="gran" checked={granularity === "day"} onChange={() => setGranularity("day")} className="accent-[var(--accent)]" /> 日
            </label>
            <label className="flex items-center gap-1 cursor-pointer text-[var(--text-muted)]">
              <input type="radio" name="gran" checked={granularity === "week"} onChange={() => setGranularity("week")} className="accent-[var(--accent)]" /> 周
            </label>
          </div>
        </div>
      </div>

      {/* 右侧：图表区 */}
      <div className="flex-1 overflow-auto p-4 space-y-4">
        {/* Token 堆叠面积图 */}
        <div className="border border-[var(--border)] rounded-lg overflow-hidden">
          <div className="flex items-center gap-2 px-4 h-8 bg-[var(--bg-sidebar)]">
            <Cpu size={13} className="text-[var(--text-muted)]" />
            <span className="text-[12px] font-medium text-[var(--text-primary)]">Token 用量趋势</span>
          </div>
          <div className="px-4 py-3">
            {hasData ? (
              <TokenAreaChart daily={filteredDaily} selectedModels={selectedModels} granularity={granularity} />
            ) : (
              <EmptyChartPlaceholder message={source === "proxy" ? "无代理请求记录" : "暂无模型详情数据"} />
            )}
          </div>
        </div>

        {/* 性能折线图 */}
        <div className="border border-[var(--border)] rounded-lg overflow-hidden">
          <div className="flex items-center gap-2 px-4 h-8 bg-[var(--bg-sidebar)]">
            <Zap size={13} className="text-[var(--text-muted)]" />
            <span className="text-[12px] font-medium text-[var(--text-primary)]">性能指标</span>
          </div>
          <div className="px-4 py-3">
            {hasData ? (
              <PerfLineChart
                daily={filteredDaily}
                selectedModels={selectedModels}
                granularity={granularity}
                hasPerfData={data.models.some((m) => m.avg_tps > 0)}
              />
            ) : (
              <EmptyChartPlaceholder message={source === "proxy" ? "无代理请求记录" : "暂无模型详情数据"} />
            )}
          </div>
        </div>

        {/* 汇总统计卡片 */}
        <div className="grid grid-cols-3 md:grid-cols-6 gap-3">
          {hasData ? (
            <>
              <MiniStat label="总请求" value={summaryStats!.totalRequests.toLocaleString()} color="#58a6ff" />
              <MiniStat label="总 Token" value={fmtTok(summaryStats!.totalTokens)} color="#d29922" />
              <MiniStat label="缓存率" value={`${(summaryStats!.cacheRate * 100).toFixed(1)}%`} color="#bc8cff" />
              <MiniStat label="平均 TPS" value={summaryStats!.avgTps.toFixed(1)} color="#d29922" />
              <MiniStat label="P50 延迟" value={`${summaryStats!.p50Latency.toFixed(0)}ms`} color="#3fb950" />
              <MiniStat label="P95 延迟" value={`${summaryStats!.p95Latency.toFixed(0)}ms`} color="#f48771" />
            </>
          ) : (
            <>
              <MiniStat label="总请求" value="--" color="#58a6ff" />
              <MiniStat label="总 Token" value="--" color="#d29922" />
              <MiniStat label="缓存率" value="--" color="#bc8cff" />
              <MiniStat label="平均 TPS" value="--" color="#d29922" />
              <MiniStat label="P50 延迟" value="--" color="#3fb950" />
              <MiniStat label="P95 延迟" value="--" color="#f48771" />
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Token 堆叠面积图 ─────────────────────────────────────

const TOKEN_LINES = [
  { key: "uncached_input", label: "输入未命中缓存", color: "#58a6ff" },
  { key: "output_tokens", label: "输出", color: "#3fb950" },
  { key: "cache_read", label: "缓存", color: "#bc8cff" },
];

function TokenAreaChart({ daily, selectedModels, granularity }: {
  daily: import("@/lib/tauri").ModelDailyDetail[];
  selectedModels: Set<string>;
  granularity: "day" | "week";
}) {
  const [visibleLines, setVisibleLines] = useState<Set<string>>(
    new Set(TOKEN_LINES.map((l) => l.key))
  );
  const toggleLine = (key: string) => {
    setVisibleLines((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key); else next.add(key);
      return next;
    });
  };

  const aggregated = useMemo(() => {
    if (granularity === "week") {
      const map = new Map<string, import("@/lib/tauri").ModelDailyDetail>();
      for (const d of daily) {
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
    return daily;
  }, [daily, granularity]);

  const dates = useMemo(() => [...new Set(aggregated.map((d) => d.date))].sort(), [aggregated]);
  const models = [...selectedModels].filter((m) => aggregated.some((d) => d.model === m));

  if (dates.length < 2) return <div className="text-[11px] text-[var(--text-muted)] text-center py-4">数据不足</div>;

  const W = 600, H = 200, PL = 50, PR = 10, PT = 10, PB = 25;
  const plotW = W - PL - PR, plotH = H - PT - PB;

  let maxY = 0;
  for (const d of aggregated) {
    for (const line of TOKEN_LINES) {
      if (visibleLines.has(line.key)) {
        const val = (d as any)[line.key] ?? 0;
        if (val > maxY) maxY = val;
      }
    }
  }
  if (maxY === 0) maxY = 1;

  const xStep = plotW / Math.max(dates.length - 1, 1);
  const xPos = (i: number) => PL + i * xStep;
  const yPos = (v: number) => PT + plotH - (v / maxY) * plotH;
  const labelEvery = Math.max(1, Math.floor(dates.length / 8));

  return (
    <div className="space-y-2">
      <svg viewBox={`0 0 ${W} ${H}`} className="w-full h-auto">
        {[0, 0.25, 0.5, 0.75, 1].map((r) => {
          const y = yPos(r * maxY);
          return <g key={r}>
            <line x1={PL} y1={y} x2={W - PR} y2={y} stroke="var(--border)" strokeWidth={0.3} strokeDasharray="2,2" />
            <text x={PL - 4} y={y + 3} textAnchor="end" fill="var(--text-muted)" fontSize={8}>{fmtTok(r * maxY)}</text>
          </g>;
        })}
        {dates.map((d, i) => i % labelEvery === 0 ? (
          <text key={d} x={xPos(i)} y={H - PB + 12} textAnchor="middle" fill="var(--text-muted)" fontSize={7}>{d.slice(5)}</text>
        ) : null)}
        {TOKEN_LINES.filter((l) => visibleLines.has(l.key)).map((line) => {
          return models.map((model) => {
            const pts = dates.map((d, i) => {
              const dd = aggregated.find((a) => a.date === d && a.model === model);
              return { x: xPos(i), y: yPos((dd as any)?.[line.key] ?? 0) };
            });
            return (
              <path key={`${line.key}-${model}`} d={smoothPath(pts)} fill="none" stroke={line.color}
                strokeWidth={1.5} strokeLinejoin="round" opacity={models.length > 1 ? 0.5 + 0.3 * models.indexOf(model) : 1} />
            );
          });
        })}
      </svg>
      <div className="flex flex-wrap gap-2">
        {TOKEN_LINES.map((line) => (
          <button key={line.key} onClick={() => toggleLine(line.key)}
            className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] transition-opacity ${visibleLines.has(line.key) ? "opacity-100" : "opacity-30"}`}>
            <div className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: line.color }} />
            <span className="text-[var(--text-primary)]">{line.label}</span>
          </button>
        ))}
        {models.length > 1 && models.map((model, i) => (
          <div key={model} className="flex items-center gap-1 ml-2">
            <div className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: "#58a6ff", opacity: 0.5 + 0.3 * i }} />
            <span className="text-[9px] text-[var(--text-muted)] font-mono">{shortModel(model)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── 性能折线图 ───────────────────────────────────────────

const PERF_LINES = [
  { key: "avg_tps", label: "TPS", color: "#d29922", axis: "left" as const },
  { key: "avg_latency", label: "平均延迟", color: "#58a6ff", axis: "right" as const },
  { key: "p50_latency", label: "P50", color: "#3fb950", axis: "right" as const },
  { key: "p95_latency", label: "P95", color: "#f48771", axis: "right" as const },
];

function PerfLineChart({ daily, selectedModels, granularity, hasPerfData }: {
  daily: import("@/lib/tauri").ModelDailyDetail[];
  selectedModels: Set<string>;
  granularity: "day" | "week";
  hasPerfData: boolean;
}) {
  const [visibleLines, setVisibleLines] = useState<Set<string>>(new Set(PERF_LINES.map((l) => l.key)));

  const toggleLine = (key: string) => {
    setVisibleLines((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key); else next.add(key);
      return next;
    });
  };

  const aggregated = useMemo(() => {
    if (granularity === "week") {
      const map = new Map<string, import("@/lib/tauri").ModelDailyDetail>();
      for (const d of daily) {
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
    return daily;
  }, [daily, granularity]);

  const dates = useMemo(() => [...new Set(aggregated.map((d) => d.date))].sort(), [aggregated]);
  const models = [...selectedModels].filter((m) => aggregated.some((d) => d.model === m));

  if (!hasPerfData) {
    return (
      <div className="flex flex-col items-center justify-center h-[180px] text-[var(--text-muted)] text-sm gap-1">
        <span>性能数据不可用</span>
        <span className="text-[10px]">未安装 qwen-code-usage</span>
      </div>
    );
  }

  if (dates.length < 2) return <div className="text-[11px] text-[var(--text-muted)] text-center py-4">数据不足</div>;

  const W = 600, H = 180, PL = 50, PR = 50, PT = 10, PB = 25;
  const plotW = W - PL - PR, plotH = H - PT - PB;

  let maxLeft = 0, maxRight = 0;
  for (const d of aggregated) {
    if (visibleLines.has("avg_tps") && d.avg_tps > maxLeft) maxLeft = d.avg_tps;
    for (const key of ["avg_latency", "p50_latency", "p95_latency"]) {
      const val = (d as any)[key] ?? 0;
      if (visibleLines.has(key) && val > maxRight) maxRight = val;
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
        {[0, 0.5, 1].map((r) => {
          const y = yLeft(r * maxLeft);
          return <g key={`l${r}`}>
            <line x1={PL} y1={y} x2={W - PR} y2={y} stroke="var(--border)" strokeWidth={0.3} strokeDasharray="2,2" />
            <text x={PL - 4} y={y + 3} textAnchor="end" fill="#d29922" fontSize={7}>{r === 0 ? "TPS" : (r * maxLeft).toFixed(0)}</text>
          </g>;
        })}
        {[0, 0.5, 1].map((r) => (
          <text key={`r${r}`} x={W - PR + 4} y={yRight(r * maxRight) + 3} fill="#58a6ff" fontSize={7}>{(r * maxRight).toFixed(0)}ms</text>
        ))}
        {dates.map((d, i) => i % labelEvery === 0 ? (
          <text key={d} x={xPos(i)} y={H - PB + 12} textAnchor="middle" fill="var(--text-muted)" fontSize={7}>{d.slice(5)}</text>
        ) : null)}
        {PERF_LINES.filter((l) => visibleLines.has(l.key)).map((line) => {
          const yFn = line.axis === "left" ? yLeft : yRight;
          return models.map((model) => {
            const points = dates.map((d, i) => {
              const dd = aggregated.find((a) => a.date === d && a.model === model);
              const val = dd ? (dd as any)[line.key] ?? 0 : 0;
              return { x: xPos(i), y: yFn(val) };
            });
            const pathD = smoothPath(points);
            return (
              <path key={`${line.key}-${model}`} d={pathD} fill="none" stroke={line.color}
                strokeWidth={1.5} strokeLinejoin="round" opacity={models.length > 1 ? 0.5 + 0.3 * models.indexOf(model) : 1} />
            );
          });
        })}
      </svg>
      <div className="flex flex-wrap gap-2">
        {PERF_LINES.map((line) => (
          <button key={line.key} onClick={() => toggleLine(line.key)}
            className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] transition-opacity ${visibleLines.has(line.key) ? "opacity-100" : "opacity-30"}`}>
            <div className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: line.color }} />
            <span className="text-[var(--text-primary)]">{line.label}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

// ── 汇总统计小卡片 ──────────────────────────────────────

function MiniStat({ label, value, color }: { label: string; value: string; color: string }) {
  return (
    <div className="border border-[var(--border)] rounded-lg p-2.5 space-y-0.5">
      <div className="text-[9px] text-[var(--text-muted)] uppercase">{label}</div>
      <div className="text-[14px] font-mono font-semibold" style={{ color }}>{value}</div>
    </div>
  );
}

function EmptyChartPlaceholder({ message }: { message: string }) {
  return (
    <div className="flex flex-col items-center justify-center h-[160px] gap-2 text-[var(--text-muted)]">
      <Zap size={20} className="opacity-20" />
      <span className="text-[11px]">{message}</span>
    </div>
  );
}

// ── 工具函数 ─────────────────────────────────────────────

function shortModel(model: string): string {
  return model.split("/").pop()?.replace(/-\d{8}$/, "").slice(-24) ?? model;
}

function fmtTok(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}

function getWeekStart(dateStr: string): string {
  const d = new Date(dateStr);
  const day = d.getDay();
  d.setDate(d.getDate() - (day === 0 ? 6 : day - 1));
  return d.toISOString().slice(0, 10);
}

function computeSummary(models: import("@/lib/tauri").ModelMeta[]) {
  let totalRequests = 0, totalTokens = 0, totalCache = 0, totalInput = 0;
  let tpsSum = 0, tpsCount = 0;
  const p50s: number[] = [], p95s: number[] = [];
  for (const m of models) {
    totalRequests += m.total_requests;
    totalTokens += m.total_input + m.total_output;
    totalCache += m.total_cache;
    totalInput += m.total_input;
    if (m.avg_tps > 0) { tpsSum += m.avg_tps; tpsCount++; }
    if (m.p50_latency > 0) p50s.push(m.p50_latency);
    if (m.p95_latency > 0) p95s.push(m.p95_latency);
  }
  return {
    totalRequests,
    totalTokens,
    cacheRate: totalInput > 0 ? totalCache / totalInput : 0,
    avgTps: tpsCount > 0 ? tpsSum / tpsCount : 0,
    p50Latency: p50s.length > 0 ? p50s.sort((a, b) => a - b)[Math.floor(p50s.length / 2)] : 0,
    p95Latency: p95s.length > 0 ? p95s.sort((a, b) => a - b)[Math.floor(p95s.length * 0.95)] : 0,
  };
}

/** D3 风格 monotone 三次 Hermite 插值 —— 保证平滑且不过冲 */
function smoothPath(points: { x: number; y: number }[]): string {
  const n = points.length;
  if (n < 2) return "";
  if (n === 2) return `M${points[0].x},${points[0].y} L${points[1].x},${points[1].y}`;

  const dx: number[] = [], dy: number[] = [], m: number[] = [];
  for (let i = 0; i < n - 1; i++) {
    dx[i] = points[i + 1].x - points[i].x;
    dy[i] = points[i + 1].y - points[i].y;
    m[i] = dy[i] / dx[i];
  }

  const tangents: number[] = [m[0]];
  for (let i = 1; i < n - 1; i++) {
    tangents[i] = m[i - 1] * m[i] <= 0 ? 0 : (3 * (dx[i - 1] + dx[i])) / ((2 * dx[i] + dx[i - 1]) / m[i - 1] + (dx[i] + 2 * dx[i - 1]) / m[i]);
  }
  tangents[n - 1] = m[n - 2];

  for (let i = 0; i < n - 1; i++) {
    if (Math.abs(m[i]) < 1e-12) { tangents[i] = 0; tangents[i + 1] = 0; }
    else {
      const a = tangents[i] / m[i], b = tangents[i + 1] / m[i], s = a * a + b * b;
      if (s > 9) { const t = 3 / Math.sqrt(s); tangents[i] = t * a * m[i]; tangents[i + 1] = t * b * m[i]; }
    }
  }

  let d = `M${points[0].x},${points[0].y}`;
  for (let i = 0; i < n - 1; i++) {
    const p1 = points[i], p2 = points[i + 1];
    d += ` C${p1.x + dx[i] / 3},${p1.y + tangents[i] * dx[i] / 3} ${p2.x - dx[i] / 3},${p2.y - tangents[i + 1] * dx[i] / 3} ${p2.x},${p2.y}`;
  }
  return d;
}
