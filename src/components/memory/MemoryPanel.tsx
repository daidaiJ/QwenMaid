import { useState, useCallback } from "react";
import {
  GenericThreeColumnPanel,
  SimpleMarkdownEditor,
  MetadataSidebar,
  type ListItem,
} from "@/components/layout/GenericPanel";
import { getIndex, listMemories, readMemory, writeMemory, deleteMemory } from "@/lib/tauri";

export function MemoryPanel() {
  const [content, setContent] = useState("");
  const [frontmatter, setFrontmatter] = useState("");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const loadItems = useCallback(async (): Promise<ListItem[]> => {
    const index = await getIndex();
    const items: ListItem[] = [];

    // 全局记忆
    if (index.memories.length > 0) {
      items.push({
        id: "global-memory",
        label: "全局记忆",
        badge: "QWEN.md",
        badgeColor: "#58a6ff",
        isGroup: true,
        defaultExpanded: true,
      });
      for (const f of index.memories) {
        items.push({
          id: f.path,
          label: f.name,
          description: f.description,
          badge: f.memory_type,
          badgeColor: typeColors[f.memory_type] ?? "var(--text-muted)",
        });
      }
    }

    // 项目级记忆（只显示有记忆文件的项目，标记为分组）
    for (const p of index.projects) {
      if (p.memory_count === 0) continue;
      const { dirName, fullPath } = decodeProjectName(p.name);
      items.push({
        id: `project:${p.name}`,
        label: dirName,
        description: fullPath,
        badge: `${p.memory_count}`,
        badgeColor: "#bc8cff",
        isGroup: true,
      });
    }
    return items;
  }, []);

  const loadGroupChildren = useCallback(async (groupId: string): Promise<ListItem[]> => {
    const project = groupId.replace("project:", "");
    const files = await listMemories(project);
    return files.map((f) => ({
      id: f.path,
      label: f.name,
      description: f.description,
      badge: f.memory_type,
      badgeColor: typeColors[f.memory_type] ?? "var(--text-muted)",
    }));
  }, []);

  const handleSelect = useCallback(async (path: string | null) => {
    if (!path || path.startsWith("project:") || path === "global-memory") {
      setSelectedPath(null);
      setContent("");
      setFrontmatter("");
      return;
    }
    setSelectedPath(path);
    const data = await readMemory(path);
    setFrontmatter(data.frontmatter);
    setContent(data.content);
  }, []);

  const handleSave = useCallback(async () => {
    if (!selectedPath) return;
    setSaving(true);
    try {
      // 重组：frontmatter + content
      const full = frontmatter
        ? `---\n${frontmatter}\n---\n\n${content}`
        : content;
      await writeMemory(selectedPath, full);
    } finally {
      setSaving(false);
    }
  }, [selectedPath, frontmatter, content]);

  const handleDelete = useCallback(async (path: string) => {
    if (!confirm("确认删除此记忆文件？")) return;
    await deleteMemory(path);
    if (selectedPath === path) {
      setSelectedPath(null);
      setContent("");
      setFrontmatter("");
    }
  }, [selectedPath]);

  return (
    <GenericThreeColumnPanel
      panelId="memory-panel-v2"
      listTitle="记忆"
      loadItems={loadItems}
      loadGroupChildren={loadGroupChildren}
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
            placeholder="编辑记忆内容…"
          />
        ) : (
          <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">
            选择左侧记忆文件查看
          </div>
        )
      }
      renderSidebar={(id) =>
        id && frontmatter ? (
          <MetadataSidebar
            title="元数据"
            items={parseFrontmatterItems(frontmatter)}
          />
        ) : (
          <div className="p-4 text-[11px] text-[var(--text-muted)]">
            选择记忆文件查看元数据
          </div>
        )
      }
    />
  );
}

const typeColors: Record<string, string> = {
  user: "#58a6ff",
  feedback: "#d29922",
  project: "#3fb950",
  reference: "#bc8cff",
};

function parseFrontmatterItems(fm: string): { label: string; value: string; color?: string }[] {
  const items: { label: string; value: string; color?: string }[] = [];
  for (const line of fm.split("\n")) {
    const trimmed = line.trim();
    const colonIdx = trimmed.indexOf(":");
    if (colonIdx <= 0) continue;
    const key = trimmed.slice(0, colonIdx).trim();
    const val = trimmed.slice(colonIdx + 1).trim().replace(/^["']|["']$/g, "");
    if (key && val) {
      items.push({
        label: key,
        value: val,
        color: key === "type" ? typeColors[val] : undefined,
      });
    }
  }
  return items;
}

function decodeProjectName(encoded: string): { dirName: string; fullPath: string } {
  const fullPath = encoded.replace(/--/g, ":\\").replace(/-/g, "\\");
  const parts = fullPath.replace(/[:/\\]+$/, "").split(/[\\/]/);
  const dirName = parts[parts.length - 1] || fullPath;
  return { dirName, fullPath };
}
