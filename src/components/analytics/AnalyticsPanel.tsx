import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { syncSessionStats, getAnalyticsSummary, getAnalyticsTopItems } from "@/lib/tauri";
import type { AnalyticsSummary, AnalyticsTopItems, ModelDailyRow } from "@/lib/tauri";
import { AnalyticsDetail } from "./AnalyticsDetail";
import {
  BarChart3,
  RefreshCw,
  MessageSquare,
  Zap,
  FolderOpen,
  Wrench,
  Cpu,
  Loader2,
  Database,
} from "lucide-react";

export function AnalyticsPanel() {
  const [data, setData] = useState<AnalyticsSummary | null>(null);
  const [topItems, setTopItems] = useState<AnalyticsTopItems | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [loading, setLoading] = useState(true);
  const [syncedCount, setSyncedCount] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"overview" | "detail">("overview");

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [summary, tops] = await Promise.all([
        getAnalyticsSummary(),
        getAnalyticsTopItems(),
      ]);
      setData(summary);
      setTopItems(tops);
    } catch (e) {
      setData(null);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleSync = async () => {
    setSyncing(true);
    try {
      const count = await syncSessionStats();
      setSyncedCount(count);
      await load();
    } catch {
      setSyncedCount(-1);
    } finally {
      setSyncing(false);
    }
  };

  if (loading && !data) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 size={20} className="text-[var(--text-muted)] animate-spin" />
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-auto">
      <div className="flex items-center justify-between px-5 h-11 border-b border-[var(--border)] shrink-0 bg-[var(--bg-sidebar)]/30">
        <div className="flex items-center gap-1">
          <BarChart3 size={14} className="text-[var(--text-muted)] mr-1" />
          <button
            onClick={() => setTab("overview")}
            className={`px-3 h-7 text-[12px] rounded-lg transition-colors ${
              tab === "overview"
                ? "bg-[var(--accent)]/10 text-[var(--accent)] font-medium"
                : "text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
            }`}
          >
            总览
          </button>
          <button
            onClick={() => setTab("detail")}
            className={`px-3 h-7 text-[12px] rounded-lg transition-colors ${
              tab === "detail"
                ? "bg-[var(--accent)]/10 text-[var(--accent)] font-medium"
                : "text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
            }`}
          >
            详情
          </button>
          {syncedCount !== null && syncedCount >= 0 && (
            <span className="text-[10px] text-[#3fb950] ml-2">同步了 {syncedCount} 个会话</span>
          )}
        </div>
        <button
          onClick={handleSync}
          disabled={syncing}
          className="flex items-center gap-1.5 px-2.5 h-7 text-[11px] text-[var(--text-secondary)] rounded-lg hover:bg-[var(--bg-hover)] disabled:opacity-40 transition-colors"
        >
          <RefreshCw size={12} className={syncing ? "animate-spin" : ""} />
          {syncing ? "同步中…" : "同步数据"}
        </button>
      </div>

      {tab === "detail" ? (
        <AnalyticsDetail />
      ) : (
      <div className="flex-1 p-5 space-y-4 overflow-auto">
        {data && data.total_sessions === 0 && (
          <div className="flex flex-col items-center justify-center py-12 gap-2 text-[var(--text-muted)]">
            <Zap size={28} className="opacity-30" />
            <span className="text-sm">暂无会话数据</span>
            {error && <span className="text-[11px] text-[var(--color-error)] max-w-md text-center">{error}</span>}
            <span className="text-[11px]">点击「同步数据」扫描项目下的会话文件</span>
          </div>
        )}
        {!data && !loading && (
          <div className="flex flex-col items-center justify-center py-12 gap-3 text-[var(--text-muted)]">
            <Zap size={28} className="opacity-30" />
            <span className="text-sm">数据加载失败</span>
            {error && <span className="text-[11px] text-[var(--color-error)] max-w-md text-center break-all">{error}</span>}
            <button
              onClick={handleSync}
              disabled={syncing}
              className="flex items-center gap-1.5 px-4 h-8 text-[12px] text-[var(--accent)] border border-[var(--accent)]/30 rounded-md hover:bg-[var(--accent)]/10 disabled:opacity-40 transition-colors"
            >
              <RefreshCw size={12} className={syncing ? "animate-spin" : ""} />
              {syncing ? "同步中…" : "同步数据"}
            </button>
          </div>
        )}

        {data && data.total_sessions > 0 && (
          <>
            {/* ── Row 1: 汇总卡片 ── */}
            <div className="flex flex-wrap gap-2 content-start">
              <StatCard icon={<MessageSquare size={13} />} label="总会话" value={data.total_sessions.toLocaleString()} color="#58a6ff" />
              <StatCard icon={<MessageSquare size={13} />} label="总消息" value={data.total_messages.toLocaleString()} color="#3fb950" />
              <StatCard icon={<Zap size={13} />} label="总 Token" value={fmtTok(data.total_input_tokens + data.total_output_tokens)} color="#d29922" />
              <StatCard icon={<Database size={13} />} label="缓存命中" value={fmtTok(data.total_cache_read)} color="#bc8cff" />
              <StatCard icon={<Zap size={13} />} label="活跃天" value={String(data.active_days)} color="#58a6ff" />
              <StatCard icon={<Wrench size={13} />} label="工具调用" value={(topItems?.top_tools ?? []).reduce((s, t) => s + t.count, 0).toLocaleString()} color="#d29922" />
            </div>

            {/* ── Row 2: 项目统计(上) | I/O(下)(左半) ─ 模型趋势(右半) ── */}
            <div className="grid grid-cols-[1fr_4fr] gap-4">
              <div className="space-y-4">
                {data.project_stats.length > 0 && (
                  <Section title="项目统计 Top 5" icon={<FolderOpen size={13} />}>
                    <div className="space-y-0">
                      {data.project_stats.map((p) => (
                        <div key={p.project} className="flex items-center gap-1.5 h-5 px-0.5 rounded hover:bg-[var(--bg-hover)]">
                          <span className="text-[11px] text-[var(--text-primary)] truncate font-mono flex-1 min-w-0">{decodeProject(p.project)}</span>
                          <span className="text-[9px] text-[var(--text-muted)] shrink-0">{p.session_count}s</span>
                          <span className="text-[9px] font-mono text-[var(--text-muted)] shrink-0">{fmtTok(p.total_tokens)}</span>
                        </div>
                      ))}
                    </div>
                  </Section>
                )}
                <div className="border border-[var(--border)] rounded-xl p-3 space-y-2 bg-[var(--bg-card)]">
                  {(() => {
                    const inp = data.total_input_tokens;
                    const out = data.total_output_tokens;
                    const cache = data.total_cache_read;
                    const maxVal = Math.max(inp, out, cache, 1);
                    const rows = [
                      { label: "输入", value: inp, color: "#58a6ff" },
                      { label: "缓存", value: cache, color: "#bc8cff" },
                      { label: "输出", value: out, color: "#3fb950" },
                    ];
                    return rows.map((r, i) => {
                      const pct = (r.value / maxVal) * 100;
                      const barW = 100 - i * 15; // 梯形递减：100% / 85% / 70%
                      return (
                        <div key={r.label} className="space-y-0.5">
                          <div className="flex items-center justify-between">
                            <span className="text-[10px] text-[var(--text-muted)]">{r.label}</span>
                            <span className="text-[12px] font-mono text-[var(--text-primary)]">{fmtTok(r.value)}</span>
                          </div>
                          <div className="h-2 bg-[var(--bg-input)] rounded-full overflow-hidden" style={{ width: `${barW}%` }}>
                            <div className="h-full rounded-full" style={{ width: `${Math.min(pct, 100)}%`, backgroundColor: r.color, opacity: 0.7 }} />
                          </div>
                        </div>
                      );
                    });
                  })()}
                </div>
              </div>
              {data.model_daily.length > 0 && (
                <Section title="模型趋势（近 30 天）" icon={<Cpu size={13} />}>
                  <ModelLineChart rows={data.model_daily} days={30} height={90} />
                </Section>
              )}
            </div>

            {/* ── Row 3: 模型排名(左) + 工具排行(右) ── */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {data.top_models.length > 0 && (
                <Section title="模型用量排名" icon={<Cpu size={13} />}>
                  <ModelRankingTable models={data.top_models} />
                </Section>
              )}
              {(topItems?.top_tools ?? []).length > 0 && (
                <Section title="工具调用排行" icon={<Wrench size={13} />}>
                  <BarList items={topItems!.top_tools} max={topItems!.top_tools[0]?.count ?? 1} color="#d29922" />
                </Section>
              )}
            </div>

            {/* ── Row 4: 技能调用(左) + 子智能体(右) ── */}
            {((topItems?.top_skills ?? []).length > 0 || (topItems?.top_agents ?? []).length > 0) && (
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                {(topItems?.top_skills ?? []).length > 0 && (
                  <Section title="技能调用排行" icon={<Zap size={13} />}>
                    <BarList items={topItems!.top_skills} max={topItems!.top_skills[0]?.count ?? 1} color="#bc8cff" />
                  </Section>
                )}
                {(topItems?.top_agents ?? []).length > 0 && (
                  <Section title="子智能体调用" icon={<Cpu size={13} />}>
                    <BarList items={topItems!.top_agents} max={topItems!.top_agents[0]?.count ?? 1} color="#58a6ff" />
                  </Section>
                )}
              </div>
            )}
          </>
        )}
      </div>
      )}
    </div>
  );
}

// ════════════════════════════════════════════════════════════
// 子组件
// ════════════════════════════════════════════════════════════

function StatCard({ icon, label, value, color }: { icon: React.ReactNode; label: string; value: string; color: string }) {
  return (
    <div className="bg-[var(--bg-card)] rounded-xl shadow-[var(--shadow-card)] px-5 py-3 flex items-center gap-2.5 flex-1 min-w-[120px]">
      <span style={{ color }} className="opacity-70 shrink-0">{icon}</span>
      <span className="text-[11px] text-[var(--text-muted)] shrink-0">{label}</span>
      <span className="text-[17px] font-mono font-semibold text-[var(--text-primary)] ml-auto">{value}</span>
    </div>
  );
}

function Section({ title, icon, children }: { title: string; icon: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="bg-[var(--bg-card)] rounded-xl shadow-[var(--shadow-card)] overflow-hidden min-w-0">
      <div className="flex items-center gap-2 px-4 h-9 bg-[var(--bg-sidebar)]/50">
        <span className="text-[var(--text-muted)]">{icon}</span>
        <span className="text-[12px] font-medium text-[var(--text-primary)]">{title}</span>
      </div>
      <div className="px-4 py-3 min-w-0">{children}</div>
    </div>
  );
}

function BarList({ items, max, color }: { items: { name: string; count: number }[]; max: number; color: string }) {
  return (
    <div className="space-y-1">
      {items.map((item) => (
        <div key={item.name} className="flex items-center gap-2 h-6">
          <span className="text-[11px] font-mono text-[var(--text-primary)] w-32 truncate shrink-0">{item.name}</span>
          <div className="flex-1 h-2 bg-[var(--bg-input)] rounded-full overflow-hidden">
            <div className="h-full rounded-full" style={{ width: `${(item.count / max) * 100}%`, backgroundColor: color, opacity: 0.6 }} />
          </div>
          <span className="text-[10px] text-[var(--text-muted)] w-10 text-right shrink-0 font-mono">{item.count}</span>
        </div>
      ))}
    </div>
  );
}

// ── 模型用量折线图（SVG） ────────────────────────────────

const MODEL_COLORS = ["#58a6ff", "#3fb950", "#d29922", "#bc8cff", "#f48771", "#79c0ff", "#56d364", "#e3b341"];

function ModelLineChart({ rows, days: limitDays, height }: { rows: ModelDailyRow[]; days?: number; height?: number }) {
  const [range, setRange] = useState<[number, number]>([0, 1]);

  // 过滤最近 N 天的数据
  const filteredRows = useMemo(() => {
    if (!limitDays) return rows;
    const cutoff = new Date();
    cutoff.setDate(cutoff.getDate() - limitDays);
    const cutoffStr = cutoff.toISOString().slice(0, 10);
    return rows.filter((r) => r.date >= cutoffStr);
  }, [rows, limitDays]);

  // 按模型分组
  const byModel = useMemo(() => {
    const map = new Map<string, { date: string; tokens: number }[]>();
    for (const r of filteredRows) {
      const arr = map.get(r.model) ?? [];
      arr.push({ date: r.date, tokens: r.input_tokens + r.output_tokens });
      map.set(r.model, arr);
    }
    for (const [, arr] of map) arr.sort((a, b) => a.date.localeCompare(b.date));
    return map;
  }, [rows]);

  // 全量日期
  const allDates = useMemo(() => {
    const set = new Set(filteredRows.map((r) => r.date));
    return [...set].sort();
  }, [filteredRows]);

  // 按 range 裁剪可见日期
  const visibleDates = useMemo(() => {
    const start = Math.floor(range[0] * allDates.length);
    const end = Math.ceil(range[1] * allDates.length);
    return allDates.slice(Math.max(start, 0), Math.max(end, start + 2));
  }, [allDates, range]);

  if (allDates.length < 2) return <div className="text-[11px] text-[var(--text-muted)]">数据不足</div>;

  const models = [...byModel.keys()];
  const W = 600, H = height ?? 180, PAD_L = 50, PAD_R = 10, PAD_T = 10, PAD_B = 30;
  const plotW = W - PAD_L - PAD_R;
  const plotH = H - PAD_T - PAD_B;

  // Y 轴范围（基于可见数据）
  let maxY = 0;
  for (const arr of byModel.values()) {
    for (const d of arr) {
      if (visibleDates.includes(d.date) && d.tokens > maxY) maxY = d.tokens;
    }
  }
  if (maxY === 0) maxY = 1;

  const xStep = plotW / Math.max(visibleDates.length - 1, 1);
  const xPos = (i: number) => PAD_L + i * xStep;
  const yPos = (v: number) => PAD_T + plotH - (v / maxY) * plotH;

  // X 轴标签（每隔 N 天显示一个）
  const labelEvery = Math.max(1, Math.floor(visibleDates.length / 8));

  return (
    <div className="space-y-2">
      <svg viewBox={`0 0 ${W} ${H}`} className="w-full h-auto">
        {/* 网格线 */}
        {[0, 0.25, 0.5, 0.75, 1].map((r) => {
          const y = yPos(r * maxY);
          return <g key={r}>
            <line x1={PAD_L} y1={y} x2={W - PAD_R} y2={y} stroke="var(--border)" strokeWidth={0.3} strokeDasharray="2,2" />
            <text x={PAD_L - 4} y={y + 3} textAnchor="end" fill="var(--text-muted)" fontSize={8}>{fmtTok(r * maxY)}</text>
          </g>;
        })}
        {/* X 轴标签 */}
        {visibleDates.map((d, i) => i % labelEvery === 0 ? (
          <text key={d} x={xPos(i)} y={H - PAD_B + 14} textAnchor="middle" fill="var(--text-muted)" fontSize={7}>{d.slice(5)}</text>
        ) : null)}
        {/* 折线 */}
        {models.map((model, mi) => {
          const arr = byModel.get(model)!;
          const pts = visibleDates.map((d, i) => {
            const found = arr.find((a) => a.date === d);
            return { x: xPos(i), y: yPos(found?.tokens ?? 0) };
          });
          return (
            <g key={model}>
              <path d={smoothPath(pts)} fill="none" stroke={MODEL_COLORS[mi % MODEL_COLORS.length]} strokeWidth={1.5} strokeLinejoin="round" />
              {visibleDates.map((d, i) => {
                const found = arr.find((a) => a.date === d);
                if (!found || found.tokens === 0) return null;
                return <circle key={d} cx={xPos(i)} cy={yPos(found.tokens)} r={2.5} fill={MODEL_COLORS[mi % MODEL_COLORS.length]} />;
              })}
            </g>
          );
        })}
      </svg>

      {/* 拖拽选区条 */}
      <BrushRange
        total={allDates.length}
        range={range}
        onChange={setRange}
        labels={allDates}
      />

      {/* 图例 */}
      <div className="flex flex-wrap gap-3">
        {models.map((model, mi) => (
          <div key={model} className="flex items-center gap-1">
            <div className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: MODEL_COLORS[mi % MODEL_COLORS.length] }} />
            <span className="text-[10px] text-[var(--text-primary)] font-mono">{shortModel(model)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── 模型排名表 ───────────────────────────────────────────

function ModelRankingTable({ models }: { models: AnalyticsSummary["top_models"] }) {
  const maxTotal = models[0] ? models[0].input_tokens + models[0].output_tokens : 1;
  return (
    <div className="space-y-1">
      {/* 表头 */}
      <div className="flex items-center gap-2 h-6 text-[9px] text-[var(--text-muted)] uppercase px-1">
        <span className="w-6">#</span>
        <span className="w-28">模型</span>
        <span className="w-16 text-right">输入</span>
        <span className="w-16 text-right">输出</span>
        <span className="w-16 text-right">缓存</span>
        <span className="w-14 text-right">命中率</span>
        <span className="flex-1">占比</span>
      </div>
      {models.map((m, i) => {
        const total = m.input_tokens + m.output_tokens;
        return (
          <div key={m.name} className="flex items-center gap-2 h-7 px-1 rounded hover:bg-[var(--bg-hover)]">
            <span className="w-6 text-[10px] text-[var(--text-muted)]">{i + 1}</span>
            <span className="w-28 text-[11px] font-mono text-[var(--text-primary)] truncate">{shortModel(m.name)}</span>
            <span className="w-16 text-[10px] font-mono text-[#58a6ff] text-right">{fmtTok(m.input_tokens)}</span>
            <span className="w-16 text-[10px] font-mono text-[#3fb950] text-right">{fmtTok(m.output_tokens)}</span>
            <span className="w-16 text-[10px] font-mono text-[#bc8cff] text-right">{fmtTok(m.cache_read)}</span>
            <span className="w-14 text-[10px] font-mono text-right" style={{ color: m.cache_hit_rate > 0.5 ? "#3fb950" : m.cache_hit_rate > 0.2 ? "#d29922" : "var(--text-muted)" }}>
              {(m.cache_hit_rate * 100).toFixed(0)}%
            </span>
            <div className="flex-1 h-2 bg-[var(--bg-input)] rounded-full overflow-hidden">
              <div className="h-full rounded-full bg-[#58a6ff]/40" style={{ width: `${(total / maxTotal) * 100}%` }} />
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ════════════════════════════════════════════════════════════
// 工具函数
// ════════════════════════════════════════════════════════════

function fmtTok(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}

function shortModel(model: string): string {
  const parts = model.split("/");
  const name = parts[parts.length - 1];
  return name.replace(/-\d{8}$/, "").slice(-24);
}

function decodeProject(encoded: string): string {
  const fullPath = encoded.replace(/--/g, ":\\").replace(/-/g, "\\");
  const parts = fullPath.replace(/[:/\\]+$/, "").split(/[\\/]/);
  return parts[parts.length - 1] || fullPath;
}

// ── 拖拽选区条 ───────────────────────────────────────────

function BrushRange({
  range,
  onChange,
  labels,
}: {
  total?: number;
  range: [number, number];
  onChange: (r: [number, number]) => void;
  labels: string[];
}) {
  const trackRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ type: "left" | "right" | "body"; startX: number; startRange: [number, number] } | null>(null);

  const toPct = (v: number) => `${v * 100}%`;

  const handleMouseDown = (type: "left" | "right" | "body") => (e: React.MouseEvent) => {
    e.preventDefault();
    dragRef.current = { type, startX: e.clientX, startRange: [...range] as [number, number] };
    const handleMove = (ev: MouseEvent) => {
      if (!dragRef.current || !trackRef.current) return;
      const dx = ev.clientX - dragRef.current.startX;
      const dpct = dx / trackRef.current.offsetWidth;
      const [s0, s1] = dragRef.current.startRange;
      if (dragRef.current.type === "left") {
        onChange([Math.max(0, Math.min(s0 + dpct, s1 - 0.02)), s1]);
      } else if (dragRef.current.type === "right") {
        onChange([s0, Math.min(1, Math.max(s1 + dpct, s0 + 0.02))]);
      } else {
        const span = s1 - s0;
        let ns = s0 + dpct;
        if (ns < 0) ns = 0;
        if (ns + span > 1) ns = 1 - span;
        onChange([ns, ns + span]);
      }
    };
    const handleUp = () => {
      dragRef.current = null;
      document.removeEventListener("mousemove", handleMove);
      document.removeEventListener("mouseup", handleUp);
    };
    document.addEventListener("mousemove", handleMove);
    document.addEventListener("mouseup", handleUp);
  };

  const startDate = labels[Math.floor(range[0] * (labels.length - 1))] ?? "";
  const endDate = labels[Math.ceil(range[1] * (labels.length - 1))] ?? "";

  return (
    <div className="space-y-1 px-1">
      <div className="flex items-center justify-between text-[9px] text-[var(--text-muted)]">
        <span>{startDate.slice(5)}</span>
        <span>{endDate.slice(5)}</span>
      </div>
      <div ref={trackRef} className="relative h-6 bg-[var(--bg-input)] rounded cursor-crosshair select-none">
        {/* 暗区（未选中） */}
        <div className="absolute inset-y-0 left-0 bg-black/20 rounded-l" style={{ width: toPct(range[0]) }} />
        <div className="absolute inset-y-0 right-0 bg-black/20 rounded-r" style={{ width: toPct(1 - range[1]) }} />
        {/* 选区 */}
        <div
          className="absolute inset-y-0 bg-[var(--accent)]/15 border-y border-[var(--accent)]/40 cursor-grab active:cursor-grabbing"
          style={{ left: toPct(range[0]), width: toPct(range[1] - range[0]) }}
          onMouseDown={handleMouseDown("body")}
        />
        {/* 左手柄 */}
        <div
          className="absolute top-0 bottom-0 w-1.5 bg-[var(--accent)]/60 cursor-ew-resize hover:bg-[var(--accent)] rounded-l"
          style={{ left: `calc(${toPct(range[0])} - 3px)` }}
          onMouseDown={handleMouseDown("left")}
        />
        {/* 右手柄 */}
        <div
          className="absolute top-0 bottom-0 w-1.5 bg-[var(--accent)]/60 cursor-ew-resize hover:bg-[var(--accent)] rounded-r"
          style={{ left: `calc(${toPct(range[1])} - 3px)` }}
          onMouseDown={handleMouseDown("right")}
        />
      </div>
    </div>
  );
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

  // 单调性约束
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