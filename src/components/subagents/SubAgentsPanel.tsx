import { useState, useCallback } from "react";
import {
  GenericThreeColumnPanel,
  SimpleMarkdownEditor,
  MetadataSidebar,
  type ListItem,
} from "@/components/layout/GenericPanel";
import { listAgents, readAgent, writeAgent, deleteAgent } from "@/lib/tauri";

export function SubAgentsPanel() {
  const [content, setContent] = useState("");
  const [frontmatter, setFrontmatter] = useState("");
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const loadItems = useCallback(async (): Promise<ListItem[]> => {
    const agents = await listAgents();
    return agents.map((a) => ({
      id: a.name,
      label: a.name,
      description: a.description,
      badge: a.model || undefined,
      badgeColor: "#bc8cff",
    }));
  }, []);

  const handleSelect = useCallback(async (name: string | null) => {
    setSelectedName(name);
    if (!name) {
      setContent("");
      setFrontmatter("");
      return;
    }
    const data = await readAgent(name);
    setFrontmatter(data.frontmatter);
    setContent(data.content);
  }, []);

  const handleSave = useCallback(async () => {
    if (!selectedName) return;
    setSaving(true);
    try {
      const full = frontmatter
        ? `---\n${frontmatter}\n---\n\n${content}`
        : content;
      await writeAgent(selectedName, full);
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
          <MetadataSidebar
            title="Agent 元数据"
            items={parseFrontmatterItems(frontmatter)}
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

function parseFrontmatterItems(fm: string): { label: string; value: string; color?: string }[] {
  const items: { label: string; value: string; color?: string }[] = [];
  for (const line of fm.split("\n")) {
    const trimmed = line.trim();
    const colonIdx = trimmed.indexOf(":");
    if (colonIdx <= 0) continue;
    const key = trimmed.slice(0, colonIdx).trim();
    const val = trimmed.slice(colonIdx + 1).trim().replace(/^["']|["']$/g, "");
    if (key && val) {
      items.push({ label: key, value: val });
    }
  }
  return items;
}
