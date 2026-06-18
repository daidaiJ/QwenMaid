import { useState, useCallback, useMemo, useRef } from "react";
import {
  GenericThreeColumnPanel,
  SimpleMarkdownEditor,
  type ListItem,
} from "@/components/layout/GenericPanel";
import { ChevronDown, Save, Check } from "lucide-react";
import { listAgents, readAgent, writeAgent, deleteAgent } from "@/lib/tauri";
import { useConfiguredModels } from "@/hooks/useConfiguredModels";

// ── 可交互字段 schema ────────────────────────────────────

interface FieldOption {
  label: string;
  value: string;
  description?: string;
}

interface FieldSchema {
  key: string;
  label: string;
  options: FieldOption[];
  /** 不在选项列表中的值是否允许自由输入 */
  allowCustom?: boolean;
}

const INTERACTIVE_FIELDS: FieldSchema[] = [
  {
    key: "model",
    label: "模型",
    options: [
      { value: "inherit", label: "inherit", description: "继承主会话模型" },
      { value: "fast", label: "fast", description: "使用 fastModel" },
    ],
    allowCustom: true,
  },
  {
    key: "approvalMode",
    label: "审批模式",
    options: [
      { value: "default", label: "default", description: "工具需交互审批" },
      { value: "plan", label: "plan", description: "只分析不执行" },
      { value: "auto-edit", label: "auto-edit", description: "自动审批（推荐）" },
      { value: "yolo", label: "yolo", description: "全部自动含危险操作" },
      { value: "bubble", label: "bubble", description: "冒泡到父会话审批" },
    ],
  },
  {
    key: "color",
    label: "颜色",
    options: [
      { value: "red", label: "red" },
      { value: "blue", label: "blue" },
      { value: "green", label: "green" },
      { value: "yellow", label: "yellow" },
      { value: "purple", label: "purple" },
      { value: "orange", label: "orange" },
      { value: "pink", label: "pink" },
      { value: "cyan", label: "cyan" },
    ],
  },
];

const COLOR_DOT: Record<string, string> = {
  red: "#f87171", blue: "#60a5fa", green: "#4ade80", yellow: "#facc15",
  purple: "#a78bfa", orange: "#fb923c", pink: "#f472b6", cyan: "#22d3ee",
};

// ── frontmatter 工具函数 ─────────────────────────────────

function updateFrontmatterLine(fm: string, key: string, newValue: string): string {
  const lines = fm.split("\n");
  let found = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();
    // 匹配 "key: ..." 或 "key:" （无值）
    if (trimmed === key || trimmed.startsWith(`${key}:`)) {
      // 保留原始缩进
      const indent = line.match(/^(\s*)/)?.[1] ?? "";
      lines[i] = `${indent}${key}: ${newValue}`;
      found = true;
      break;
    }
  }
  if (!found) {
    // 字段不存在，追加到末尾
    lines.push(`${key}: ${newValue}`);
  }
  return lines.join("\n");
}

// ── 主面板 ───────────────────────────────────────────────

export function SubAgentsPanel() {
  const [content, setContent] = useState("");
  const [frontmatter, setFrontmatter] = useState("");
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const savedFmRef = useRef("");

  const loadItems = useCallback(async (): Promise<ListItem[]> => {
    const agents = await listAgents();
    return agents.map((a) => ({
      id: a.path,
      label: a.name,
      description: a.description,
      badge: a.model || undefined,
      badgeColor: "#bc8cff",
    }));
  }, []);

  const handleSelect = useCallback(async (path: string | null) => {
    setSelectedName(path);
    setDirty(false);
    if (!path) {
      setContent("");
      setFrontmatter("");
      savedFmRef.current = "";
      return;
    }
    const data = await readAgent(path);
    setFrontmatter(data.frontmatter);
    setContent(data.content);
    savedFmRef.current = data.frontmatter;
  }, []);

  const handleSave = useCallback(async () => {
    if (!selectedName) return;
    // 内容未变则跳过写入
    if (frontmatter === savedFmRef.current) {
      setDirty(false);
      return;
    }
    setSaving(true);
    try {
      const full = frontmatter
        ? `---\n${frontmatter}\n---\n\n${content}`
        : content;
      await writeAgent(selectedName, full);
      savedFmRef.current = frontmatter;
      setDirty(false);
    } finally {
      setSaving(false);
    }
  }, [selectedName, frontmatter, content]);

  const handleDelete = useCallback(async (name: string) => {
    if (!confirm(`确认删除 Agent "${name}"？`)) return;
    await deleteAgent(name);
    if (selectedName === name) {
      setSelectedName(null);
      setContent("");
      setFrontmatter("");
    }
  }, [selectedName]);

  /** 修改 frontmatter 中某个字段的值（仅更新本地状态，不自动保存） */
  const handleFieldChange = useCallback((key: string, value: string) => {
    const newFm = updateFrontmatterLine(frontmatter, key, value);
    setFrontmatter(newFm);
    setDirty(newFm !== savedFmRef.current);
  }, [frontmatter]);

  return (
    <GenericThreeColumnPanel
      panelId="subagents-panel-v2"
      listTitle="子 Agent"
      loadItems={loadItems}
      searchable
      filterItem={(item, q) => item.label.toLowerCase().includes(q.toLowerCase())}
      onDelete={handleDelete}
      onSelect={handleSelect}
      renderContent={(id) =>
        id ? (
          <SimpleMarkdownEditor
            content={content}
            onChange={setContent}
            onSave={handleSave}
            saving={saving}
            placeholder="编辑 Agent 定义…"
          />
        ) : (
          <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">
            选择左侧 Agent 查看定义
          </div>
        )
      }
      renderSidebar={(id) =>
        id && frontmatter ? (
          <InteractiveMetadataSidebar
            frontmatter={frontmatter}
            onChange={handleFieldChange}
            dirty={dirty}
            saving={saving}
            onSave={handleSave}
          />
        ) : (
          <div className="p-4 text-[11px] text-[var(--text-muted)]">
            选择 Agent 查看元数据
          </div>
        )
      }
    />
  );
}

// ── 交互式元数据侧边栏 ───────────────────────────────────

function InteractiveMetadataSidebar({
  frontmatter,
  onChange,
  dirty,
  saving,
  onSave,
}: {
  frontmatter: string;
  onChange: (key: string, value: string) => void;
  dirty: boolean;
  saving: boolean;
  onSave: () => void;
}) {
  const items = parseFrontmatterItems(frontmatter);
  const { models: configuredModels } = useConfiguredModels();

  // 动态生成 model schema，复用 settings.json 已配置的模型列表
  const fields: FieldSchema[] = useMemo(() => {
    const modelOptions: FieldOption[] = [
      { value: "inherit", label: "inherit", description: "继承主会话模型" },
      { value: "fast", label: "fast", description: "使用 fastModel" },
      ...configuredModels.map((id) => ({ value: id, label: id })),
    ];
    return [
      { key: "model", label: "模型", options: modelOptions, allowCustom: true },
      ...INTERACTIVE_FIELDS.filter((f) => f.key !== "model"),
    ];
  }, [configuredModels]);

  return (
    <div className="p-4 space-y-3 overflow-auto h-full">
      <div className="flex items-center justify-between">
        <h3 className="text-[12px] font-medium text-[var(--text-muted)]">
          Agent 元数据
        </h3>
        {dirty && (
          <button
            onClick={onSave}
            disabled={saving}
            className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium
              bg-[var(--accent)] text-[var(--text-inverse)]
              hover:bg-[var(--accent-hover)] disabled:opacity-50 transition-colors"
          >
            {saving ? (
              <Check size={10} />
            ) : (
              <Save size={10} />
            )}
            {saving ? "保存中…" : "保存"}
          </button>
        )}
      </div>
      <div className="space-y-2.5">
        {items.map((item) => {
          const schema = fields.find((f) => f.key === item.key);
          if (schema) {
            return (
              <FieldSelect
                key={item.key}
                schema={schema}
                currentValue={item.value}
                onChange={(val) => onChange(item.key, val)}
              />
            );
          }
          return (
            <div key={item.key} className="space-y-0.5">
              <div className="text-[10px] text-[var(--text-muted)]">{item.key}</div>
              <div className="text-[12px] font-mono text-[var(--text-primary)] break-all">
                {item.value}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ── 单选字段组件 ─────────────────────────────────────────

function FieldSelect({
  schema,
  currentValue,
  onChange,
}: {
  schema: FieldSchema;
  currentValue: string;
  onChange: (value: string) => void;
}) {
  const allOptions = schema.options;
  const isKnown = allOptions.some((o) => o.value === currentValue);
  // 当前值不在预设列表中且不允许自定义 → 仍显示当前值
  const displayOptions = isKnown || !schema.allowCustom
    ? allOptions
    : [{ value: currentValue, label: currentValue, description: "自定义" }, ...allOptions];

  return (
    <div className="space-y-1">
      <div className="text-[10px] text-[var(--text-muted)]">{schema.label}</div>
      <div className="relative">
        <select
          value={currentValue}
          onChange={(e) => onChange(e.target.value)}
          className="w-full appearance-none bg-[var(--bg-input)] border border-[var(--border)] rounded-md
            px-2 py-1 pr-6 text-[11px] font-mono text-[var(--text-primary)]
            hover:border-[var(--border-strong)] focus:outline-none focus:border-[var(--accent)]
            cursor-pointer transition-colors"
        >
          {displayOptions.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}{opt.description ? ` — ${opt.description}` : ""}
            </option>
          ))}
        </select>
        <ChevronDown
          size={12}
          className="absolute right-1.5 top-1/2 -translate-y-1/2 text-[var(--text-muted)] pointer-events-none"
        />
      </div>
      {/* color 字段显示色点预览 */}
      {schema.key === "color" && COLOR_DOT[currentValue] && (
        <div className="flex items-center gap-1.5 mt-0.5">
          <div
            className="w-3 h-3 rounded-full border border-[var(--border)]"
            style={{ backgroundColor: COLOR_DOT[currentValue] }}
          />
          <span className="text-[9px] text-[var(--text-muted)]">{currentValue}</span>
        </div>
      )}
    </div>
  );
}

// ── frontmatter 解析 ─────────────────────────────────────

function parseFrontmatterItems(fm: string): { key: string; value: string }[] {
  const items: { key: string; value: string }[] = [];
  for (const line of fm.split("\n")) {
    const trimmed = line.trim();
    const colonIdx = trimmed.indexOf(":");
    if (colonIdx <= 0) continue;
    const key = trimmed.slice(0, colonIdx).trim();
    const val = trimmed.slice(colonIdx + 1).trim().replace(/^["']|["']$/g, "");
    if (key) {
      items.push({ key, value: val });
    }
  }
  return items;
}
