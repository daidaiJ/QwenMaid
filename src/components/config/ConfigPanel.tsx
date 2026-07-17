import { useState, useEffect, useCallback, useMemo } from "react";
import { ResizableColumns } from "@/components/layout/ResizableColumns";
import {
  Cpu,
  SlidersHorizontal,
  Settings,
  Wrench,
  Palette,
  FileText,
  Brain,
  Shield,
  Code,
  Lock,
  Monitor,
  Fingerprint,
  RotateCcw,
  Save,
  PanelBottom,
  Plug,
  Webhook,
} from "lucide-react";
import { readSettings, writeSettingsField, getQwenPaths, listModels } from "@/lib/tauri";
import {
  settingCategories,
  getByPath,
  setByPath,
  deleteByPath,
  type SettingField,
} from "./settingsSchema";
import {
  Toggle,
  Select,
  TextInput,
  SecretInput,
  NumberInput,
  TagInput,
  FilePathField,
  QuickPathNav,
} from "./FormControls";

// ── 图标映射 ─────────────────────────────────────────────

const ICONS: Record<string, typeof Cpu> = {
  Cpu,
  SlidersHorizontal,
  Settings,
  Wrench,
  Palette,
  FileText,
  Brain,
  Shield,
  Code,
  Lock,
  Monitor,
  Fingerprint,
  PanelBottom,
  Plug,
  Webhook,
};

// ── 主面板 ───────────────────────────────────────────────

export function ConfigPanel() {
  const [activeCategory, setActiveCategory] = useState("model");
  const [settings, setSettings] = useState<Record<string, unknown>>({});
  const [original, setOriginal] = useState<Record<string, unknown>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState<Set<string>>(new Set());
  const [toast, setToast] = useState<string | null>(null);
  const [qwenPaths, setQwenPaths] = useState<Record<string, string>>({});
  const [modelIds, setModelIds] = useState<string[]>([]);

  // 读取 settings.json、Qwen Code 路径和 DB 模型列表
  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [data, paths, models] = await Promise.all([
        readSettings(),
        getQwenPaths().catch(() => ({})),
        listModels().catch(() => []),
      ]);
      setSettings(data ?? {});
      setOriginal(data ?? {});
      setQwenPaths(paths);
      setModelIds([...new Set(models.map((m) => m.model_id))]);
      setDirty(new Set());
    } catch {
      setSettings({});
      setOriginal({});
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  // 更新字段值
  const updateField = useCallback(
    (path: string, value: unknown) => {
      setSettings((prev) => {
        if (value === undefined) return deleteByPath(prev, path);
        return setByPath(prev, path, value);
      });
      setDirty((prev) => new Set(prev).add(path));
    },
    []
  );

  // 重置单个字段
  const resetField = useCallback(
    (field: SettingField) => {
      const orig = getByPath(original, field.path);
      if (orig !== undefined) {
        updateField(field.path, orig);
      } else {
        updateField(
          field.path,
          field.defaultValue !== undefined ? field.defaultValue : undefined
        );
      }
    },
    [original, updateField]
  );

  // 保存所有已修改的字段
  // 规则：值等于默认值的字段不写入配置文件（删除该 key），避免配置文件膨胀
  const saveAll = useCallback(async () => {
    if (dirty.size === 0) return;
    setSaving(true);
    try {
      for (const path of dirty) {
        const value = getByPath(settings, path);
        const field = allFieldsMap.get(path);
        const isDefault =
          field?.defaultValue !== undefined &&
          value === field.defaultValue;
        const isEmpty =
          value === undefined || value === null || value === "";

        if (isDefault || isEmpty) {
          // 值等于默认值或为空 → 删除该字段（不写入配置文件）
          await writeSettingsField(path, null);
        } else {
          await writeSettingsField(path, value);
        }
      }
      // 重新加载以获取实际文件状态
      const fresh = await readSettings();
      setSettings(fresh ?? {});
      setOriginal(fresh ?? {});
      setDirty(new Set());
      showToast("已保存");
    } catch (e) {
      showToast(`保存失败: ${e}`);
    } finally {
      setSaving(false);
    }
  }, [dirty, settings]);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 2500);
  };

  const category = useMemo(
    () => settingCategories.find((c) => c.id === activeCategory)!,
    [activeCategory]
  );

  // 所有字段的 path → field 映射（用于保存时查找默认值）
  const allFieldsMap = useMemo(() => {
    const map = new Map<string, SettingField>();
    for (const cat of settingCategories) {
      for (const f of cat.fields) map.set(f.path, f);
    }
    return map;
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-[#8b949e] text-sm">
        加载配置…
      </div>
    );
  }

  return (
    <>
    <ResizableColumns
      autoSaveId="config-panel-v3"
      left={{
        defaultSize: 20,
        minSize: 12,
        maxSize: 30,
        className: "bg-[var(--bg-sidebar)] flex flex-col",
        children: (
          <>
            <div className="flex items-center justify-between px-3 h-9 border-b border-[var(--border)]">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-secondary)]">
                配置分类
              </span>
            </div>
            <nav className="flex-1 overflow-auto py-0.5">
              {settingCategories.map((cat) => {
                const Icon = ICONS[cat.icon] ?? Settings;
                const catDirty = cat.fields.some((f) => dirty.has(f.path));
                return (
                  <button
                    key={cat.id}
                    onClick={() => setActiveCategory(cat.id)}
                    className={`w-full flex items-center gap-2 px-3 h-8 text-left text-[13px] transition-colors ${
                      activeCategory === cat.id
                        ? "bg-[var(--accent-light)] text-[var(--accent)] font-medium"
                        : "text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
                    }`}
                  >
                    <Icon size={14} className="shrink-0 text-[var(--text-muted)]" />
                    <span className="truncate flex-1">{cat.label}</span>
                    {catDirty && (
                      <span className="w-1.5 h-1.5 rounded-full bg-[var(--color-warning)]" />
                    )}
                  </button>
                );
              })}
            </nav>
            <div className="border-t border-[var(--border)] p-2 space-y-1.5">
              <button
                onClick={saveAll}
                disabled={saving || dirty.size === 0}
                className="w-full flex items-center justify-center gap-1.5 h-7 text-xs bg-[var(--accent)] text-white rounded-sm hover:bg-[var(--accent-hover)] disabled:opacity-40 transition-colors"
              >
                <Save size={12} />
                {saving ? "保存中…" : `保存${dirty.size > 0 ? ` (${dirty.size})` : ""}`}
              </button>
              <button
                onClick={load}
                className="w-full flex items-center justify-center gap-1.5 h-7 text-xs text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] rounded-sm transition-colors"
              >
                <RotateCcw size={12} />
                重新加载
              </button>
            </div>
          </>
        ),
      }}
      center={{
        className: "overflow-auto",
        children: (
          <div className="p-6 max-w-2xl">
            {/* 常用目录快捷入口 */}
            {qwenPaths.qwenDir && (
              <QuickPathNav
                entries={[
                  { label: "~/.qwen", path: qwenPaths.qwenDir },
                  { label: "settings.json", path: qwenPaths.settingsFile },
                  { label: "skills", path: qwenPaths.skillsDir },
                  { label: "extensions", path: qwenPaths.extensionsDir },
                  { label: "projects", path: qwenPaths.projectsDir },
                ].filter((e) => e.path)}
              />
            )}

            {/* 分类标题 */}
            <div className="mb-4">
            <h2 className="text-base font-medium text-[var(--text-primary)]">
              {category.label}
            </h2>
            {category.description && (
              <p className="text-[12px] text-[var(--text-secondary)] mt-0.5">
                {category.description}
              </p>
            )}
          </div>

          {/* 字段列表或自定义渲染器 */}
          {category.customRenderer === "mcpServers" ? (
            <McpServersEditor
              settings={settings}
              onUpdate={(path, val) => updateField(path, val)}
            />
          ) : category.customRenderer === "hooks" ? (
            <HooksEditor
              settings={settings}
              onUpdate={(path, val) => updateField(path, val)}
            />
          ) : (
          <div className="divide-y divide-[var(--border)]/50">
            {category.fields.map((field) => (
              <FieldRenderer
                key={field.path}
                field={field}
                value={getByPath(settings, field.path)}
                isDirty={dirty.has(field.path)}
                onChange={(v) => updateField(field.path, v)}
                onReset={() => resetField(field)}
                modelIds={modelIds}
              />
            ))}
          </div>
          )}
        </div>
        ),
      }}
    />

    {/* Toast */}
    {toast && (
      <div className="fixed bottom-12 right-4 z-50 px-3 py-1.5 bg-[var(--bg-panel)] border border-[var(--border-strong)] rounded-sm text-[12px] text-[var(--text-primary)] shadow-lg">
        {toast}
      </div>
    )}
  </>
  );
}

// ── MCP 服务器编辑器 ─────────────────────────────────────

interface McpServerConfig {
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  httpUrl?: string;
  description?: string;
  timeout?: number;
  trust?: boolean;
  includeTools?: string[];
  excludeTools?: string[];
}

function McpServersEditor({
  settings,
  onUpdate,
}: {
  settings: Record<string, unknown>;
  onUpdate: (path: string, val: unknown) => void;
}) {
  const servers = (settings.mcpServers ?? {}) as Record<string, McpServerConfig>;
  const names = Object.keys(servers);
  const [selected, setSelected] = useState<string | null>(names[0] ?? null);
  const [adding, setAdding] = useState(false);
  const [newName, setNewName] = useState("");

  const selectedCfg = selected ? servers[selected] : null;

  const updateServer = (name: string, key: string, value: unknown) => {
    onUpdate(`mcpServers.${name}.${key}`, value);
  };

  const addServer = () => {
    if (!newName.trim()) return;
    onUpdate(`mcpServers.${newName.trim()}`, { command: "", args: [] });
    setSelected(newName.trim());
    setNewName("");
    setAdding(false);
  };

  const removeServer = (name: string) => {
    onUpdate(`mcpServers.${name}`, undefined);
    if (selected === name) setSelected(null);
  };

  return (
    <div className="flex gap-4 min-h-[300px]">
      {/* 左侧列表 */}
      <div className="w-48 shrink-0 space-y-1">
        {names.map((name) => (
          <button
            key={name}
            onClick={() => setSelected(name)}
            className={`w-full text-left px-3 py-2 rounded-md text-[12px] transition-colors flex items-center justify-between group ${
              selected === name
                ? "bg-[var(--accent-light)] text-[var(--accent)]"
                : "hover:bg-[var(--bg-hover)] text-[var(--text-primary)]"
            }`}
          >
            <span className="truncate">{name}</span>
            <button
              onClick={(e) => { e.stopPropagation(); removeServer(name); }}
              className="opacity-0 group-hover:opacity-100 text-[var(--text-muted)] hover:text-[var(--color-error)] transition-opacity"
            >
              ×
            </button>
          </button>
        ))}
        {adding ? (
          <div className="flex gap-1">
            <input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addServer()}
              placeholder="服务器名称"
              autoFocus
              className="flex-1 h-7 bg-[var(--bg-input)] border border-[var(--border)] rounded-sm px-2 text-[11px] text-[var(--text-primary)] outline-none"
            />
            <button onClick={addServer} className="px-2 h-7 text-[10px] bg-[var(--accent)] text-white rounded-sm">+</button>
          </div>
        ) : (
          <button
            onClick={() => setAdding(true)}
            className="w-full text-left px-3 py-1.5 text-[11px] text-[var(--text-muted)] hover:text-[var(--accent)] transition-colors"
          >
            + 添加服务器
          </button>
        )}
      </div>

      {/* 右侧详情 */}
      {selectedCfg ? (
        <div className="flex-1 space-y-3">
          <div className="space-y-2">
            <label className="text-[11px] text-[var(--text-muted)]">描述</label>
            <input
              value={selectedCfg.description ?? ""}
              onChange={(e) => updateServer(selected!, "description", e.target.value || undefined)}
              placeholder="可选描述"
              className="w-full h-8 bg-[var(--bg-input)] border border-[var(--border)] rounded-md px-3 text-[12px] text-[var(--text-primary)] outline-none"
            />
          </div>
          <div className="space-y-2">
            <label className="text-[11px] text-[var(--text-muted)]">command（stdio 模式）</label>
            <input
              value={selectedCfg.command ?? ""}
              onChange={(e) => updateServer(selected!, "command", e.target.value || undefined)}
              placeholder="如 npx, node, python"
              className="w-full h-8 bg-[var(--bg-input)] border border-[var(--border)] rounded-md px-3 text-[12px] font-mono text-[var(--text-primary)] outline-none"
            />
          </div>
          <div className="space-y-2">
            <label className="text-[11px] text-[var(--text-muted)]">args（每行一个）</label>
            <textarea
              value={(selectedCfg.args ?? []).join("\n")}
              onChange={(e) => updateServer(selected!, "args", e.target.value.split("\n").filter(Boolean))}
              placeholder="-m\nmcp_server"
              rows={3}
              className="w-full bg-[var(--bg-input)] border border-[var(--border)] rounded-md px-3 py-2 text-[12px] font-mono text-[var(--text-primary)] outline-none resize-y"
            />
          </div>
          <div className="space-y-2">
            <label className="text-[11px] text-[var(--text-muted)]">url（SSE 模式）</label>
            <input
              value={selectedCfg.url ?? ""}
              onChange={(e) => updateServer(selected!, "url", e.target.value || undefined)}
              placeholder="http://host/sse"
              className="w-full h-8 bg-[var(--bg-input)] border border-[var(--border)] rounded-md px-3 text-[12px] font-mono text-[var(--text-primary)] outline-none"
            />
          </div>
          <div className="space-y-2">
            <label className="text-[11px] text-[var(--text-muted)]">httpUrl（Streamable HTTP 模式）</label>
            <input
              value={selectedCfg.httpUrl ?? ""}
              onChange={(e) => updateServer(selected!, "httpUrl", e.target.value || undefined)}
              placeholder="http://host/mcp"
              className="w-full h-8 bg-[var(--bg-input)] border border-[var(--border)] rounded-md px-3 text-[12px] font-mono text-[var(--text-primary)] outline-none"
            />
          </div>
          <div className="flex items-center gap-3">
            <label className="flex items-center gap-2 text-[12px] text-[var(--text-primary)]">
              <input
                type="checkbox"
                checked={selectedCfg.trust ?? false}
                onChange={(e) => updateServer(selected!, "trust", e.target.checked || undefined)}
                className="accent-[var(--accent)]"
              />
              跳过确认（trust）
            </label>
            <div className="flex items-center gap-2">
              <label className="text-[11px] text-[var(--text-muted)]">超时 (ms)</label>
              <input
                type="number"
                value={selectedCfg.timeout ?? ""}
                onChange={(e) => updateServer(selected!, "timeout", e.target.value ? Number(e.target.value) : undefined)}
                placeholder="30000"
                className="w-24 h-7 bg-[var(--bg-input)] border border-[var(--border)] rounded-sm px-2 text-[11px] font-mono text-[var(--text-primary)] outline-none"
              />
            </div>
          </div>
        </div>
      ) : (
        <div className="flex-1 flex items-center justify-center text-[var(--text-muted)] text-[12px]">
          选择一个 MCP 服务器或添加新的
        </div>
      )}
    </div>
  );
}

// ── Hooks 编辑器 ─────────────────────────────────────────

const HOOK_EVENTS = ["SessionStart", "PreToolUse", "SessionEnd"] as const;

function HooksEditor({
  settings,
  onUpdate,
}: {
  settings: Record<string, unknown>;
  onUpdate: (path: string, val: unknown) => void;
}) {
  const hooks = (settings.hooks ?? {}) as Record<string, { command: string }[]>;
  const [selectedEvent, setSelectedEvent] = useState<string>(HOOK_EVENTS[0]);

  const entries = hooks[selectedEvent] ?? [];

  const updateEntry = (idx: number, command: string) => {
    const newEntries = [...entries];
    newEntries[idx] = { command };
    onUpdate(`hooks.${selectedEvent}`, newEntries);
  };

  const addEntry = () => {
    onUpdate(`hooks.${selectedEvent}`, [...entries, { command: "" }]);
  };

  const removeEntry = (idx: number) => {
    const newEntries = entries.filter((_, i) => i !== idx);
    if (newEntries.length === 0) {
      onUpdate(`hooks.${selectedEvent}`, undefined);
    } else {
      onUpdate(`hooks.${selectedEvent}`, newEntries);
    }
  };

  return (
    <div className="flex gap-4 min-h-[300px]">
      {/* 左侧事件列表 */}
      <div className="w-40 shrink-0 space-y-1">
        {HOOK_EVENTS.map((ev) => (
          <button
            key={ev}
            onClick={() => setSelectedEvent(ev)}
            className={`w-full text-left px-3 py-2 rounded-md text-[12px] transition-colors ${
              selectedEvent === ev
                ? "bg-[var(--accent-light)] text-[var(--accent)]"
                : "hover:bg-[var(--bg-hover)] text-[var(--text-primary)]"
            }`}
          >
            {ev}
            <span className="ml-1 text-[10px] text-[var(--text-muted)]">
              ({(hooks[ev] ?? []).length})
            </span>
          </button>
        ))}
      </div>

      {/* 右侧钩子列表 */}
      <div className="flex-1 space-y-2">
        {entries.map((entry, idx) => (
          <div key={idx} className="flex items-center gap-2">
            <input
              value={entry.command}
              onChange={(e) => updateEntry(idx, e.target.value)}
              placeholder="bash ~/.qwen/hooks/script.sh"
              className="flex-1 h-8 bg-[var(--bg-input)] border border-[var(--border)] rounded-md px-3 text-[12px] font-mono text-[var(--text-primary)] outline-none"
            />
            <button
              onClick={() => removeEntry(idx)}
              className="text-[var(--text-muted)] hover:text-[var(--color-error)] transition-colors px-1"
            >
              ×
            </button>
          </div>
        ))}
        <button
          onClick={addEntry}
          className="text-[11px] text-[var(--text-muted)] hover:text-[var(--accent)] transition-colors"
        >
          + 添加钩子
        </button>
      </div>
    </div>
  );
}

// ── 单个字段渲染器 ───────────────────────────────────────

// 通用样式常量
const labelCls = "text-[13px] text-[var(--text-primary)] font-medium";
const descCls = "text-[11px] text-[var(--text-secondary)] mt-0.5 leading-relaxed";
const restartBadge = "text-[10px] text-[var(--color-warning)] bg-[var(--color-warning-bg)] px-1.5 py-0.5 rounded font-medium";
const dirtyDot = "w-1.5 h-1.5 rounded-full bg-[var(--color-warning)]";
const resetBtn = "text-[var(--text-muted)] hover:text-[var(--color-error)] opacity-0 group-hover:opacity-100 transition-opacity";
const defaultHint = "text-[10px] text-[var(--text-muted)] opacity-70";

function defaultLabel(field: SettingField): string | null {
  if (field.defaultValue === undefined) return null;
  if (field.type === "toggle") return `默认: ${field.defaultValue ? "开" : "关"}`;
  if (field.type === "select") {
    const opt = field.options?.find((o) => o.value === String(field.defaultValue));
    return `默认: ${opt?.label ?? field.defaultValue}`;
  }
  if (field.type === "number") return `默认: ${field.defaultValue}${field.unit ? ` ${field.unit}` : ""}`;
  return null;
}

function FieldRenderer({
  field,
  value,
  isDirty,
  onChange,
  onReset,
  modelIds,
}: {
  field: SettingField;
  value: unknown;
  isDirty: boolean;
  onChange: (v: unknown) => void;
  onReset: () => void;
  modelIds: string[];
}) {
  // toggle 类型用 inline 布局
  if (field.type === "toggle") {
    const effectiveValue = value !== undefined ? Boolean(value) : Boolean(field.defaultValue ?? false);
    const isUnset = value === undefined;
    return (
      <div className="flex items-center justify-between py-2.5 px-2 group hover:bg-[var(--bg-hover)] rounded-md transition-colors">
        <div className="flex-1 min-w-0 mr-4">
          <div className="flex items-center gap-2">
            <label className={labelCls}>{field.label}</label>
            {isUnset && <span className={defaultHint}>(未设置)</span>}
            {isDirty && <span className={dirtyDot} />}
            {field.requiresRestart && <span className={restartBadge}>重启生效</span>}
          </div>
          <div className="flex items-center gap-2">
            {field.description && <p className={descCls}>{field.description}</p>}
            {isUnset && defaultLabel(field) && <span className={defaultHint}>{defaultLabel(field)}</span>}
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Toggle value={effectiveValue} onChange={onChange} />
          {isDirty && (
            <button onClick={onReset} className={resetBtn} title="重置">
              <RotateCcw size={12} />
            </button>
          )}
        </div>
      </div>
    );
  }

  // select 类型也用 inline 布局
  if (field.type === "select") {
    return (
      <div className="flex items-center justify-between py-2.5 px-2 group hover:bg-[var(--bg-hover)] rounded-md transition-colors">
        <div className="flex-1 min-w-0 mr-4">
          <div className="flex items-center gap-2">
            <label className={labelCls}>{field.label}</label>
            {isDirty && <span className={dirtyDot} />}
            {field.requiresRestart && <span className={restartBadge}>重启生效</span>}
          </div>
          {field.description && <p className={descCls}>{field.description}</p>}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Select
            value={String(value ?? field.defaultValue ?? "")}
            onChange={onChange}
            options={
              field.path === "model.name" || field.path === "fastModel"
                ? modelIds.map((id) => ({ value: id, label: id }))
                : field.options ?? []
            }
            placeholder={field.placeholder}
          />
          {isDirty && (
            <button onClick={onReset} className={resetBtn} title="重置">
              <RotateCcw size={12} />
            </button>
          )}
        </div>
      </div>
    );
  }

  // 其他类型用块级布局
  return (
    <div className="py-3 px-2 group hover:bg-[var(--bg-hover)] rounded-md transition-colors">
      <div className="flex items-center gap-2 mb-2">
        <label className={labelCls}>{field.label}</label>
        {isDirty && <span className={dirtyDot} />}
        {field.requiresRestart && <span className={restartBadge}>重启生效</span>}
        {isDirty && (
          <button onClick={onReset} className={`ml-auto ${resetBtn} flex items-center gap-1`} title="重置">
            <RotateCcw size={11} />
            <span className="text-[10px]">重置</span>
          </button>
        )}
      </div>
      {field.description && <p className={descCls}>{field.description}</p>}
      {renderControl(field, value, onChange)}
    </div>
  );
}

// ── 控件渲染 ─────────────────────────────────────────────

function renderControl(
  field: SettingField,
  value: unknown,
  onChange: (v: unknown) => void,
  dynamicOptions?: { value: string; label: string }[],
) {
  switch (field.type) {
    case "text":
      return (
        <TextInput
          value={String(value ?? "")}
          onChange={onChange}
          placeholder={field.placeholder}
        />
      );

    case "password":
      return (
        <SecretInput
          value={String(value ?? "")}
          onChange={onChange}
          placeholder={field.placeholder}
        />
      );

    case "number":
      return (
        <NumberInput
          value={typeof value === "number" ? value : undefined}
          onChange={onChange}
          min={field.min}
          max={field.max}
          step={field.step}
          placeholder={field.placeholder ?? (field.defaultValue !== undefined ? `默认: ${field.defaultValue}` : undefined)}
          unit={field.unit}
        />
      );

    case "tags":
      return (
        <TagInput
          value={Array.isArray(value) ? (value as string[]) : []}
          onChange={onChange}
          placeholder={field.placeholder ?? "输入后按 Enter 添加"}
          suggestions={field.suggestions}
        />
      );

    case "select":
      return (
        <Select
          value={String(value ?? "")}
          onChange={onChange}
          options={dynamicOptions ?? field.options ?? []}
          placeholder={field.placeholder}
        />
      );

    case "path": {
      const strVal = String(value ?? "");
      // 从命令中提取可能的路径用于 reveal
      const revealTarget = strVal.replace(/^(bash|sh|cmd|powershell)\s+/i, "").trim();
      return (
        <FilePathField
          value={strVal}
          onChange={onChange}
          placeholder={field.placeholder}
          revealPath={revealTarget || undefined}
        />
      );
    }

    default:
      return (
        <TextInput
          value={typeof value === "object" ? JSON.stringify(value) : String(value ?? "")}
          onChange={onChange}
          mono
        />
      );
  }
}
