import { useState, useEffect, useCallback, type ReactNode } from "react";
import { ResizableColumns } from "@/components/layout/ResizableColumns";
import { Search, Plus, Trash2, RefreshCw, Loader2, Eye, Pencil, ChevronRight } from "lucide-react";
import ReactMarkdown from "react-markdown";

// ── 通用列表项 ───────────────────────────────────────────

export interface ListItem {
  id: string;
  label: string;
  description?: string;
  badge?: string;
  badgeColor?: string;
  icon?: ReactNode;
  /** 是否是分组标题项（可展开/折叠） */
  isGroup?: boolean;
  /** 是否默认展开（仅对 isGroup=true 有效） */
  defaultExpanded?: boolean;
  /** 分组层级缩进（0=分组标题, 1=子项） */
  level?: number;
}

// ── 通用三栏面板属性 ─────────────────────────────────────

export interface GenericThreeColumnPanelProps {
  /** 面板唯一标识（用于 localStorage 持久化宽度） */
  panelId: string;
  /** 左栏标题 */
  listTitle: string;
  /** 加载列表数据（分组标题项） */
  loadItems: () => Promise<ListItem[]>;
  /** 异步加载分组子项（点击分组标题时调用） */
  loadGroupChildren?: (groupId: string) => Promise<ListItem[]>;
  /** 渲染中栏内容 */
  renderContent: (selectedId: string | null) => ReactNode;
  /** 渲染右栏内容 */
  renderSidebar: (selectedId: string | null) => ReactNode;
  /** 左栏底部额外操作区 */
  listFooter?: ReactNode;
  /** 是否支持搜索 */
  searchable?: boolean;
  /** 搜索过滤函数 */
  filterItem?: (item: ListItem, query: string) => boolean;
  /** 新建按钮回调 */
  onAdd?: () => void;
  /** 删除按钮回调 */
  onDelete?: (id: string) => void;
  /** 选中项变化回调 */
  onSelect?: (id: string | null) => void;
  /** 列表项点击拦截器，返回 false 可阻止默认行为（选中/展开） */
  onItemClick?: (itemId: string) => boolean;
  /** 设置此值可触发指定分组重新加载（折叠后重新展开）；值变化时触发 */
  reloadGroupId?: string | null;
  /** 值变化时触发整个列表重新加载（用于后台数据更新后刷新） */
  refreshKey?: number;
}

// ── 通用三栏面板 ─────────────────────────────────────────

export function GenericThreeColumnPanel({
  panelId,
  listTitle,
  loadItems,
  loadGroupChildren,
  renderContent,
  renderSidebar,
  listFooter,
  searchable = false,
  filterItem,
  onAdd,
  onDelete,
  onSelect,
  onItemClick,
  reloadGroupId,
  refreshKey,
}: GenericThreeColumnPanelProps) {
  const [items, setItems] = useState<ListItem[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [groupLoading, setGroupLoading] = useState<Set<string>>(new Set());

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const data = await loadItems();
      setItems(data);
      // 默认展开标记了 defaultExpanded 的分组
      const defaults = new Set(data.filter((i) => i.isGroup && i.defaultExpanded).map((i) => i.id));
      setExpandedGroups(defaults);
    } finally {
      setLoading(false);
    }
  }, [loadItems]);

  useEffect(() => { refresh(); }, [refresh]);

  // refreshKey 变化时触发重新加载
  useEffect(() => { if (refreshKey !== undefined) refresh(); }, [refreshKey]);

  const handleSelect = (id: string) => {
    setSelected(id);
    const rawId = id.includes("::") ? id.split("::").slice(1).join("::") : id;
    onSelect?.(rawId);
  };

  const toggleGroup = async (groupId: string) => {
    if (!loadGroupChildren) return;
    const wasExpanded = expandedGroups.has(groupId);
    if (wasExpanded) {
      setExpandedGroups((prev) => { const n = new Set(prev); n.delete(groupId); return n; });
      setItems((prev) => prev.filter((i) => !(i.level && i.id.startsWith(`${groupId}::`))));
      return;
    }

    setGroupLoading((prev) => new Set(prev).add(groupId));
    try {
      const children = await loadGroupChildren(groupId);
      setItems((prev) => {
        const nextItems = [...prev];
        const groupIdx = nextItems.findIndex((i) => i.id === groupId);
        if (groupIdx !== -1) {
          const indented = children.map((c) => ({ ...c, level: 1, id: `${groupId}::${c.id}` }));
          nextItems.splice(groupIdx + 1, 0, ...indented);
        }
        return nextItems;
      });
      setExpandedGroups((prev) => new Set(prev).add(groupId));
    } finally {
      setGroupLoading((prev) => {
        const n = new Set(prev);
        n.delete(groupId);
        return n;
      });
    }
  };

  // 当 reloadGroupId 变化时，折叠再展开该分组以触发 loadGroupChildren 刷新
  useEffect(() => {
    if (!reloadGroupId || !expandedGroups.has(reloadGroupId)) return;
    toggleGroup(reloadGroupId); // 折叠
    const t = setTimeout(() => toggleGroup(reloadGroupId), 30); // 展开
    return () => clearTimeout(t);
  }, [reloadGroupId]);

  const filtered = searchQuery && filterItem
    ? items.filter((item) => filterItem(item, searchQuery))
    : items;

  return (
    <ResizableColumns
      autoSaveId={panelId}
      left={{
        defaultSize: 18,
        minSize: 12,
        maxSize: 30,
        className: "bg-[var(--bg-sidebar)] flex flex-col",
        children: (
          <>
            {/* 标题栏 */}
            <div className="flex items-center justify-between px-3 h-10 border-b border-[var(--border)]">
              <span className="text-[12px] font-medium text-[var(--text-muted)]">
                {listTitle}
              </span>
              <div className="flex items-center gap-1">
                {onAdd && (
                  <button
                    onClick={onAdd}
                    className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--bg-input)] text-[var(--text-muted)] hover:text-[var(--text-primary)]"
                    title="新建"
                  >
                    <Plus size={14} />
                  </button>
                )}
                <button
                  onClick={refresh}
                  className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--bg-input)] text-[var(--text-muted)] hover:text-[var(--text-primary)]"
                  title="刷新"
                >
                  <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
                </button>
              </div>
            </div>

            {/* 搜索框 */}
            {searchable && (
              <div className="px-2 py-1.5 border-b border-[var(--border)]">
                <div className="flex items-center gap-1.5 h-7 bg-[var(--bg-input)] border border-[var(--border)] rounded-md px-2">
                  <Search size={12} className="text-[var(--text-muted)] shrink-0" />
                  <input
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder="搜索…"
                    className="flex-1 bg-transparent text-[11px] text-[var(--text-primary)] placeholder:text-[var(--text-muted)] outline-none"
                  />
                </div>
              </div>
            )}

            {/* 列表 */}
            <div className="flex-1 overflow-auto py-0.5">
              {loading ? (
                // 骨架屏：5 行占位条，不阻塞中栏/右栏渲染
                Array.from({ length: 5 }).map((_, i) => (
                  <div key={i} className="px-3 py-2">
                    <div className="h-3 bg-[var(--border)] rounded animate-pulse mb-1.5" style={{ width: `${60 + (i % 3) * 12}%` }} />
                    <div className="h-2 bg-[var(--border)] rounded animate-pulse opacity-50" style={{ width: `${30 + (i % 4) * 10}%` }} />
                  </div>
                ))
              ) : filtered.length === 0 ? (
                <p className="px-3 py-4 text-xs text-[var(--text-muted)]">
                  {searchQuery ? "无匹配结果" : "暂无数据"}
                </p>
              ) : (
                filtered.map((item) => {
                  const isGroup = !item.level && (item.isGroup ?? item.id.startsWith("project:") ?? item.id === "global-memory");
                  const isExpanded = expandedGroups.has(item.id);
                  const isLoading = groupLoading.has(item.id);
                  const paddingLeft = item.level ? `${12 + item.level * 16}px` : undefined;

                  return (
                    <div
                      key={item.id}
                      className={`group flex items-center gap-2 px-3 h-9 text-[13px] transition-colors cursor-pointer ${
                        selected === item.id
                          ? "bg-[var(--accent-light)] text-[var(--accent)] font-medium"
                          : isGroup
                          ? "text-[var(--text-primary)] hover:bg-[var(--bg-hover)] font-medium"
                          : "text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
                      }`}
                      style={paddingLeft ? { paddingLeft } : undefined}
                      onClick={() => {
                        if (onItemClick && onItemClick(item.id) === false) return;
                        if (isGroup && loadGroupChildren && !searchQuery) {
                          toggleGroup(item.id);
                        } else {
                          handleSelect(item.id);
                        }
                      }}
                    >
                      {isGroup && loadGroupChildren && !searchQuery && (
                        <span className={`shrink-0 text-[var(--text-muted)] transition-transform duration-150 ${isExpanded ? "rotate-90" : ""}`}>
                          {isLoading ? <Loader2 size={12} className="animate-spin" /> : <ChevronRight size={12} />}
                        </span>
                      )}
                      {item.icon && <span className="shrink-0">{item.icon}</span>}
                      <span className="truncate flex-1">{item.label}</span>
                      {item.badge && (
                        <span
                          className="text-[10px] px-1.5 py-0.5 rounded-md shrink-0"
                          style={{
                            color: item.badgeColor ?? "var(--text-muted)",
                            backgroundColor: `${item.badgeColor ?? "var(--text-muted)"}1a`,
                          }}
                        >
                          {item.badge}
                        </span>
                      )}
                      {onDelete && !isGroup && (
                        <button
                          onClick={(e) => { e.stopPropagation(); onDelete(item.id); }}
                          className="shrink-0 text-[var(--text-muted)] hover:text-[var(--color-error)] opacity-0 group-hover:opacity-100 transition-opacity"
                          title="删除"
                        >
                          <Trash2 size={12} />
                        </button>
                      )}
                    </div>
                  );
                })
              )}
            </div>

            {/* 底部操作区 */}
            {listFooter && (
              <div className="border-t border-[var(--border)] p-2">
                {listFooter}
              </div>
            )}
          </>
        ),
      }}
      center={{
        className: "flex flex-col overflow-auto",
        children: renderContent(selected),
      }}
      right={{
        defaultSize: 25,
        minSize: 15,
        maxSize: 40,
        collapsible: true,
        className: "flex flex-col overflow-auto",
        children: renderSidebar(selected),
      }}
    />
  );
}

// ── 通用 Markdown 编辑器（简单版） ───────────────────────

export function SimpleMarkdownEditor({
  content,
  onChange,
  onSave,
  saving,
  placeholder,
}: {
  content: string;
  onChange: (v: string) => void;
  onSave: () => void;
  saving?: boolean;
  placeholder?: string;
}) {
  const [preview, setPreview] = useState(true);

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-4 h-9 border-b border-[var(--border)]">
        <div className="flex items-center gap-2">
          <button
            onClick={() => setPreview(false)}
            className={`flex items-center gap-1 px-2 h-6 text-[11px] rounded-sm transition-colors ${
              !preview
                ? "bg-[var(--accent)] text-white"
                : "text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
            }`}
          >
            <Pencil size={11} />
            编辑
          </button>
          <button
            onClick={() => setPreview(true)}
            className={`flex items-center gap-1 px-2 h-6 text-[11px] rounded-sm transition-colors ${
              preview
                ? "bg-[var(--accent)] text-white"
                : "text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
            }`}
          >
            <Eye size={11} />
            预览
          </button>
        </div>
        {!preview && (
          <button
            onClick={onSave}
            disabled={saving}
            className="px-3 h-6 text-[11px] bg-[var(--accent)] text-white rounded-sm hover:bg-[var(--accent-hover)] disabled:opacity-40 transition-colors"
          >
            {saving ? "保存中…" : "保存"}
          </button>
        )}
      </div>
      {preview ? (
        <div className="flex-1 overflow-auto p-4 text-[13px] text-[var(--text-primary)] leading-relaxed">
          <ReactMarkdown>{content}</ReactMarkdown>
        </div>
      ) : (
        <textarea
          value={content}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className="flex-1 resize-none bg-transparent text-[12px] font-mono text-[var(--text-primary)] placeholder:text-[var(--text-muted)] p-4 outline-none"
        />
      )}
    </div>
  );
}

// ── 通用元数据展示 ───────────────────────────────────────

export function MetadataSidebar({
  title,
  items,
}: {
  title: string;
  items: { label: string; value: string; color?: string }[];
}) {
  return (
    <div className="p-4 space-y-3">
      <h3 className="text-[12px] font-medium text-[var(--text-muted)]">
        {title}
      </h3>
      <div className="space-y-2">
        {items.map((item, i) => (
          <div key={i} className="space-y-0.5">
            <div className="text-[10px] text-[var(--text-muted)]">{item.label}</div>
            <div
              className="text-[12px] font-mono"
              style={{ color: item.color ?? "var(--text-primary)" }}
            >
              {item.value}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Frontmatter 感知的 Markdown 渲染器 ───────────────────

export function FrontmatterMarkdown({ content }: { content: string }) {
  const { frontmatter, body } = splitFrontmatter(content);

  return (
    <div className="space-y-3">
      {frontmatter.length > 0 && (
        <div className="rounded-md border border-[var(--border)] bg-[var(--bg-sidebar)] px-3 py-2">
          <div className="flex flex-wrap gap-x-4 gap-y-1">
            {frontmatter.map((item, i) => (
              <div key={i} className="flex items-baseline gap-1.5 min-w-0">
                <span className="text-[10px] font-medium text-[var(--text-muted)] uppercase shrink-0">
                  {item.key}
                </span>
                <span className="text-[11px] text-[var(--text-primary)] truncate">
                  {item.value}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
      {body.trim() && (
        <div className="markdown-body text-[12px] text-[var(--text-primary)] leading-relaxed">
          <ReactMarkdown>{body}</ReactMarkdown>
        </div>
      )}
    </div>
  );
}

function splitFrontmatter(content: string): {
  frontmatter: { key: string; value: string }[];
  body: string;
} {
  if (!content.startsWith("---")) return { frontmatter: [], body: content };
  const end = content.indexOf("\n---", 3);
  if (end < 0) return { frontmatter: [], body: content };

  const fmBlock = content.slice(3, end).trim();
  const body = content.slice(end + 4);
  const items: { key: string; value: string }[] = [];

  for (const line of fmBlock.split("\n")) {
    const colonIdx = line.indexOf(":");
    if (colonIdx <= 0) continue;
    const key = line.slice(0, colonIdx).trim();
    const val = line.slice(colonIdx + 1).trim().replace(/^["']|["']$/g, "");
    if (key && val) items.push({ key, value: val });
  }

  return { frontmatter: items, body };
}
