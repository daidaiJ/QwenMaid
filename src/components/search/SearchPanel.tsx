import { useState, useEffect, useCallback, useRef } from "react";
import {
  Search,
  Globe,
  BookOpen,
  FileText,
  RefreshCw,
  CheckCircle,
  XCircle,
  Activity,
  Zap,
} from "lucide-react";
import {
  Toggle,
  Select,
  SecretInput,
  Field,
  Section,
} from "@/components/config/FormControls";
import {
  getMcpConfig,
  saveMcpConfig,
  restartMcpServer,
  getMcpStatus,
  getMcpStats,
  injectStatusline,
  removeStatusline,
  readSettings,
  checkUsageAutostart,
  setUsageAutostart,
} from "@/lib/tauri";
import type { McpConfig, McpStats } from "@/lib/tauri";

const searchModeOptions = [
  { value: "engine", label: "自动（Bing + Baidu 回退）" },
  { value: "bing", label: "Bing" },
  { value: "baidu", label: "Baidu" },
  { value: "tavily", label: "Tavily（需要 API Key）" },
];

const DEFAULT_CONFIG: McpConfig = {
  port: 8339,
  auto_inject: false,
  smartsearch_enabled: true,
  academicsearch_enabled: false,
  cleanfetch_enabled: true,
  search_mode: "engine",
  tavily_api_key: null,
  baidu_api_key: null,
  jina_api_key: null,
  proxy_url: null,
};

export function SearchPanel() {
  // 立即用默认值渲染，不等待 IPC
  const [config, setConfig] = useState<McpConfig>(DEFAULT_CONFIG);
  const [stats, setStats] = useState<McpStats | null>(null);
  const [portInput, setPortInput] = useState("8339");
  const [saving, setSaving] = useState(false);
  const [restarting, setRestarting] = useState(false);
  const [statuslineEnabled, setStatuslineEnabled] = useState(false);
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [mcpRunning, setMcpRunning] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 2500);
  }, []);

  // 并行加载所有数据，不阻塞渲染
  useEffect(() => {
    mountedRef.current = true;

    // 最关键：配置（表单状态依赖它）
    getMcpConfig().then((cfg) => {
      if (!mountedRef.current) return;
      setConfig(cfg);
      setPortInput(String(cfg.port));
    }).catch(() => {});

    // 次要：统计数据（纯展示）
    getMcpStats().then((s) => {
      if (!mountedRef.current) return;
      setStats(s);
    }).catch(() => {});

    // 次要：状态行状态（读 settings.json 的 ui.statusLine）
    readSettings().then((settings) => {
      if (!mountedRef.current) return;
      const ui = settings?.ui as Record<string, unknown> | undefined;
      const sl = ui?.statusLine as Record<string, unknown> | undefined;
      setStatuslineEnabled(!!sl?.command);
    }).catch(() => {});

    // 运行时 MCP 状态检测
    getMcpStatus().then((running) => {
      if (!mountedRef.current) return;
      setMcpRunning(running);
    }).catch(() => {});

    // 开机自启动状态检测
    checkUsageAutostart().then((enabled) => {
      if (!mountedRef.current) return;
      setAutostartEnabled(enabled);
    }).catch(() => {});

    // 每 10 秒轮询 MCP 运行状态
    const statusTimer = setInterval(() => {
      getMcpStatus().then((running) => {
        if (mountedRef.current) setMcpRunning(running);
      }).catch(() => {});
    }, 10000);

    return () => { mountedRef.current = false; clearInterval(statusTimer); };
  }, []);

  const updateConfig = async (patch: Partial<McpConfig>) => {
    const next = { ...config, ...patch };
    setConfig(next);
    setSaving(true);
    try {
      await saveMcpConfig(next);
      showToast("配置已保存");
    } catch (e) {
      showToast(`保存失败: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const handlePortApply = async () => {
    const newPort = parseInt(portInput, 10);
    if (isNaN(newPort) || newPort < 1 || newPort > 65535) {
      showToast("端口范围 1-65535");
      return;
    }
    setSaving(true);
    try {
      await saveMcpConfig({ ...config, port: newPort });
      setConfig({ ...config, port: newPort });
      await restartMcpServer();
      showToast(`端口已改为 ${newPort}，服务已重启`);
    } catch (e) {
      showToast(`端口更新失败: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const handleRestart = async () => {
    setRestarting(true);
    try {
      await restartMcpServer();
      // 等待服务器启动后检测状态
      await new Promise((r) => setTimeout(r, 500));
      const running = await getMcpStatus();
      setMcpRunning(running);
      showToast(running ? "MCP 服务已重启" : "MCP 服务重启失败");
    } catch (e) {
      setMcpRunning(false);
      showToast(`重启失败: ${e}`);
    } finally {
      setRestarting(false);
    }
  };

  const handleStatuslineToggle = async (enabled: boolean) => {
    try {
      if (enabled) {
        await injectStatusline();
        setStatuslineEnabled(true);
        showToast("状态行已注入");
      } else {
        await removeStatusline();
        setStatuslineEnabled(false);
        showToast("状态行已移除");
      }
    } catch (e) {
      showToast(`操作失败: ${e}`);
    }
  };

  const handleAutostartToggle = async (enabled: boolean) => {
    try {
      await setUsageAutostart(enabled);
      setAutostartEnabled(enabled);
      showToast(enabled ? "已设置开机自启动" : "已取消开机自启动");
    } catch (e) {
      showToast(`操作失败: ${e}`);
    }
  };

  const successRate =
    stats && stats.monthly_total > 0
      ? ((stats.monthly_success / stats.monthly_total) * 100).toFixed(1)
      : "—";

  return (
    <div className="h-full overflow-auto p-6 max-w-3xl mx-auto space-y-1">
      {/* Toast */}
      {toast && (
        <div className="fixed top-4 right-4 z-50 bg-[var(--accent)] text-white text-xs px-3 py-2 rounded-md shadow-lg animate-fade-in">
          {toast}
        </div>
      )}

      <div className="flex items-center gap-2 mb-4">
        <Search size={20} className="text-[var(--accent)]" />
        <h2 className="text-base font-semibold text-[var(--text-primary)]">
          MCP 搜索服务
        </h2>
      </div>

      {/* ── 服务器控制 ──────────────────────────────────── */}
      <Section title="服务器" description="MCP HTTP 服务状态与端口配置" />

      <div className="flex items-center gap-3 py-2 px-1">
        <div
          className={`flex items-center gap-1.5 text-xs px-2.5 py-1 rounded-full ${
            mcpRunning
              ? "bg-green-50 text-green-700 border border-green-200"
              : "bg-red-50 text-red-600 border border-red-200"
          }`}
        >
          {mcpRunning ? (
            <CheckCircle size={12} />
          ) : (
            <XCircle size={12} />
          )}
          {mcpRunning ? "运行中" : "已停止"}
        </div>
        <span className="text-xs text-[var(--text-muted)]">
          端口 {config.port}
        </span>
        <button
          onClick={handleRestart}
          disabled={restarting}
          className="ml-auto flex items-center gap-1 text-xs text-[var(--text-muted)] hover:text-[var(--accent)] transition-colors disabled:opacity-50"
        >
          <RefreshCw
            size={12}
            className={restarting ? "animate-spin" : ""}
          />
          重启服务
        </button>
      </div>

      <Field label="MCP 端口" description="修改后点击「应用」将热重启服务">
        <div className="flex items-center gap-2">
          <input
            type="number"
            value={portInput}
            onChange={(e) => setPortInput(e.target.value)}
            min={1}
            max={65535}
            className="h-9 w-28 bg-[var(--bg-input)] border border-[var(--border)] rounded-md px-3 text-[13px] font-mono text-[var(--text-primary)] focus:border-[var(--accent)] outline-none transition-all shadow-sm"
          />
          <button
            onClick={handlePortApply}
            disabled={saving || portInput === String(config.port)}
            className="h-9 px-3 bg-[var(--accent)] text-white text-xs rounded-md hover:opacity-90 disabled:opacity-40 transition-all"
          >
            应用
          </button>
        </div>
      </Field>

      <Field
        label="自动注入 Qwen Code"
        description="自动将 MCP 服务地址写入 ~/.qwen/settings.json 的 mcpServers 段"
        inline
      >
        <Toggle
          value={config.auto_inject}
          onChange={(v) => updateConfig({ auto_inject: v })}
        />
      </Field>

      {/* ── 工具配置 ──────────────────────────────────── */}
      <Section title="工具配置" description="启用/禁用各 MCP 工具及细粒度参数" />

      {/* smartsearch */}
      <div className="border border-[var(--border)] rounded-lg p-3 space-y-2 bg-[var(--bg-card)] shadow-[var(--shadow-card)]">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Globe size={15} className="text-blue-500" />
            <span className="text-[13px] font-medium text-[var(--text-primary)]">
              smartsearch
            </span>
            <span className="text-[10px] text-[var(--text-muted)] bg-[var(--bg-hover)] px-1.5 py-0.5 rounded">
              网络搜索
            </span>
          </div>
          <Toggle
            value={config.smartsearch_enabled}
            onChange={(v) => updateConfig({ smartsearch_enabled: v })}
          />
        </div>
        {config.smartsearch_enabled && (
          <Field label="搜索引擎" description="engine 模式自动选择最佳引擎">
            <Select
              value={config.search_mode}
              onChange={(v) => updateConfig({ search_mode: v })}
              options={searchModeOptions}
            />
          </Field>
        )}
      </div>

      {/* academicsearch */}
      <div className="border border-[var(--border)] rounded-lg p-3 space-y-2 bg-[var(--bg-card)] shadow-[var(--shadow-card)]">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <BookOpen size={15} className="text-purple-500" />
            <span className="text-[13px] font-medium text-[var(--text-primary)]">
              academicsearch
            </span>
            <span className="text-[10px] text-[var(--text-muted)] bg-[var(--bg-hover)] px-1.5 py-0.5 rounded">
              学术搜索
            </span>
          </div>
          <Toggle
            value={config.academicsearch_enabled}
            onChange={(v) => updateConfig({ academicsearch_enabled: v })}
          />
        </div>
        {config.academicsearch_enabled && (
          <p className="text-[11px] text-[var(--text-muted)]">
            引擎: arXiv, Crossref, OpenAlex（均无需 API Key）
          </p>
        )}
      </div>

      {/* cleanfetch */}
      <div className="border border-[var(--border)] rounded-lg p-3 space-y-2 bg-[var(--bg-card)] shadow-[var(--shadow-card)]">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <FileText size={15} className="text-emerald-500" />
            <span className="text-[13px] font-medium text-[var(--text-primary)]">
              cleanfetch
            </span>
            <span className="text-[10px] text-[var(--text-muted)] bg-[var(--bg-hover)] px-1.5 py-0.5 rounded">
              网页抓取
            </span>
          </div>
          <Toggle
            value={config.cleanfetch_enabled}
            onChange={(v) => updateConfig({ cleanfetch_enabled: v })}
          />
        </div>
        {config.cleanfetch_enabled && (
          <p className="text-[11px] text-[var(--text-muted)]">
            直接抓取 → 失败回退 Jina Reader（需 API Key）
          </p>
        )}
      </div>

      {/* ── API Keys ─────────────────────────────────────── */}
      <Section
        title="API Keys"
        description="可选的第三方服务密钥，用于增强搜索能力"
      />

      {(config.search_mode === "tavily" || config.search_mode === "engine") && (
        <Field
          label="Tavily API Key"
          description="用于 Tavily 搜索引擎（search_mode=tavily 时必需）"
        >
          <SecretInput
            value={config.tavily_api_key || ""}
            onChange={(v) => updateConfig({ tavily_api_key: v || null })}
            placeholder="tvly-..."
          />
        </Field>
      )}

      {(config.search_mode === "baidu" || config.search_mode === "engine") && (
        <Field
          label="百度 API Key"
          description="用于百度搜索 API（千帆），留空则走抓取模式"
        >
          <SecretInput
            value={config.baidu_api_key || ""}
            onChange={(v) => updateConfig({ baidu_api_key: v || null })}
            placeholder="百度千帆 API Key"
          />
        </Field>
      )}

      {config.cleanfetch_enabled && (
        <Field
          label="Jina API Key"
          description="用于 cleanfetch 的 Jina Reader 回退"
        >
          <SecretInput
            value={config.jina_api_key || ""}
            onChange={(v) => updateConfig({ jina_api_key: v || null })}
            placeholder="jina_..."
          />
        </Field>
      )}

      {/* ── 状态行成本追踪 ──────────────────────────────── */}
      <Section
        title="状态行"
        description="注入成本追踪脚本到 Qwen Code 状态行"
      />

      <Field
        label="状态行成本追踪"
        description="启用后将 qwen-usage CLI 注入为 Qwen Code 状态行命令"
        inline
      >
        <Toggle
          value={statuslineEnabled}
          onChange={handleStatuslineToggle}
        />
      </Field>

      <Field
        label="开机自启动"
        description="开机时自动启动 qwen-usage 后台服务（用于状态行数据采集）"
        inline
      >
        <Toggle
          value={autostartEnabled}
          onChange={handleAutostartToggle}
        />
      </Field>

      {/* ── 本月调用统计 ──────────────────────────────── */}
      <Section title="本月调用统计" description="当前月份的 API 调用次数和成功率" />

      <div className="grid grid-cols-3 gap-3 py-2">
        <StatsCard
          label="总调用"
          value={stats ? String(stats.monthly_total) : "—"}
          sub={stats && stats.monthly_total > 0 ? `${successRate}% 成功` : "—"}
          icon={<Activity size={14} />}
          color="text-blue-500"
        />
        <StatsCard
          label="成功"
          value={stats ? String(stats.monthly_success) : "—"}
          sub="次"
          icon={<CheckCircle size={14} />}
          color="text-green-500"
        />
        <StatsCard
          label="失败"
          value={stats ? String(stats.monthly_total - stats.monthly_success) : "—"}
          sub="次"
          icon={<XCircle size={14} />}
          color="text-red-500"
        />
      </div>

      {/* 按工具 */}
      {stats && stats.by_tool.length > 0 && (
        <div className="space-y-1 py-2">
          <h4 className="text-xs font-medium text-[var(--text-secondary)]">
            按工具
          </h4>
          <div className="grid grid-cols-1 gap-1.5">
            {stats.by_tool.map((t) => (
              <div
                key={t.tool_name}
                className="flex items-center justify-between text-xs px-2 py-1.5 bg-[var(--bg-card)] rounded border border-[var(--border)]"
              >
                <span className="font-mono text-[var(--text-primary)]">
                  {t.tool_name}
                </span>
                <span className="text-[var(--text-muted)]">
                  {t.total} 次 ·{" "}
                  {t.total > 0
                    ? ((t.success / t.total) * 100).toFixed(0)
                    : "—"}
                  % 成功
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 按引擎 */}
      {stats && stats.by_api.length > 0 && (
        <div className="space-y-1 py-2">
          <h4 className="text-xs font-medium text-[var(--text-secondary)]">
            按引擎
          </h4>
          <div className="flex flex-wrap gap-1.5">
            {stats.by_api.map((a) => (
              <span
                key={a.api_name}
                className="inline-flex items-center gap-1 text-[11px] px-2 py-1 bg-[var(--bg-card)] rounded border border-[var(--border)]"
              >
                <Zap size={10} className="text-[var(--accent)]" />
                <span className="font-mono">{a.api_name}</span>
                <span className="text-[var(--text-muted)]">{a.total}</span>
              </span>
            ))}
          </div>
        </div>
      )}

      {/* 空状态 */}
      {stats && stats.monthly_total === 0 && (
        <div className="text-center py-6 text-[var(--text-muted)] text-xs">
          本月暂无调用记录
        </div>
      )}
    </div>
  );
}

function StatsCard({
  label,
  value,
  sub,
  icon,
  color,
}: {
  label: string;
  value: string;
  sub: string;
  icon: React.ReactNode;
  color: string;
}) {
  return (
    <div className="border border-[var(--border)] rounded-lg p-3 bg-[var(--bg-card)] shadow-[var(--shadow-card)]">
      <div className="flex items-center gap-1.5 mb-1">
        <span className={color}>{icon}</span>
        <span className="text-[11px] text-[var(--text-muted)]">{label}</span>
      </div>
      <div className="text-lg font-semibold text-[var(--text-primary)]">
        {value}
      </div>
      <div className="text-[10px] text-[var(--text-muted)]">{sub}</div>
    </div>
  );
}
