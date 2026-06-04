import { useState, useCallback } from "react";
import {
  GenericThreeColumnPanel,
  SimpleMarkdownEditor,
  MetadataSidebar,
  type ListItem,
} from "@/components/layout/GenericPanel";
import { listSkills, readSkillContent, writeSkill, deleteSkill } from "@/lib/tauri";
import { Package } from "lucide-react";

export function SkillsPanel() {
  const [content, setContent] = useState("");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const loadItems = useCallback(async (): Promise<ListItem[]> => {
    const skills = await listSkills();
    return skills.map((s) => ({
      id: s.path,
      label: s.name,
      description: s.description,
      badge: s.source.startsWith("ext:") ? "扩展" : "用户",
      badgeColor: s.source.startsWith("ext:") ? "#bc8cff" : "#58a6ff",
      icon: <Package size={13} className="text-[var(--text-muted)]" />,
    }));
  }, []);

  const handleSelect = useCallback(async (path: string | null) => {
    setSelectedPath(path);
    if (!path) {
      setContent("");
      return;
    }
    const md = await readSkillContent(path);
    setContent(md);
  }, []);

  const handleSave = useCallback(async () => {
    if (!selectedPath) return;
    setSaving(true);
    try {
      await writeSkill(selectedPath, content);
    } finally {
      setSaving(false);
    }
  }, [selectedPath, content]);

  const handleDelete = useCallback(
    async (path: string) => {
      if (!confirm("确认删除此技能？")) return;
      await deleteSkill(path);
      if (selectedPath === path) {
        setSelectedPath(null);
        setContent("");
      }
    },
    [selectedPath],
  );

  return (
    <GenericThreeColumnPanel
      panelId="skills-panel-v2"
      listTitle="技能"
      loadItems={loadItems}
      searchable
      filterItem={(item, q) =>
        item.label.toLowerCase().includes(q.toLowerCase())
      }
      onDelete={handleDelete}
      onSelect={handleSelect}
      renderContent={(id) =>
        id ? (
          <SimpleMarkdownEditor
            content={content}
            onChange={setContent}
            onSave={handleSave}
            saving={saving}
            placeholder="SKILL.md 内容…"
          />
        ) : (
          <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">
            选择左侧技能查看 SKILL.md
          </div>
        )
      }
      renderSidebar={(id) => {
        if (!id)
          return (
            <div className="p-4 text-[11px] text-[var(--text-muted)]">
              选择技能查看元数据
            </div>
          );
        const items = parseSkillFrontmatter(content);
        return <MetadataSidebar title="技能元数据" items={items} />;
      }}
    />
  );
}

function parseSkillFrontmatter(content: string): { label: string; value: string }[] {
  const items: { label: string; value: string }[] = [];
  if (!content.startsWith("---")) return items;
  const end = content.indexOf("---", 3);
  if (end < 0) return items;
  const fm = content.slice(3, end);
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
