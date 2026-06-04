import { useState, useCallback } from "react";
import {
  GenericThreeColumnPanel,
  SimpleMarkdownEditor,
  MetadataSidebar,
  type ListItem,
} from "@/components/layout/GenericPanel";
import {
  listExtensions,
  readExtensionDetail,
  toggleExtension,
  deleteExtension,
  writeExtensionContext,
} from "@/lib/tauri";
import type { ExtensionInfo } from "@/lib/tauri";
import { Puzzle } from "lucide-react";

export function ExtensionsPanel() {
  const [detail, setDetail] = useState<{
    config: unknown;
    context: string | null;
    path: string;
  } | null>(null);
  const [extensions, setExtensions] = useState<ExtensionInfo[]>([]);
  const [contextContent, setContextContent] = useState("");
  const [saving, setSaving] = useState(false);
  const [selectedName, setSelectedName] = useState<string | null>(null);

  const loadItems = useCallback(async (): Promise<ListItem[]> => {
    const exts = await listExtensions();
    setExtensions(exts);
    return exts.map((e) => ({
      id: e.name,
      label: e.name,
      description: e.description,
      badge: e.version || undefined,
      badgeColor: e.enabled ? "#3fb950" : "var(--text-muted)",
      icon: (
        <Puzzle
          size={13}
          className={e.enabled ? "text-[#3fb950]" : "text-[var(--text-muted)]"}
        />
      ),
    }));
  }, []);

  const handleSelect = useCallback(async (name: string | null) => {
    setSelectedName(name);
    if (!name) {
      setDetail(null);
      setContextContent("");
      return;
    }
    const d = await readExtensionDetail(name);
    setDetail(d);
    setContextContent(d.context ?? "");
  }, []);

  const handleToggle = useCallback(
    async (name: string) => {
      const ext = extensions.find((e) => e.name === name);
      if (!ext) return;
      await toggleExtension(name, !ext.enabled);
      const exts = await listExtensions();
      setExtensions(exts);
    },
    [extensions],
  );

  const handleDelete = useCallback(async (name: string) => {
    if (!confirm(`确认删除扩展 "${name}"？`)) return;
    await deleteExtension(name);
    setDetail(null);
    setSelectedName(null);
    setContextContent("");
  }, []);

  const handleSaveContext = useCallback(async () => {
    if (!selectedName) return;
    setSaving(true);
    try {
      await writeExtensionContext(selectedName, contextContent);
    } finally {
      setSaving(false);
    }
  }, [selectedName, contextContent]);

  return (
    <GenericThreeColumnPanel
      panelId="extensions-panel-v2"
      listTitle="扩展"
      loadItems={loadItems}
      searchable
      filterItem={(item, q) =>
        item.label.toLowerCase().includes(q.toLowerCase())
      }
      onDelete={handleDelete}
      onSelect={handleSelect}
      renderContent={(id) => {
        if (!id || !detail)
          return (
            <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">
              选择左侧扩展查看详情
            </div>
          );
        const ext = extensions.find((e) => e.name === id);
        return (
          <div className="flex flex-col h-full">
            {/* 顶部标题栏 */}
            <div className="px-4 h-9 border-b border-[var(--border)] flex items-center justify-between shrink-0">
              <span className="text-[12px] text-[var(--text-primary)] font-medium">
                {id}
              </span>
              {ext && (
                <button
                  onClick={() => handleToggle(id)}
                  className={`px-2.5 h-6 text-[10px] rounded-md border transition-colors ${
                    ext.enabled
                      ? "border-[#3fb950]/40 bg-[#3fb950]/10 text-[#3fb950]"
                      : "border-[var(--border)] bg-[var(--bg-input)] text-[var(--text-muted)] hover:bg-[var(--bg-hover)]"
                  }`}
                >
                  {ext.enabled ? "已启用" : "已禁用"}
                </button>
              )}
            </div>

            {/* Tags */}
            {ext && (
              <div className="flex flex-wrap gap-1.5 px-4 pt-3 shrink-0">
                {ext.has_skills && <Tag label="Skills" color="#58a6ff" />}
                {ext.has_hooks && <Tag label="Hooks" color="#d29922" />}
                {ext.has_commands && <Tag label="Commands" color="#3fb950" />}
                {ext.has_agents && <Tag label="Agents" color="#bc8cff" />}
              </div>
            )}

            {/* 内容区 */}
            <div className="flex-1 min-h-0 overflow-auto">
              {detail.context ? (
                <SimpleMarkdownEditor
                  content={contextContent}
                  onChange={setContextContent}
                  onSave={handleSaveContext}
                  saving={saving}
                  placeholder="上下文注入文件…"
                />
              ) : (
                <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-[11px]">
                  无上下文注入文件
                </div>
              )}
            </div>
          </div>
        );
      }}
      renderSidebar={(id) => {
        if (!id || !detail)
          return (
            <div className="p-4 text-[11px] text-[var(--text-muted)]">
              选择扩展查看配置
            </div>
          );
        const cfg = detail.config as Record<string, unknown>;
        const items = Object.entries(cfg ?? {}).map(([k, v]) => ({
          label: k,
          value: typeof v === "string" ? v : String(v),
        }));
        return <MetadataSidebar title="扩展配置" items={items} />;
      }}
    />
  );
}

function Tag({ label, color }: { label: string; color: string }) {
  return (
    <span
      className="text-[10px] px-1.5 py-0.5 rounded"
      style={{ color, backgroundColor: `${color}1a` }}
    >
      {label}
    </span>
  );
}
