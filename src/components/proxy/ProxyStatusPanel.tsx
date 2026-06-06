import { useState, useEffect, useCallback } from "react";
import {
  getProxyStatus,
  getProxyProviderStats,
  resetProviderCounts,
  listProviders,
} from "@/lib/tauri";
import type { ProxyStatus, ProxyProviderStats, Provider, ProviderModelStats } from "@/lib/tauri";
import {
  Activity,
  CheckCircle2,
  XCircle,
  RefreshCw,
  RotateCcw,
  Server,
  Cpu,
  ArrowUpRight,
  ArrowDownRight,
  Zap,
  Archive,
} from "lucide-react";

export function ProxyStatusPanel() {
  const [status, setStatus] = useState<ProxyStatus | null>(null);
  const [stats, setStats] = useState<ProxyProviderStats | null>(null);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [days, setDays] = useState(7);
  const [loading, setLoading] = useState(true);
  const [resetting, setResetting] = useState<number | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [s, st, ps] = await Promise.all([
        getProxyStatus(),
        getProxyProviderStats(days),
        listProviders(),
      ]);
      setStatus(s);
      setStats(st);
      setProviders(ps);
    } finally {
      setLoading(false);
    }
  }, [days]);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, 10000);
    return () => clearInterval(timer);
  }, [refresh]);

  const handleReset = async (providerId: number, providerName: string) => {
    if (!confirm(`重置 "${providerName}" 的所有调用计数？此操作不可恢复。`)) return;
    setResetting(providerId);
    try {
      await resetProviderCounts(providerId);
      await refresh();
    } finally {
      setResetting(null);
    }
  };

  // 按供应商分组
  const grouped = new Map<
    number,
    { provider: ProviderModelStats; models: ProviderModelStats[] }
  >();
  if (stats) {
    for (const p of stats.providers) {
      const existing = grouped.get(p.provider_id);
      if (existing) {
        existing.models.push(p);
        existing.provider.call_count += p.call_count;
        existing.provider.success_count += p.success_count;
        existing.provider.failure_count += p.failure_count;
        existing.provider.total_input_tokens += p.total_input_tokens;
        existing.provider.total_output_tokens += p.total_output_tokens;
        existing.provider.total_tokens_saved += p.total_tokens_saved;
        existing.provider.compressed_count += p.compressed_count;
      } else {
        grouped.set(p.provider_id, {
          provider: { ...p },
          models: [p],
        });
      }
    }
  }

  if (loading && !status) {
    return (
      <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">
        加载中…
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-5 space-y-4">
      {/* 标题栏 */}
      <div className="flex items-center justify-between">
        <h1 className="text-sm font-semibold text-[var(--text-primary)] flex items-center gap-2">
          <Activity size={15} className="text-[var(--accent)]" />
          代理服务状态
        </h1>
        <div className="flex items-center gap-2">
          <select
            value={days}
            onChange={(e) => setDays(Number(e.target.value))}
            className="h-7 bg-[var(--bg-input)] border border-[var(--border)] rounded-sm px-2 text-[12px] text-[var(--text-primary)] outline-none"
          >
            <option value={1}>1 天</option>
            <option value={7}>7 天</option>
            <option value={30}>30 天</option>
          </select>
          <button
            onClick={refresh}
            disabled={loading}
            className="w-7 h-7 flex items-center justify-center rounded hover:bg-[var(--bg-input)] text-[var(--text-muted)] hover:text-[var(--text-primary)] disabled:opacity-40"
          >
            <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
          </button>
        </div>
      </div>

      {/* 服务状态卡片 */}
      <div
        className={`flex items-center gap-4 p-4 rounded-xl border transition-all ${
          status?.running
            ? "bg-[#3fb950]/8 border-[#3fb950]/25"
            : "bg-[#f85149]/8 border-[#f85149]/25"
        }`}
      >
        <div
          className={`w-10 h-10 rounded-full flex items-center justify-center ${
            status?.running ? "bg-[#3fb950]/15" : "bg-[#f85149]/15"
          }`}
        >
          {status?.running ? (
            <CheckCircle2 size={20} className="text-[#3fb950]" />
          ) : (
            <XCircle size={20} className="text-[#f85149]" />
          )}
        </div>
        <div className="flex-1 min-w-0">
          <div className="text-[13px] font-medium text-[var(--text-primary)]">
            {status?.running ? "代理服务运行中" : "代理服务未启动"}
          </div>
          <div className="text-[11px] text-[var(--text-muted)] mt-0.5">
            端口 {status?.port ?? 18900} · localhost
          </div>
        </div>
        {status?.running && (
          <div className="text-right">
            <div className="text-[11px] text-[var(--text-muted)]">转发路径</div>
            <div className="text-[11px] text-[var(--text-secondary)] font-mono mt-0.5">
              Qwen Code → localhost:{status.port} → 上游
            </div>
          </div>
        )}
      </div>

      {/* 汇总统计 */}
      {stats && (
        <div className="grid grid-cols-5 gap-2">
          <StatCard
            label="总调用"
            value={stats.total_calls}
            icon={<Zap size={14} />}
            color="#58a6ff"
          />
          <StatCard
            label="成功"
            value={stats.total_calls - stats.total_failures}
            icon={<ArrowUpRight size={14} />}
            color="#3fb950"
          />
          <StatCard
            label="失败"
            value={stats.total_failures}
            icon={<ArrowDownRight size={14} />}
            color="#f85149"
          />
          <StatCard
            label="成功率"
            value={
              stats.total_calls > 0
                ? `${(((stats.total_calls - stats.total_failures) / stats.total_calls) * 100).toFixed(1)}%`
                : "—"
            }
            icon={<Activity size={14} />}
            color="#d29922"
            isText
          />
          <StatCard
            label="压缩节约"
            value={formatTokens(stats.total_tokens_saved)}
            icon={<Archive size={14} />}
            color="#a371f7"
            isText
          />
        </div>
      )}

      {/* 供应商列表 */}
      <div className="space-y-2">
        {Array.from(grouped.entries()).map(([pid, { provider, models }]) => {
          const dbProvider = providers.find((p) => p.id === pid);
          const useLocalProxy =
            dbProvider?.proxy_mode === "system" ||
            dbProvider?.proxy_mode === "custom";
          const successRate =
            provider.call_count > 0
              ? (
                  (provider.success_count / provider.call_count) *
                  100
                ).toFixed(1)
              : "—";

          return (
            <div
              key={pid}
              className="bg-[var(--bg-card)] rounded-xl shadow-[var(--shadow-card)] overflow-hidden"
            >
              {/* 供应商头 */}
              <div className="flex items-center gap-3 px-3 py-3 border-b border-[var(--border)] bg-[var(--bg-sidebar)]">
                <Server
                  size={13}
                  className="text-[var(--text-muted)] shrink-0"
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-[13px] font-medium text-[var(--text-primary)] truncate">
                      {provider.provider_name}
                    </span>
                    {useLocalProxy && (
                      <span className="text-[9px] px-1 py-0.5 rounded bg-[var(--accent)]/10 text-[var(--accent)]">
                        代理
                      </span>
                    )}
                  </div>
                  <div className="text-[10px] text-[var(--text-muted)] font-mono truncate">
                    {provider.base_url}
                  </div>
                </div>
                <div className="flex items-center gap-3 text-[11px] shrink-0">
                  <span className="text-[var(--text-muted)]">
                    <span className="text-[var(--text-primary)] font-medium">
                      {provider.call_count}
                    </span>{" "}
                    次调用
                  </span>
                  <span
                    className={
                      provider.failure_count > 0
                        ? "text-[#f85149]"
                        : "text-[var(--text-muted)]"
                    }
                  >
                    {provider.failure_count} 失败
                  </span>
                  <span className="text-[var(--text-muted)]">
                    {successRate}% 成功
                  </span>
                  {provider.total_tokens_saved > 0 && (
                    <span className="text-[#a371f7]" title="上下文压缩节约 tokens">
                      🗜️ {formatTokens(provider.total_tokens_saved)} 节约
                    </span>
                  )}
                  <button
                    onClick={() =>
                      handleReset(pid, provider.provider_name)
                    }
                    disabled={resetting === pid || provider.call_count === 0}
                    title="重置计数"
                    className="w-6 h-6 flex items-center justify-center rounded hover:bg-[var(--bg-input)] text-[var(--text-muted)] hover:text-[#f85149] disabled:opacity-30 transition-colors"
                  >
                    <RotateCcw
                      size={12}
                      className={resetting === pid ? "animate-spin" : ""}
                    />
                  </button>
                </div>
              </div>

              {/* 模型列表 */}
              <div className="divide-y divide-[var(--border)]/50">
                {models.map((m) => (
                  <div
                    key={m.model_id}
                    className="flex items-center gap-3 px-3 py-1.5 hover:bg-[var(--bg-hover)] transition-colors"
                  >
                    <Cpu
                      size={11}
                      className="text-[var(--text-muted)] shrink-0"
                    />
                    <span className="text-[12px] text-[var(--text-primary)] font-mono min-w-[140px] truncate">
                      {m.model_id}
                    </span>
                    <div className="flex-1" />
                    <div className="flex items-center gap-4 text-[11px] text-[var(--text-muted)] shrink-0">
                      <span title="调用次数">
                        <span className="text-[var(--text-secondary)]">
                          {m.call_count}
                        </span>{" "}
                        次
                      </span>
                      {m.failure_count > 0 && (
                        <span className="text-[#f85149]" title="失败次数">
                          {m.failure_count} 失败
                        </span>
                      )}
                      <span title="输入/输出 tokens">
                        {formatTokens(m.total_input_tokens)} /{" "}
                        {formatTokens(m.total_output_tokens)}
                      </span>
                      {m.total_tokens_saved > 0 && (
                        <span className="text-[#a371f7]" title={`压缩 ${m.compressed_count} 次，节约 ${m.total_tokens_saved.toLocaleString()} tokens`}>
                          🗜️ {formatTokens(m.total_tokens_saved)}
                        </span>
                      )}
                      {m.avg_duration_ms > 0 && (
                        <span title="平均延迟">
                          {m.avg_duration_ms.toFixed(0)}ms
                        </span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          );
        })}

        {grouped.size === 0 && (
          <div className="text-center py-8 text-[12px] text-[var(--text-muted)]">
            <Activity size={24} className="mx-auto mb-2 opacity-30" />
            <div className="text-[13px] font-medium">暂无代理调用记录</div>
            <div className="text-[11px] mt-1 opacity-70">
              在模型供应商配置中启用「本地路由代理」后，调用数据将在此显示
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function StatCard({
  label,
  value,
  icon,
  color,
  isText,
}: {
  label: string;
  value: number | string;
  icon: React.ReactNode;
  color: string;
  isText?: boolean;
}) {
  return (
    <div className="bg-[var(--bg-card)] rounded-xl shadow-[var(--shadow-card)] p-3">
      <div className="flex items-center gap-1.5 mb-1.5">
        <span style={{ color }} className="opacity-70">
          {icon}
        </span>
        <span className="text-[11px] text-[var(--text-muted)]">
          {label}
        </span>
      </div>
      <div
        className={`text-[var(--text-primary)] font-semibold ${
          isText ? "text-[15px]" : "text-[18px] tabular-nums"
        }`}
      >
        {typeof value === "number" ? value.toLocaleString() : value}
      </div>
    </div>
  );
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}
