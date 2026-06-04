import { useRef, useCallback } from "react";
import { Settings, Network, BarChart3, Puzzle, Search, BookOpen, Download, Package, MessageSquare, Bot } from "lucide-react";

export type PanelId =
  | "config"
  | "proxy"
  | "cost"
  | "extensions"
  | "skills"
  | "search"
  | "memory"
  | "sessions"
  | "subagents"
  | "install";

interface ActivityBarProps {
  active: PanelId;
  onSelect: (id: PanelId) => void;
  width?: number;
  onResize?: (w: number) => void;
}

const items: { id: PanelId; icon: typeof Settings; label: string }[] = [
  { id: "cost", icon: BarChart3, label: "成本" },
  { id: "config", icon: Settings, label: "配置" },
  { id: "proxy", icon: Network, label: "代理" },
  { id: "extensions", icon: Puzzle, label: "扩展" },
  { id: "skills", icon: Package, label: "技能" },
  { id: "search", icon: Search, label: "搜索" },
  { id: "memory", icon: BookOpen, label: "记忆" },
  { id: "sessions", icon: MessageSquare, label: "会话" },
  { id: "subagents", icon: Bot, label: "子 Agent" },
  { id: "install", icon: Download, label: "安装/更新" },
];

const MIN_W = 48;
const MAX_W = 80;

export function ActivityBar({ active, onSelect, width = 48, onResize }: ActivityBarProps) {
  const showLabel = width > 60;
  const dragRef = useRef<{ startX: number; startW: number } | null>(null);

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      dragRef.current = { startX: e.clientX, startW: width };
      const onMove = (ev: MouseEvent) => {
        if (!dragRef.current) return;
        const dx = ev.clientX - dragRef.current.startX;
        const newW = Math.max(MIN_W, Math.min(MAX_W, dragRef.current.startW + dx));
        onResize?.(newW);
      };
      const onUp = () => {
        dragRef.current = null;
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
      };
      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    },
    [width, onResize],
  );

  return (
    <nav
      className="flex flex-col bg-[var(--bg-sidebar)] border-r border-[var(--border)] py-2 shrink-0 shadow-sm relative select-none"
      style={{ width }}
    >
      {items.map(({ id, icon: Icon, label }) => (
        <button
          key={id}
          title={label}
          onClick={() => onSelect(id)}
          className={`flex items-center gap-2 mx-1 px-1 h-10 rounded-md transition-all duration-150 ${
            active === id
              ? "bg-[var(--accent-light)] text-[var(--accent)] shadow-[var(--shadow-sm)]"
              : "text-[var(--text-muted)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
          } ${showLabel ? "justify-start" : "justify-center"}`}
        >
          <Icon size={19} strokeWidth={1.5} className="shrink-0" />
          {showLabel && (
            <span className="text-xs truncate">{label}</span>
          )}
        </button>
      ))}

      {/* 右边缘拖拽条 */}
      <div
        onMouseDown={onMouseDown}
        className="absolute top-0 right-0 w-1 h-full cursor-col-resize hover:bg-[var(--accent)]/30 transition-colors z-10"
      />
    </nav>
  );
}
