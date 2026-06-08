import { useState, useCallback, useRef, useEffect } from "react";
import ReactMarkdown from "react-markdown";
import {
  GenericThreeColumnPanel,
  MetadataSidebar,
  type ListItem,
} from "@/components/layout/GenericPanel";
import { getIndex, listSessions, getSessionDetail, getSessionMessagesPaged } from "@/lib/tauri";
import type { SessionDetail, SessionMessage, ToolCallStat } from "@/lib/tauri";
import { listen } from "@tauri-apps/api/event";
import {
  User,
  Bot,
  Settings,
  ChevronDown,
  ChevronRight,
  Wrench,
  Brain,
  Zap,
} from "lucide-react";

export function SessionsPanel() {
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [messages, setMessages] = useState<SessionMessage[]>([]);
  const [msgTotal, setMsgTotal] = useState(0);
  const [hasOlder, setHasOlder] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const msgOffsetRef = useRef(0);
  const [selectedSession, setSelectedSession] = useState<{
    project: string;
    id: string;
  } | null>(null);
  const [loading, setLoading] = useState(false);
  const loadedCountsRef = useRef<Record<string, number>>({});
  const [reloadGroupId, setReloadGroupId] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  // 后台同步完成后自动刷新列表
  useEffect(() => {
    const unlisten = listen("stats-synced", () => {
      setRefreshKey((k) => k + 1);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // 后台同步完成后，如果当前有选中会话，刷新其统计信息
  useEffect(() => {
    if (selectedSession && detail) {
      getSessionDetail(selectedSession.project, selectedSession.id)
        .then(setDetail)
        .catch(() => {});
    }
  }, [refreshKey]);

  const loadItems = useCallback(async (): Promise<ListItem[]> => {
    const index = await getIndex(20);
    const items: ListItem[] = [];
    for (const p of index.projects) {
      if (p.session_count === 0 && p.memory_count === 0) continue;
      const { dirName, fullPath } = decodeProjectName(p.name);
      items.push({
        id: `project:${p.name}`,
        label: dirName,
        description: fullPath,
        badge: `${p.valid_session_count}`,
        badgeColor: "#58a6ff",
        isGroup: true,
      });
    }
    return items;
  }, []);

  const loadGroupChildren = useCallback(
    async (groupId: string): Promise<ListItem[]> => {
      const project = groupId.replace("project:", "");
      const count = loadedCountsRef.current[groupId] ?? 30;
      // 后端 limit，只扫描需要的文件数
      const sessions = await listSessions(project, count);

      // 隐藏 input_tokens=0 的会话（未同步或无数据）
      const validSessions = sessions.filter((s) => s.input_tokens > 0);

      const items: ListItem[] = validSessions.map((s) => ({
        id: `session:${project}:${s.id}`,
        label: s.title || s.id.slice(0, 8),
        description: formatRelativeTime(s.started_at),
        badge: `~${s.message_count}`,
        badgeColor: "var(--text-muted)",
      }));

      // 追加"加载更多"伪项（用 count 估算剩余，避免全量扫描）
      if (sessions.length >= count) {
        items.push({
          id: `__load_more_${groupId}`,
          label: `加载更多`,
          badge: "…",
          badgeColor: "var(--text-muted)",
        });
      }
      return items;
    },
    [],
  );

  const handleItemClick = useCallback((itemId: string): boolean => {
    // 拦截"加载更多"伪项点击
    // GenericPanel 会将子项 ID 前缀为 `${groupId}::`
    const loadMoreMatch = itemId.match(/__load_more_(project:.+)$/);
    if (!loadMoreMatch) return true; // 非加载更多，放行
    const groupId = loadMoreMatch[1];

    // 增加已加载数量，再通过 reloadGroupId 触发 GenericPanel 折叠+展开刷新
    loadedCountsRef.current[groupId] = (loadedCountsRef.current[groupId] ?? 30) + 30;
    setReloadGroupId(groupId);
    return false; // 阻止默认行为
  }, []);

  // reloadGroupId 触发后需重置，以便下次再触发同一分组时能再次变化
  const prevReloadRef = useRef<string | null>(null);
  if (reloadGroupId !== prevReloadRef.current) {
    requestAnimationFrame(() => setReloadGroupId(null));
    prevReloadRef.current = reloadGroupId;
  }

  const handleSelect = useCallback(
    async (id: string | null) => {
      if (!id || !id.startsWith("session:")) {
        setSelectedSession(null);
        setDetail(null);
        setMessages([]);
        return;
      }
      const parts = id.split(":");
      const project = parts[1];
      const sessionId = parts.slice(2).join(":");
      setSelectedSession({ project, id: sessionId });
      setLoading(true);
      setMessages([]);
      setDetail(null);

      // Phase 1: 先加载消息（用户立刻看到内容）
      try {
        const paged = await getSessionMessagesPaged(project, sessionId, 0, 50);
        setMessages(paged.messages);
        setMsgTotal(paged.total_count);
        setHasOlder(paged.has_older);
        msgOffsetRef.current = 50;
      } catch {
        setMessages([]);
      } finally {
        setLoading(false);
      }

      // Phase 2: 统计信息延迟填充（不阻塞消息展示）
      getSessionDetail(project, sessionId).then(setDetail).catch(() => {});
    },
    [],
  );

  const loadOlder = useCallback(async () => {
    if (!selectedSession || loadingOlder || !hasOlder) return;
    setLoadingOlder(true);
    try {
      const paged = await getSessionMessagesPaged(
        selectedSession.project,
        selectedSession.id,
        msgOffsetRef.current,
        50,
      );
      setMessages((prev) => [...paged.messages, ...prev]);
      setHasOlder(paged.has_older);
      msgOffsetRef.current += paged.messages.length;
    } finally {
      setLoadingOlder(false);
    }
  }, [selectedSession, loadingOlder, hasOlder]);

  return (
    <GenericThreeColumnPanel
      panelId="sessions-panel-v3"
      listTitle="会话"
      loadItems={loadItems}
      loadGroupChildren={loadGroupChildren}
      searchable
      filterItem={(item, q) =>
        item.label.toLowerCase().includes(q.toLowerCase())
      }
      onSelect={handleSelect}
      onItemClick={handleItemClick}
      reloadGroupId={reloadGroupId}
      refreshKey={refreshKey}
      renderContent={() => {
        if (loading) {
          return (
            <div className="flex items-center justify-center h-full">
              <div className="text-[var(--text-muted)] text-sm">加载中…</div>
            </div>
          );
        }
        if (!detail || !selectedSession) {
          return (
            <div className="flex flex-col items-center justify-center h-full gap-2 text-[var(--text-muted)]">
              <Zap size={24} className="opacity-30" />
              <span className="text-sm">选择左侧会话查看消息</span>
            </div>
          );
        }
        return (
          <div className="flex flex-col h-full">
            {/* 滚动到顶部时自动加载更早消息 */}
            {hasOlder && (
              <LoadMoreSentinel
                loading={loadingOlder}
                loaded={messages.length}
                total={msgTotal}
                onLoadMore={loadOlder}
              />
            )}
            <MessageList messages={messages} />
          </div>
        );
      }}
      renderSidebar={() => {
        if (!detail || !selectedSession) {
          return (
            <div className="p-4 text-[11px] text-[var(--text-muted)]">
              选择会话查看统计
            </div>
          );
        }
        return (
          <div className="p-4 space-y-4">
            <MetadataSidebar
              title="会话统计"
              items={[
                { label: "消息数", value: `${detail.message_count}` },
                {
                  label: "输入 Token",
                  value: detail.input_tokens.toLocaleString(),
                },
                {
                  label: "输出 Token",
                  value: detail.output_tokens.toLocaleString(),
                },
                { label: "模型", value: detail.models || "—" },
                { label: "时长", value: detail.duration || "—" },
              ]}
            />
            {detail.tool_calls?.length > 0 && (
              <ToolCallStats toolCalls={detail.tool_calls} title="工具调用" />
            )}
            {/* 技能 + 子智能体调用统计 */}
            <div className="px-4 space-y-2">
              <h4 className="text-xs font-semibold text-[var(--text-muted)]">
                技能 & 子智能体
              </h4>
              <div className="flex gap-3">
                <div className="flex-1 rounded-lg bg-[var(--bg-card)] shadow-[var(--shadow-card)] p-2 text-center">
                  <div className="text-[10px] text-[var(--text-muted)]">技能调用</div>
                  <div className="text-[16px] font-mono font-semibold" style={{ color: "#bc8cff" }}>
                    {detail.skill_calls?.reduce((s, c) => s + c.count, 0) ?? 0}
                  </div>
                  {detail.skill_calls?.map((sc) => (
                    <div key={sc.name} className="text-[9px] text-[var(--text-muted)] truncate">{sc.name} ×{sc.count}</div>
                  ))}
                </div>
                <div className="flex-1 rounded-lg bg-[var(--bg-card)] shadow-[var(--shadow-card)] p-2 text-center">
                  <div className="text-[10px] text-[var(--text-muted)]">子智能体</div>
                  <div className="text-[16px] font-mono font-semibold" style={{ color: "#58a6ff" }}>
                    {detail.agent_calls?.reduce((s, c) => s + c.count, 0) ?? 0}
                  </div>
                  {detail.agent_calls?.map((ac) => (
                    <div key={ac.name} className="text-[9px] text-[var(--text-muted)] truncate">{ac.name} ×{ac.count}</div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        );
      }}
    />
  );
}

// ── 滚动触底自动加载 ─────────────────────────────────────

function LoadMoreSentinel({
  loading, loaded, total, onLoadMore,
}: {
  loading: boolean; loaded: number; total: number; onLoadMore: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting && !loading) {
          onLoadMore();
        }
      },
      { rootMargin: "200px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [loading, onLoadMore]);

  return (
    <div ref={ref} className="flex justify-center py-2 border-b border-[var(--border)]">
      <span className="text-[11px] text-[var(--text-muted)]">
        {loading ? "加载中…" : `已加载 ${loaded}/${total}`}
      </span>
    </div>
  );
}

// ── 消息列表 ─────────────────────────────────────────────

function MessageList({ messages }: { messages: SessionMessage[] }) {
  // 过滤掉空消息和纯 tool_result 消息
  const visible = messages.filter(
    (m) =>
      m.text ||
      m.thinking ||
      (m.msg_type !== "system" && m.has_tool_use),
  );

  return (
    <div className="flex flex-col h-full overflow-auto p-4 gap-3">
      {visible.map((msg, i) => (
        <MessageBubble key={msg.uuid || i} msg={msg} />
      ))}
    </div>
  );
}

// ── 消息气泡 ─────────────────────────────────────────────

function MessageBubble({ msg }: { msg: SessionMessage }) {
  const isUser = msg.msg_type === "user";
  const isAssistant = msg.msg_type === "assistant";
  const isSystem = msg.msg_type === "system";

  return (
    <div className={`flex gap-2 ${isUser ? "justify-end" : "justify-start"}`}>
      <div
        className={`max-w-[85%] rounded-xl overflow-hidden shadow-[var(--shadow-card)] ${
          isUser
            ? "border-t-[2px] border-t-[#58a6ff]"
            : isSystem
              ? "border-t-[2px] border-t-[var(--text-muted)]"
              : "border-t-[2px] border-t-[#bc8cff]"
        }`}
      >
        {/* 头部 */}
        <div className="flex items-center gap-1.5 px-3 pt-2 pb-1">
          <div
            className={`w-5 h-5 rounded-full flex items-center justify-center ${
              isUser
                ? "bg-[#58a6ff]/20"
                : isSystem
                  ? "bg-[var(--bg-input)]"
                  : "bg-[#bc8cff]/20"
            }`}
          >
            {isUser ? (
              <User size={11} className="text-[#58a6ff]" />
            ) : isAssistant ? (
              <Bot size={11} className="text-[#bc8cff]" />
            ) : (
              <Settings size={11} className="text-[var(--text-muted)]" />
            )}
          </div>
          <span className="text-[11px] font-medium text-[var(--text-primary)]">
            {isUser ? "用户" : isAssistant ? "AI" : "系统"}
          </span>
          {msg.model && (
            <span className="text-[9px] font-mono text-[var(--text-muted)] ml-auto">
              {shortModelName(msg.model)}
            </span>
          )}
          {(msg.input_tokens > 0 || msg.output_tokens > 0) && (
            <span className="text-[9px] font-mono text-[var(--text-muted)]">
              {msg.input_tokens > 0 && `in:${formatTokens(msg.input_tokens)}`}
              {msg.output_tokens > 0 &&
                ` out:${formatTokens(msg.output_tokens)}`}
            </span>
          )}
        </div>

        {/* Thinking 块 */}
        {msg.thinking && <ThinkingBlock content={msg.thinking} />}

        {/* 工具调用 */}
        {msg.has_tool_use && msg.tool_name && (
          <ToolCallInline
            name={msg.tool_name}
            preview={msg.tool_input_preview}
          />
        )}

        {/* 文本内容 */}
        {msg.text && (
          <div className="px-3 pb-2 text-[12px] text-[var(--text-primary)] leading-relaxed">
            <div className="markdown-body">
              <ReactMarkdown>{msg.text}</ReactMarkdown>
            </div>
          </div>
        )}

        {/* 时间戳 */}
        {msg.timestamp && (
          <div className="px-3 pb-1.5">
            <span className="text-[9px] text-[var(--text-muted)]">
              {formatTime(msg.timestamp)}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Thinking 折叠块 ──────────────────────────────────────

function ThinkingBlock({ content }: { content: string }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="mx-3 mb-1.5 rounded-lg border border-[#bc8cff]/20 bg-[#bc8cff08]">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 w-full px-2 py-1 text-[10px] text-[#bc8cff] hover:bg-[#bc8cff10] transition-colors"
      >
        {open ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
        <Brain size={10} />
        <span>Thinking</span>
        <span className="text-[var(--text-muted)] ml-auto">
          {content.length} chars
        </span>
      </button>
      {open && (
        <div className="px-2 pb-2 text-[11px] text-[var(--text-muted)] italic whitespace-pre-wrap break-words max-h-60 overflow-auto">
          {content}
        </div>
      )}
    </div>
  );
}

// ── 工具调用行 ───────────────────────────────────────────

function ToolCallInline({
  name,
  preview,
}: {
  name: string;
  preview: string | null;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div className="mx-3 mb-1.5 rounded-lg border border-[#d29922]/20 bg-[#d2992208]">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 w-full px-2 py-1 text-[10px] text-[#d29922] hover:bg-[#d2992210] transition-colors"
      >
        {open ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
        <Wrench size={10} />
        <span className="font-medium">{name}</span>
        {preview && !open && (
          <span className="text-[var(--text-muted)] truncate ml-1">
            {preview}
          </span>
        )}
      </button>
      {open && preview && (
        <div className="px-2 pb-2">
          <pre className="text-[10px] font-mono text-[var(--text-primary)] bg-[var(--bg-input)] rounded p-1.5 overflow-auto max-h-32 whitespace-pre-wrap break-all">
            {preview}
          </pre>
        </div>
      )}
    </div>
  );
}

// ── 工具调用统计 ─────────────────────────────────────────

function ToolCallStats({ toolCalls, title = "工具调用分布", color = "#d29922" }: { toolCalls: ToolCallStat[]; title?: string; color?: string }) {
  const max = toolCalls[0]?.count ?? 1;
  return (
    <div className="px-4">
      <h4 className="text-xs font-semibold text-[var(--text-muted)] mb-2">
        {title}
      </h4>
      <div className="space-y-1">
        {toolCalls.map((tc) => (
          <div key={tc.name} className="flex items-center gap-2 h-5">
            <span className="text-[10px] font-mono text-[var(--text-primary)] w-24 truncate shrink-0">
              {tc.name}
            </span>
            <div className="flex-1 h-2 bg-[var(--bg-input)] rounded-full overflow-hidden">
              <div
                className="h-full rounded-full"
                style={{ width: `${(tc.count / max) * 100}%`, backgroundColor: `${color}99` }}
              />
            </div>
            <span className="text-[10px] text-[var(--text-muted)] w-6 text-right shrink-0">
              {tc.count}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── 工具函数 ─────────────────────────────────────────────

function decodeProjectName(encoded: string): { dirName: string; fullPath: string } {
  // 还原完整路径用于 description 显示
  const fullPath = encoded.replace(/--/g, ":\\").replace(/-/g, "\\");
  // dirName：去掉盘符前缀（如 d--），保留编码名原样，避免连字符目录名被截断
  const dirName = encoded.replace(/^[a-zA-Z]--/, "");
  return { dirName, fullPath };
}

function shortModelName(model: string): string {
  // "claude-sonnet-4-20250514" → "sonnet-4"
  const parts = model.split("/");
  const name = parts[parts.length - 1];
  if (name.length <= 20) return name;
  return name.replace(/-\d{8}$/, "").slice(-20);
}

function formatTokens(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return `${n}`;
}

function formatTime(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return ts;
  }
}

function formatRelativeTime(ts: string): string {
  if (!ts) return "";
  try {
    const d = new Date(ts);
    const now = new Date();
    const diff = now.getTime() - d.getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return "刚刚";
    if (mins < 60) return `${mins}分钟前`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}小时前`;
    const days = Math.floor(hours / 24);
    if (days < 7) return `${days}天前`;
    return d.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
  } catch {
    return ts;
  }
}
