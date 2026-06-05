import { useState, useEffect, useCallback } from "react";
import { ResizableColumns } from "@/components/layout/ResizableColumns";
import {
  listProviders,
  createProvider,
  deleteProvider,
  updateProvider,
  listModels,
  createModel,
  deleteModel,
  updateModel,
  syncConfigToSettings,
  discoverExistingProviders,
  syncPresetModelsToSettings,
} from "@/lib/tauri";
import type { Provider, Model, DiscoveredProvider } from "@/lib/tauri";
import { Plus, Trash2, Server, Cpu, RefreshCw, ChevronRight, Search } from "lucide-react";

// ── 预设供应商模板 ────────────────────────────────────────

interface ProviderTemplate {
  name: string;
  baseUrl: string;
  envPrefix: string;
  proxyMode: string;
  billingType: string;
  authHeader?: string;
  models: {
    id: string;
    name: string;
    authType: string[];
    /** 写入 DB config_json，同步时变为 generationConfig */
    generationConfig?: Record<string, unknown>;
  }[];
}

const TEMPLATES: ProviderTemplate[] = [
  {
    name: "OpenAI",
    baseUrl: "https://api.openai.com",
    envPrefix: "OPENAI_API_KEY",
    proxyMode: "system",
    billingType: "pay_per_use",
    models: [
      { id: "gpt-4o", name: "GPT-4o", authType: ["openai"] },
      { id: "gpt-4o-mini", name: "GPT-4o Mini", authType: ["openai"] },
      { id: "o3", name: "o3", authType: ["openai"] },
    ],
  },
  {
    name: "Anthropic",
    baseUrl: "https://api.anthropic.com",
    envPrefix: "ANTHROPIC_API_KEY",
    proxyMode: "system",
    billingType: "pay_per_use",
    authHeader: "x-api-key",
    models: [
      { id: "claude-sonnet-4-20250514", name: "Claude Sonnet 4", authType: ["anthropic", "openai"] },
      { id: "claude-haiku-35-20241022", name: "Claude 3.5 Haiku", authType: ["anthropic", "openai"] },
    ],
  },
  {
    name: "Google Gemini",
    baseUrl: "https://generativelanguage.googleapis.com",
    envPrefix: "GEMINI_API_KEY",
    proxyMode: "system",
    billingType: "pay_per_use",
    models: [
      { id: "gemini-2.5-pro", name: "Gemini 2.5 Pro", authType: ["gemini", "openai"] },
      { id: "gemini-2.5-flash", name: "Gemini 2.5 Flash", authType: ["gemini", "openai"] },
    ],
  },
  {
    name: "DeepSeek",
    baseUrl: "https://api.deepseek.com",
    envPrefix: "DEEPSEEK_API_KEY",
    proxyMode: "direct",
    billingType: "pay_per_use",
    models: [
      { id: "deepseek-chat", name: "DeepSeek V3", authType: ["openai"] },
      { id: "deepseek-reasoner", name: "DeepSeek R1", authType: ["openai"] },
    ],
  },
  {
    name: "Qwen (DashScope)",
    baseUrl: "https://dashscope.aliyuncs.com/compatible-mode",
    envPrefix: "DASHSCOPE_API_KEY",
    proxyMode: "direct",
    billingType: "plan",
    models: [
      { id: "qwen-max", name: "Qwen Max", authType: ["openai"] },
      { id: "qwen-plus", name: "Qwen Plus", authType: ["openai"] },
      { id: "qwen-turbo", name: "Qwen Turbo", authType: ["openai"] },
    ],
  },
  {
    name: "Moonshot (Kimi)",
    baseUrl: "https://api.moonshot.cn",
    envPrefix: "MOONSHOT_API_KEY",
    proxyMode: "direct",
    billingType: "pay_per_use",
    models: [
      { id: "moonshot-v1-auto", name: "Moonshot V1 Auto", authType: ["openai"] },
    ],
  },
  {
    name: "MiMo (按量 API)",
    baseUrl: "https://api.xiaomimimo.com/v1",
    envPrefix: "MIMO_API_KEY",
    proxyMode: "direct",
    billingType: "pay_per_use",
    models: [
      {
        id: "mimo-v2.5-pro",
        name: "MiMo V2.5 Pro (推理/长文档)",
        authType: ["openai"],
        generationConfig: {
          contextWindowSize: 1048576,
          extra_body: { enable_thinking: true },
        },
      },
      {
        id: "mimo-v2.5",
        name: "MiMo V2.5 (多模态)",
        authType: ["openai"],
        generationConfig: {
          contextWindowSize: 1048576,
          modalities: { image: true, audio: true, video: true },
        },
      },
      {
        id: "mimo-v2-omni",
        name: "MiMo V2 Omni (多模态)",
        authType: ["openai"],
        generationConfig: {
          contextWindowSize: 1048576,
          modalities: { image: true, audio: true, video: true },
        },
      },
      {
        id: "mimo-v2-flash",
        name: "MiMo V2 Flash (快速/低成本)",
        authType: ["openai"],
        generationConfig: {
          contextWindowSize: 131072,
        },
      },
    ],
  },
  {
    name: "MiMo (按量 Anthropic)",
    baseUrl: "https://api.xiaomimimo.com",
    envPrefix: "MIMO_API_KEY",
    proxyMode: "direct",
    billingType: "pay_per_use",
    authHeader: "api-key",
    models: [
      {
        id: "mimo-v2.5-pro",
        name: "MiMo V2.5 Pro (推理/长文档)",
        authType: ["anthropic"],
        generationConfig: {
          contextWindowSize: 1048576,
        },
      },
      {
        id: "mimo-v2.5",
        name: "MiMo V2.5 (多模态)",
        authType: ["anthropic"],
        generationConfig: {
          contextWindowSize: 1048576,
          modalities: { image: true, audio: true, video: true },
        },
      },
    ],
  },
  {
    name: "MiMo (Token Plan)",
    baseUrl: "https://token-plan-cn.xiaomimimo.com/v1",
    envPrefix: "MIMO_TP_KEY",
    proxyMode: "direct",
    billingType: "plan",
    models: [
      {
        id: "mimo-v2.5-pro",
        name: "MiMo V2.5 Pro (推理/长文档)",
        authType: ["openai"],
        generationConfig: {
          contextWindowSize: 1048576,
          extra_body: { enable_thinking: true },
        },
      },
      {
        id: "mimo-v2.5",
        name: "MiMo V2.5 (多模态)",
        authType: ["openai"],
        generationConfig: {
          contextWindowSize: 1048576,
          modalities: { image: true, audio: true, video: true },
        },
      },
      {
        id: "mimo-v2-flash",
        name: "MiMo V2 Flash (快速/低成本)",
        authType: ["openai"],
        generationConfig: {
          contextWindowSize: 131072,
        },
      },
    ],
  },
  {
    name: "MiMo (Token Plan Anthropic)",
    baseUrl: "https://token-plan-cn.xiaomimimo.com",
    envPrefix: "MIMO_TP_KEY",
    proxyMode: "direct",
    billingType: "plan",
    authHeader: "api-key",
    models: [
      {
        id: "mimo-v2.5-pro",
        name: "MiMo V2.5 Pro (推理/长文档)",
        authType: ["anthropic"],
        generationConfig: {
          contextWindowSize: 1048576,
        },
      },
    ],
  },
  {
    name: "OpenCode-Go (OpenAI)",
    baseUrl: "https://opencode.ai/zen/go/v1",
    envPrefix: "OPENCODE_API_KEY",
    proxyMode: "system",
    billingType: "pay_per_use",
    models: [
      { id: "glm-5.1", name: "GLM-5.1", authType: ["openai"] },
      { id: "glm-5", name: "GLM-5", authType: ["openai"] },
      { id: "kimi-k2.5", name: "Kimi K2.5", authType: ["openai"] },
      { id: "kimi-k2.6", name: "Kimi K2.6", authType: ["openai"] },
      { id: "deepseek-v4-pro", name: "DeepSeek V4 Pro", authType: ["openai"] },
      { id: "deepseek-v4-flash", name: "DeepSeek V4 Flash", authType: ["openai"] },
      { id: "mimo-v2.5", name: "MiMo V2.5", authType: ["openai"] },
      { id: "mimo-v2.5-pro", name: "MiMo V2.5 Pro", authType: ["openai"] },
    ],
  },
  {
    name: "OpenCode-Go (Anthropic)",
    baseUrl: "https://opencode.ai/zen/go",
    envPrefix: "OPENCODE_API_KEY",
    proxyMode: "system",
    billingType: "pay_per_use",
    models: [
      { id: "minimax-m3", name: "MiniMax M3", authType: ["anthropic"] },
      { id: "minimax-m2.7", name: "MiniMax M2.7", authType: ["anthropic"] },
      { id: "minimax-m2.5", name: "MiniMax M2.5", authType: ["anthropic"] },
      { id: "qwen3.7-max", name: "Qwen 3.7 Max", authType: ["anthropic"] },
      { id: "qwen3.6-plus", name: "Qwen 3.6 Plus", authType: ["anthropic"] },
    ],
  },
];

const BILLING_TYPES = [
  { value: "plan", label: "订阅制 (Plan)" },
  { value: "pay_per_use", label: "按量计费" },
] as const;

const AUTH_OPTIONS = ["openai", "anthropic", "gemini"] as const;

// ── 主面板 ────────────────────────────────────────────────

export function ProviderPanel() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [models, setModels] = useState<Model[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [syncResult, setSyncResult] = useState<string | null>(null);
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [discovered, setDiscovered] = useState<DiscoveredProvider[]>([]);
  const [discovering, setDiscovering] = useState(false);
  const [presetSyncing, setPresetSyncing] = useState(false);

  const runDiscovery = async () => {
    setDiscovering(true);
    try {
      const result = await discoverExistingProviders();
      setDiscovered(result);
    } catch {
      setDiscovered([]);
    } finally {
      setDiscovering(false);
    }
  };

  const refresh = useCallback(async () => {
    setLoading(true);
    const ps = await listProviders();
    setProviders(ps);
    if (ps.length === 0) {
      setSelected(null);
    } else if (selected === null || !ps.some(p => p.id === selected)) {
      setSelected(ps[0].id);
    }
    setLoading(false);
    // 自动运行发现以获取预设补齐信息
    runDiscovery();
  }, [selected]);

  useEffect(() => { refresh(); }, []);

  useEffect(() => {
    if (selected != null) {
      listModels(selected).then(setModels);
    } else {
      setModels([]);
    }
  }, [selected]);

  if (loading) return <div className="p-6 text-[var(--text-muted)]">加载中…</div>;

  return (
    <>
    <ResizableColumns
      autoSaveId="provider-panel-v2"
      left={{
        defaultSize: 22,
        minSize: 15,
        maxSize: 35,
        className: "bg-[var(--bg-sidebar)] flex flex-col",
        children: (
          <>
            <div className="flex items-center justify-between px-3 h-9 border-b border-[var(--border)]">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">供应商</span>
              <div className="flex items-center gap-0.5">
                <button
                  onClick={runDiscovery}
                  disabled={discovering}
                  title="发现已有配置"
                  className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--bg-input)] text-[var(--text-muted)] hover:text-[var(--text-primary)] disabled:opacity-40"
                >
                  <Search size={13} className={discovering ? "animate-pulse" : ""} />
                </button>
                <button
                  onClick={() => setShowAddDialog(true)}
                  className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--bg-input)] text-[var(--text-muted)] hover:text-[var(--text-primary)]"
                >
                  <Plus size={14} />
                </button>
              </div>
            </div>
            <div className="flex-1 overflow-auto py-0.5">
              {providers.map((p) => {
                // 计算该供应商可补齐的预设模型数
                const dp = discovered.find(d =>
                  d.preset_name === p.name || d.name === p.name
                );
                const presetCount = dp?.models.filter(m => m.from_preset).length ?? 0;
                return (
                <button
                  key={p.id}
                  onClick={() => setSelected(p.id)}
                  className={`w-full flex items-center gap-2 px-3 h-7 text-left text-[13px] transition-colors ${
                    selected === p.id
                      ? "bg-[var(--accent-light)] text-[var(--accent)] font-medium"
                      : "text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
                  }`}
                >
                  <Server size={13} className="shrink-0 text-[var(--text-muted)]" />
                  <span className="truncate">{p.name}</span>
                  {presetCount > 0 && (
                    <span className="ml-auto text-[10px] text-[#3fb950] bg-[#3fb9501a] px-1 rounded" title={`可补齐 ${presetCount} 个预设模型`}>
                      +{presetCount}
                    </span>
                  )}
                  {p.billing_type === "plan" && (
                    <span className={`${presetCount > 0 ? "" : "ml-auto"} text-[10px] text-[#3794ff] bg-[#3794ff1a] px-1 rounded`}>P</span>
                  )}
                  {!p.is_active && (
                    <span className="text-[10px] text-[var(--text-muted)]">停用</span>
                  )}
                </button>
                );
              })}
              {providers.length === 0 && (
                <p className="px-3 py-4 text-xs text-[var(--text-muted)]">
                  暂无供应商，点击 + 添加
                </p>
              )}
            </div>
            {/* 同步按钮 */}
            <div className="border-t border-[var(--border)] p-2 space-y-1">
              {(() => {
                const totalPreset = discovered.reduce(
                  (sum, d) => sum + d.models.filter(m => m.from_preset).length, 0
                );
                return totalPreset > 0 ? (
                  <button
                    onClick={async () => {
                      setPresetSyncing(true);
                      try {
                        const count = await syncPresetModelsToSettings();
                        setSyncResult(`已补齐 ${count} 个预设模型`);
                        refresh();
                        setTimeout(() => setSyncResult(null), 3000);
                      } catch (e) {
                        setSyncResult(`补齐失败: ${e}`);
                      } finally {
                        setPresetSyncing(false);
                      }
                    }}
                    disabled={presetSyncing}
                    className="w-full flex items-center justify-center gap-1.5 h-7 text-xs bg-[#3fb950] text-white rounded-sm hover:opacity-90 disabled:opacity-40 transition-colors"
                  >
                    <RefreshCw size={12} className={presetSyncing ? "animate-spin" : ""} />
                    {presetSyncing ? "补齐中…" : `补齐预设 (+${totalPreset})`}
                  </button>
                ) : null;
              })()}
              <button
                onClick={async () => {
                  setSyncing(true);
                  try {
                    await syncConfigToSettings();
                    setSyncResult("已同步到 settings.json");
                    setTimeout(() => setSyncResult(null), 3000);
                  } catch (e) {
                    setSyncResult(`失败: ${e}`);
                  } finally {
                    setSyncing(false);
                  }
                }}
                disabled={syncing || providers.length === 0}
                className="w-full flex items-center justify-center gap-1.5 h-7 text-xs bg-[var(--accent)] text-white rounded-sm hover:bg-[var(--accent-hover)] disabled:opacity-40 transition-colors"
              >
                <RefreshCw size={12} className={syncing ? "animate-spin" : ""} />
                {syncing ? "同步中…" : "同步到 Qwen Code"}
              </button>
              {syncResult && (
                <p className="text-[11px] text-[var(--text-muted)] px-1 truncate">{syncResult}</p>
              )}
            </div>
          </>
        ),
      }}
      center={{
        className: "flex flex-col overflow-auto",
        children: (() => {
          const sp = selected != null ? providers.find((p) => p.id === selected) : undefined;
          return sp ? (
            <ProviderDetail
              provider={sp}
              models={models}
              onRefresh={refresh}
              onModelsRefresh={() => listModels(selected!).then(setModels)}
            />
          ) : (
            <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">
              {providers.length > 0 ? "选择左侧供应商查看详情" : ""}
            </div>
          );
        })(),
      }}
    />

    {/* 添加供应商弹窗 */}
    {showAddDialog && (
      <AddProviderDialog
        discovered={discovered}
        onClose={() => setShowAddDialog(false)}
        onCreated={() => { setShowAddDialog(false); refresh(); }}
      />
    )}
    </>
  );
}

// ── 供应商详情 ───────────────────────────────────────────

function ProviderDetail({
  provider,
  models,
  onRefresh,
  onModelsRefresh,
}: {
  provider: Provider;
  models: Model[];
  onRefresh: () => void;
  onModelsRefresh: () => void;
}) {
  const [form, setForm] = useState({
    name: provider.name,
    baseUrl: provider.base_url,
    apiKeyEnv: provider.api_key_env,
    proxyMode: provider.proxy_mode,
    proxyUrl: provider.proxy_url ?? "",
    billingType: provider.billing_type ?? "pay_per_use",
    compressEnabled: provider.compress_enabled ?? false,
  });
  const [saving, setSaving] = useState(false);
  const [showAddModel, setShowAddModel] = useState(false);

  // 是否使用本地路由代理（system/custom = 代理，direct = 直连）
  const useLocalProxy = form.proxyMode === "system" || form.proxyMode === "custom";

  const toggleProxyMode = (enabled: boolean) => {
    setForm({
      ...form,
      proxyMode: enabled ? "system" : "direct",
      proxyUrl: enabled ? "" : form.proxyUrl,
      compressEnabled: enabled ? form.compressEnabled : false,
    });
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await updateProvider({
        id: provider.id,
        name: form.name,
        baseUrl: form.baseUrl,
        apiKeyEnv: form.apiKeyEnv,
        proxyMode: form.proxyMode,
        proxyUrl: form.proxyUrl || undefined,
        billingType: form.billingType,
        compressEnabled: form.compressEnabled,
      });
      onRefresh();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="p-6 space-y-6 max-w-2xl">
      <div className="flex items-center justify-between">
        <h2 className="text-base font-medium text-[var(--text-primary)]">{provider.name}</h2>
        <button
          onClick={async () => {
            if (confirm(`删除供应商 "${provider.name}"？此操作不可恢复。`)) {
              await deleteProvider(provider.id);
              onRefresh();
            }
          }}
          className="px-3 h-7 text-xs bg-[#fde8e8] text-[#d32f2f] rounded-md border border-[#f5c6cb] hover:bg-[#f8d7da] dark:bg-[#4d1a1a] dark:text-[#f48771] dark:border-transparent transition-colors"
        >
          删除
        </button>
      </div>

      {/* ── 本地代理开关（醒目卡片） ─────────────────── */}
      <div className={`flex items-center justify-between p-3 rounded-md border transition-all ${
        useLocalProxy
          ? "bg-[var(--accent-light)] border-[var(--accent)]/30 shadow-[0_0_0_1px_rgba(124,58,237,0.08),var(--shadow-sm)]"
          : "bg-[var(--bg-card)] border-[var(--border)] shadow-[var(--shadow-card)]"
      }`}>
        <div className="flex-1 min-w-0 mr-4">
          <div className="flex items-center gap-2">
            <span className={`w-2 h-2 rounded-full ${useLocalProxy ? "bg-[#3fb950]" : "bg-[#8b949e]"}`} />
            <span className="text-[13px] font-medium text-[var(--text-primary)]">
              本地路由代理
            </span>
          </div>
          <p className="text-[11px] text-[var(--text-muted)] mt-0.5 pl-4">
            {useLocalProxy
              ? `Qwen Code → QWenMaid (localhost:18900) → ${provider.base_url}`
              : `Qwen Code → ${provider.base_url}（直连）`}
          </p>
          {useLocalProxy && (
            <div className="flex items-center justify-between mt-2 pl-4">
              <span className="text-[11px] text-[var(--text-muted)]">
                🗜️ 上下文压缩（节省 40-90% tokens）
              </span>
              <button
                type="button"
                role="switch"
                aria-checked={form.compressEnabled}
                onClick={() => setForm({ ...form, compressEnabled: !form.compressEnabled })}
                className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border transition-colors ${
                  form.compressEnabled
                    ? "bg-[var(--accent)] border-[var(--accent)]"
                    : "bg-[var(--bg-input)] border-[var(--border)]"
                }`}
              >
                <span
                  className={`pointer-events-none block h-3.5 w-3.5 rounded-full bg-white shadow transition-transform ${
                    form.compressEnabled ? "translate-x-[18px]" : "translate-x-[2px]"
                  }`}
                />
              </button>
            </div>
          )}
        </div>
        {/* 自定义 Toggle 按钮 */}
        <button
          type="button"
          role="switch"
          aria-checked={useLocalProxy}
          onClick={() => toggleProxyMode(!useLocalProxy)}
          className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border transition-colors ${
            useLocalProxy
              ? "bg-[var(--accent)] border-[var(--accent)]"
              : "bg-[var(--bg-input)] border-[var(--border)]"
          }`}
        >
          <span
            className={`pointer-events-none block h-4 w-4 rounded-full bg-white shadow transition-transform ${
              useLocalProxy ? "translate-x-[22px]" : "translate-x-[3px]"
            }`}
          />
        </button>
      </div>

      {/* ── 供应商配置表单 ───────────────────────────── */}
      <div className="space-y-3">
        <div className="grid grid-cols-2 gap-3">
          <Field label="名称" value={form.name} onChange={(v) => setForm({ ...form, name: v })} />
          <Field label="Base URL" value={form.baseUrl} onChange={(v) => setForm({ ...form, baseUrl: v })} />
          <Field label="环境变量名" value={form.apiKeyEnv} onChange={(v) => setForm({ ...form, apiKeyEnv: v })} placeholder="OPENAI_API_KEY" />
          <div>
            <label className="block text-[11px] text-[var(--text-muted)] mb-1">计费类型</label>
            <select
              value={form.billingType}
              onChange={(e) => setForm({ ...form, billingType: e.target.value as any })}
              className="w-full h-8 bg-[var(--bg-input)] border border-[var(--border)] rounded-sm px-2 text-[13px] text-[var(--text-primary)] focus:border-[#007fd4] outline-none"
            >
              {BILLING_TYPES.map((b) => (
                <option key={b.value} value={b.value}>{b.label}</option>
              ))}
            </select>
          </div>
          {/* 高级代理设置已移除，默认使用系统代理 */}
        </div>
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-4 h-7 text-xs bg-[var(--accent)] text-white rounded-sm hover:bg-[var(--accent-hover)] disabled:opacity-40 transition-colors"
        >
          {saving ? "保存中…" : "保存"}
        </button>
      </div>

      {/* 模型列表 */}
      <div className="border-t border-[var(--border)] pt-4">
        <div className="flex items-center justify-between mb-2">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-[#bbbbbb] flex items-center gap-1.5">
            <Cpu size={12} /> 模型 ({models.length})
          </h3>
          <button
            onClick={() => setShowAddModel(true)}
            className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--bg-input)] text-[var(--text-muted)] hover:text-[var(--text-primary)]"
          >
            <Plus size={13} />
          </button>
        </div>
        <div className="space-y-0.5">
          {models.map((m) => (
            <ModelRow
              key={m.id}
              model={m}
              onDelete={async () => { await deleteModel(m.id); onModelsRefresh(); }}
              onConfigSaved={onModelsRefresh}
            />
          ))}
          {models.length === 0 && (
            <p className="text-xs text-[var(--text-muted)] px-3 py-2">暂无模型，点击 + 添加</p>
          )}
        </div>
      </div>

      {showAddModel && (
        <AddModelDialog
          providerId={provider.id}
          onClose={() => setShowAddModel(false)}
          onCreated={() => { setShowAddModel(false); onModelsRefresh(); }}
        />
      )}
    </div>
  );
}

// ── 模型行（可展开 generationConfig 编辑） ──────────────

interface GenerationConfig {
  contextWindowSize?: number;
  modalities?: {
    image?: boolean;
    text?: boolean;
    audio?: boolean;
    video?: boolean;
  };
  extra_body?: {
    thinking?: { type: "enabled" | "disabled" };
    reasoning_effort?: "high" | "max";
    enable_thinking?: boolean;
  };
}

function ModelRow({
  model,
  onDelete,
  onConfigSaved,
}: {
  model: Model;
  onDelete: () => void;
  onConfigSaved: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const authTypes: string[] = (() => {
    try { return JSON.parse(model.auth_type); } catch { return [model.auth_type]; }
  })();
  const config: GenerationConfig = (() => {
    try { return model.config_json ? JSON.parse(model.config_json) : {}; } catch { return {}; }
  })();

  return (
    <div className="bg-[var(--bg-card)] rounded-md shadow-[var(--shadow-card)] group hover:shadow-[var(--shadow-md)] transition-shadow">
      {/* 行头 */}
      <div className="flex items-center gap-2 px-3 h-8 text-[13px]">
        <button
          onClick={() => setExpanded(!expanded)}
          className="text-[var(--text-muted)] hover:text-[var(--text-primary)] shrink-0"
        >
          <ChevronRight size={12} className={`transition-transform ${expanded ? "rotate-90" : ""}`} />
        </button>
        <span className="flex-1 truncate font-mono text-xs text-[var(--text-primary)]">{model.model_id}</span>
        {config.extra_body?.thinking?.type === "enabled" && (
          <span className="text-[10px] text-[#89d185] bg-[#89d1851a] px-1 rounded-sm">思考</span>
        )}
        {authTypes.map((t) => (
          <span key={t} className="text-[10px] text-[var(--text-muted)] px-1.5 py-0.5 bg-[var(--bg-input)] rounded-sm">{t}</span>
        ))}
        {model.is_default && (
          <span className="text-[10px] text-[#89d185]">默认</span>
        )}
        <button
          onClick={onDelete}
          className="text-[var(--text-muted)] hover:text-[#f44747] opacity-0 group-hover:opacity-100 transition-opacity"
        >
          <Trash2 size={12} />
        </button>
      </div>

      {/* 展开：generationConfig 编辑 */}
      {expanded && (
        <ModelGenConfigEditor
          model={model}
          config={config}
          onSaved={onConfigSaved}
        />
      )}
    </div>
  );
}

// ── 模型 generationConfig 编辑器 ────────────────────────

function ModelGenConfigEditor({
  model,
  config,
  onSaved,
}: {
  model: Model;
  config: GenerationConfig;
  onSaved: () => void;
}) {
  const [thinkingType, setThinkingType] = useState<"enabled" | "disabled" | "off">(
    config.extra_body?.thinking?.type ?? (config.extra_body?.enable_thinking === true ? "enabled" : config.extra_body?.enable_thinking === false ? "disabled" : "off")
  );
  const [reasoningEffort, setReasoningEffort] = useState<string>(
    config.extra_body?.reasoning_effort ?? ""
  );
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    setSaving(true);
    try {
      const genConfig: GenerationConfig = {};

      // extra_body
      const eb: GenerationConfig["extra_body"] = {};
      if (thinkingType !== "off") {
        eb.thinking = { type: thinkingType };
      }
      if (reasoningEffort) {
        eb.reasoning_effort = reasoningEffort as "high" | "max";
      }
      if (Object.keys(eb).length > 0) {
        genConfig.extra_body = eb;
      }

      await updateModel({
        id: model.id,
        configJson: Object.keys(genConfig).length > 0 ? JSON.stringify(genConfig) : undefined,
      });
      onSaved();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="px-3 pb-3 pt-1 border-t border-[var(--border)] space-y-3">
      {/* 思考模式 */}
      <div className="flex items-center gap-3">
        <label className="text-[11px] text-[var(--text-muted)] w-24 shrink-0">思考模式</label>
        <select
          value={thinkingType}
          onChange={(e) => setThinkingType(e.target.value as "enabled" | "disabled" | "off")}
          className="h-7 bg-[var(--bg-input)] border border-[#3c3c3c] rounded-sm px-2 text-[12px] text-[var(--text-primary)] focus:border-[#007fd4] outline-none"
        >
          <option value="off">不配置</option>
          <option value="enabled">启用 (thinking.type=enabled)</option>
          <option value="disabled">禁用 (thinking.type=disabled)</option>
        </select>
      </div>

      {/* 推理深度 */}
      {thinkingType === "enabled" && (
        <div className="flex items-center gap-3">
          <label className="text-[11px] text-[var(--text-muted)] w-24 shrink-0">推理深度</label>
          <select
            value={reasoningEffort}
            onChange={(e) => setReasoningEffort(e.target.value)}
            className="h-7 bg-[var(--bg-input)] border border-[#3c3c3c] rounded-sm px-2 text-[12px] text-[var(--text-primary)] focus:border-[#007fd4] outline-none"
          >
            <option value="">不配置</option>
            <option value="high">high</option>
            <option value="max">max</option>
          </select>
          <span className="text-[10px] text-[#5a5a5a]">reasoning_effort（DeepSeek 等）</span>
        </div>
      )}

      {/* 保存 */}
      <div className="flex items-center gap-2 pt-1">
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-3 h-6 text-[11px] bg-[var(--accent)] text-white rounded-sm hover:bg-[var(--accent-hover)] disabled:opacity-40 transition-colors"
        >
          {saving ? "保存中…" : "保存配置"}
        </button>
        <span className="text-[10px] text-[#5a5a5a]">
          保存后需点击「同步到 Qwen Code」生效
        </span>
      </div>
    </div>
  );
}

// ── 添加供应商弹窗（模板选择） ──────────────────────────

function AddProviderDialog({
  discovered,
  onClose,
  onCreated,
}: {
  discovered: DiscoveredProvider[];
  onClose: () => void;
  onCreated: () => void;
}) {
  const [step, setStep] = useState<"template" | "form">("template");
  const [tab, setTab] = useState<"templates" | "discovered">(
    discovered.length > 0 ? "discovered" : "templates"
  );
  const [apiKey, setApiKey] = useState("");
  const [form, setForm] = useState({
    name: "",
    baseUrl: "",
    apiKeyEnv: "",
    proxyMode: "direct",
    proxyUrl: "",
    authHeader: "",
    billingType: "pay_per_use",
  });
  const [selectedTemplate, setSelectedTemplate] = useState<ProviderTemplate | null>(null);

  const selectTemplate = (t: ProviderTemplate) => {
    setSelectedTemplate(t);
    setForm({
      name: t.name,
      baseUrl: t.baseUrl,
      apiKeyEnv: t.envPrefix,
      proxyMode: t.proxyMode,
      proxyUrl: "",
      authHeader: t.authHeader ?? "",
      billingType: t.billingType,
    });
    setStep("form");
  };

  const submit = async () => {
    const provider = await createProvider({
      name: form.name,
      baseUrl: form.baseUrl,
      apiKeyEnv: form.apiKeyEnv,
      proxyMode: form.proxyMode,
      proxyUrl: form.proxyUrl || undefined,
      authHeader: form.authHeader || undefined,
      billingType: form.billingType,
    });

    // 如果填了 API Key，写入环境变量
    if (apiKey.trim()) {
      try {
        const settings = await readSettingsFromTauri();
        if (!settings.env) settings.env = {};
        (settings.env as Record<string, string>)[form.apiKeyEnv] = apiKey.trim();
        await writeSettingsToTauri(settings);
      } catch {
        // settings.json 可能不存在，忽略
      }
    }

    // 如果选了模板，自动创建模板中的模型
    if (selectedTemplate) {
      for (const m of selectedTemplate.models) {
        await createModel({
          providerId: provider.id,
          modelId: m.id,
          displayName: m.name,
          authType: JSON.stringify(m.authType),
          isDefault: false,
          configJson: m.generationConfig ? JSON.stringify(m.generationConfig) : undefined,
        });
      }
    }

    onCreated();
  };

  // 导入已发现的供应商
  const importDiscovered = async (dp: DiscoveredProvider) => {
    const provider = await createProvider({
      name: dp.preset_name ?? dp.name,
      baseUrl: dp.base_url,
      apiKeyEnv: dp.env_key,
      proxyMode: "direct",
      billingType: "pay_per_use",
      authHeader: dp.protocol === "anthropic" ? "x-api-key" : undefined,
    });
    for (const m of dp.models) {
      await createModel({
        providerId: provider.id,
        modelId: m.id,
        displayName: m.name,
        authType: JSON.stringify(m.auth_type),
        isDefault: false,
      });
    }
    onCreated();
  };

  // 模板选择步骤
  if (step === "template") {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-[var(--bg-overlay)]" onClick={onClose}>
        <div className="bg-[var(--bg-panel)] rounded-md p-0 w-[480px] border border-[var(--border-strong)] shadow-[var(--shadow-dialog)]" onClick={(e) => e.stopPropagation()}>
          <div className="px-4 py-3 border-b border-[var(--border-strong)]">
            <h3 className="text-sm font-medium text-[var(--text-primary)]">添加供应商</h3>
            {/* Tab 切换 */}
            <div className="flex gap-1 mt-2">
              <button
                onClick={() => setTab("templates")}
                className={`px-3 h-6 text-[11px] rounded-md transition-colors ${
                  tab === "templates"
                    ? "bg-[var(--accent)]/10 text-[var(--accent)] font-medium"
                    : "text-[var(--text-muted)] hover:text-[var(--text-primary)]"
                }`}
              >
                模板
              </button>
              {discovered.length > 0 && (
                <button
                  onClick={() => setTab("discovered")}
                  className={`px-3 h-6 text-[11px] rounded-md transition-colors ${
                    tab === "discovered"
                      ? "bg-[var(--accent)]/10 text-[var(--accent)] font-medium"
                      : "text-[var(--text-muted)] hover:text-[var(--text-primary)]"
                  }`}
                >
                  已有配置 ({discovered.length})
                </button>
              )}
            </div>
          </div>

          {tab === "discovered" ? (
            <div className="p-3 space-y-2 max-h-[400px] overflow-auto">
              {discovered.map((dp, i) => (
                <div key={i} className="flex items-start gap-3 p-3 bg-[var(--bg-card)] border border-[var(--border)] rounded-sm">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-[13px] font-medium text-[var(--text-primary)]">
                        {dp.preset_name ?? dp.name}
                      </span>
                      {dp.is_preset && (
                        <span className="text-[9px] text-[#3fb950] bg-[#3fb9501a] px-1 rounded">预设</span>
                      )}
                      {!dp.is_preset && (
                        <span className="text-[9px] text-[#d29922] bg-[#d299221a] px-1 rounded">自定义</span>
                      )}
                      {dp.has_key ? (
                        <span className="text-[9px] text-[#3fb950]">Key ✓</span>
                      ) : (
                        <span className="text-[9px] text-[#f48771]">Key ✗</span>
                      )}
                    </div>
                    <span className="text-[11px] text-[var(--text-muted)] font-mono">{dp.base_url}</span>
                    <div className="flex flex-wrap gap-1 mt-1.5">
                      {dp.models.map((m) => (
                        <span
                          key={m.id}
                          className={`text-[10px] px-1 rounded-sm ${
                            m.from_preset
                              ? "text-[#3fb950] bg-[#3fb95012] border border-dashed border-[#3fb95040]"
                              : "text-[var(--text-muted)] bg-[var(--bg-input)]"
                          }`}
                          title={m.from_preset ? "预设补齐" : undefined}
                        >
                          {m.from_preset ? `+ ${m.name}` : m.name}
                        </span>
                      ))}
                    </div>
                  </div>
                  <button
                    onClick={() => importDiscovered(dp)}
                    className="shrink-0 px-3 h-7 text-[11px] bg-[var(--accent)] text-white rounded-sm hover:opacity-90 transition-opacity"
                  >
                    导入
                  </button>
                </div>
              ))}
            </div>
          ) : (
            <div className="p-3 grid grid-cols-2 gap-2 max-h-[400px] overflow-auto">
              {TEMPLATES.map((t) => (
                <button
                  key={t.name}
                  onClick={() => selectTemplate(t)}
                  className="flex flex-col items-start p-3 bg-[var(--bg-card)] border border-[var(--border)] rounded-sm hover:border-[#007fd4] hover:bg-[var(--bg-hover)] transition-colors text-left"
                >
                  <span className="text-[13px] font-medium text-[var(--text-primary)]">{t.name}</span>
                  <span className="text-[11px] text-[var(--text-muted)] mt-0.5 truncate w-full">{t.baseUrl}</span>
                  <div className="flex gap-1 mt-1.5">
                    {t.models.slice(0, 3).map((m) => (
                      <span key={m.id} className="text-[10px] text-[var(--text-muted)] bg-[var(--bg-input)] px-1 rounded-sm">{m.name}</span>
                    ))}
                  </div>
                </button>
              ))}
              <button
                onClick={() => setStep("form")}
                className="flex flex-col items-center justify-center p-3 bg-[var(--bg-card)] border border-[var(--border)] border-dashed rounded-sm hover:border-[#007fd4] hover:bg-[var(--bg-hover)] transition-colors"
              >
                <Plus size={20} className="text-[var(--text-muted)]" />
                <span className="text-[12px] text-[var(--text-muted)] mt-1">自定义</span>
              </button>
            </div>
          )}

          <div className="px-4 py-2 border-t border-[var(--border-strong)] flex justify-end">
            <button onClick={onClose} className="px-3 h-7 text-xs text-[var(--text-primary)] hover:bg-[var(--bg-input)] rounded-sm transition-colors">取消</button>
          </div>
        </div>
      </div>
    );
  }

  // 表单步骤
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-[var(--bg-overlay)]" onClick={onClose}>
      <div className="bg-[var(--bg-panel)] rounded-md p-0 w-[420px] border border-[var(--border-strong)] shadow-[var(--shadow-dialog)]" onClick={(e) => e.stopPropagation()}>
        <div className="px-4 py-3 border-b border-[var(--border-strong)]">
          <h3 className="text-sm font-medium text-[var(--text-primary)]">
            {selectedTemplate ? `配置 ${selectedTemplate.name}` : "自定义供应商"}
          </h3>
        </div>
        <div className="p-4 space-y-3">
          <Field label="名称" value={form.name} onChange={(v) => setForm({ ...form, name: v })} />
          <Field label="Base URL" value={form.baseUrl} onChange={(v) => setForm({ ...form, baseUrl: v })} />
          <Field label="环境变量名" value={form.apiKeyEnv} onChange={(v) => setForm({ ...form, apiKeyEnv: v })} placeholder="OPENAI_API_KEY" />
          <div>
            <label className="block text-[11px] text-[var(--text-muted)] mb-1">API Key (SK)</label>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
              className="w-full h-8 bg-[var(--bg-input)] border border-[var(--border)] rounded-sm px-2 text-[13px] text-[var(--text-primary)] placeholder:text-[#5a5a5a] focus:border-[#007fd4] outline-none font-mono"
            />
            <p className="text-[10px] text-[#5a5a5a] mt-0.5">写入 settings.json 的 env.{form.apiKeyEnv}</p>
          </div>
          <div>
            <label className="block text-[11px] text-[var(--text-muted)] mb-1">计费类型</label>
            <select
              value={form.billingType}
              onChange={(e) => setForm({ ...form, billingType: e.target.value })}
              className="w-full h-8 bg-[var(--bg-input)] border border-[var(--border)] rounded-sm px-2 text-[13px] text-[var(--text-primary)] focus:border-[#007fd4] outline-none"
            >
              {BILLING_TYPES.map((b) => (
                <option key={b.value} value={b.value}>{b.label}</option>
              ))}
            </select>
          </div>
        </div>
        <div className="px-4 py-2 border-t border-[var(--border-strong)] flex justify-end gap-2">
          <button onClick={() => { setStep("template"); setSelectedTemplate(null); }} className="px-3 h-7 text-xs text-[var(--text-primary)] hover:bg-[var(--bg-input)] rounded-sm transition-colors">返回</button>
          <button onClick={onClose} className="px-3 h-7 text-xs text-[var(--text-primary)] hover:bg-[var(--bg-input)] rounded-sm transition-colors">取消</button>
          <button
            onClick={submit}
            disabled={!form.name || !form.baseUrl}
            className="px-3 h-7 text-xs bg-[var(--accent)] text-white rounded-sm hover:bg-[var(--accent-hover)] disabled:opacity-40 transition-colors"
          >
            创建{selectedTemplate ? ` (含 ${selectedTemplate.models.length} 个模型)` : ""}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── 添加模型弹窗 ─────────────────────────────────────────

function AddModelDialog({
  providerId,
  onClose,
  onCreated,
}: {
  providerId: number;
  onClose: () => void;
  onCreated: () => void;
}) {
  const [form, setForm] = useState({
    modelId: "",
    displayName: "",
    authTypes: ["openai"] as string[],
    isDefault: false,
  });

  const toggleAuth = (t: string) => {
    setForm((f) => ({
      ...f,
      authTypes: f.authTypes.includes(t)
        ? f.authTypes.filter((x) => x !== t)
        : [...f.authTypes, t],
    }));
  };

  const submit = async () => {
    await createModel({
      providerId,
      modelId: form.modelId,
      displayName: form.displayName || undefined,
      authType: JSON.stringify(form.authTypes),
      isDefault: form.isDefault,
    });
    onCreated();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-[var(--bg-overlay)]" onClick={onClose}>
      <div className="bg-[var(--bg-panel)] rounded-md p-0 w-[380px] border border-[var(--border-strong)] shadow-[var(--shadow-dialog)]" onClick={(e) => e.stopPropagation()}>
        <div className="px-4 py-3 border-b border-[var(--border-strong)]">
          <h3 className="text-sm font-medium text-[var(--text-primary)]">添加模型</h3>
        </div>
        <div className="p-4 space-y-3">
          <Field label="模型 ID" value={form.modelId} onChange={(v) => setForm({ ...form, modelId: v })} placeholder="gpt-4o" />
          <Field label="显示名称" value={form.displayName} onChange={(v) => setForm({ ...form, displayName: v })} placeholder="GPT-4o" />
          <div>
            <label className="block text-[11px] text-[var(--text-muted)] mb-1">支持的协议（可多选）</label>
            <div className="flex gap-3">
              {AUTH_OPTIONS.map((t) => (
                <label key={t} className="flex items-center gap-1.5 text-[13px] text-[var(--text-primary)] cursor-pointer">
                  <input
                    type="checkbox"
                    checked={form.authTypes.includes(t)}
                    onChange={() => toggleAuth(t)}
                    className="accent-[#0e639c]"
                  />
                  {t}
                </label>
              ))}
            </div>
          </div>
          <label className="flex items-center gap-2 text-[13px] text-[var(--text-primary)] cursor-pointer">
            <input type="checkbox" checked={form.isDefault} onChange={(e) => setForm({ ...form, isDefault: e.target.checked })} className="accent-[#0e639c]" />
            设为默认模型
          </label>
        </div>
        <div className="px-4 py-2 border-t border-[var(--border-strong)] flex justify-end gap-2">
          <button onClick={onClose} className="px-3 h-7 text-xs text-[var(--text-primary)] hover:bg-[var(--bg-input)] rounded-sm transition-colors">取消</button>
          <button onClick={submit} disabled={!form.modelId} className="px-3 h-7 text-xs bg-[var(--accent)] text-white rounded-sm hover:bg-[var(--accent-hover)] disabled:opacity-40 transition-colors">添加</button>
        </div>
      </div>
    </div>
  );
}

// ── 通用字段组件 ─────────────────────────────────────────

function Field({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <div>
      <label className="block text-[11px] text-[var(--text-muted)] mb-1">{label}</label>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full h-8 bg-[var(--bg-input)] border border-[var(--border)] rounded-sm px-2 text-[13px] text-[var(--text-primary)] placeholder:text-[#5a5a5a] focus:border-[#007fd4] outline-none"
      />
    </div>
  );
}

// ── settings.json 读写辅助 ───────────────────────────────

async function readSettingsFromTauri(): Promise<Record<string, unknown>> {
  try {
    const { readSettings } = await import("@/lib/tauri");
    return await readSettings();
  } catch {
    return {};
  }
}

async function writeSettingsToTauri(_settings: Record<string, unknown>): Promise<void> {
  // TODO: 实现 settings.json 直接写入命令
}
