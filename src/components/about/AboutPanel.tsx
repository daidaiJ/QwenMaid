import { ExternalLink, Star, Code2, Layers, Settings, Network, BarChart3, MessageSquare, Puzzle, BookOpen, Search, Activity } from "lucide-react";
import type { LucideIcon } from "lucide-react";

const APP_VERSION = "0.1.0";
const REPO_URL = "https://github.com/daidaiJ/QwenMaid";
const AUTHOR_URL = "https://github.com/daidaiJ";

const features: { icon: LucideIcon; color: string; title: string; desc: string }[] = [
  { icon: Settings, color: "#58a6ff", title: "可视化配置管理", desc: "图形化编辑 settings.json，13 个预设供应商模板" },
  { icon: Network, color: "#3fb950", title: "本地路由代理", desc: "鉴权转换 · 多供应商加权路由 · 上下文压缩" },
  { icon: BarChart3, color: "#d29922", title: "用量分析", desc: "Token 统计 · 模型排名 · TPS/P50/P95 · 热力图" },
  { icon: MessageSquare, color: "#bc8cff", title: "会话分析", desc: "JSONL 解析 · 消息详情 · Token 消耗追踪" },
  { icon: Search, color: "#f48771", title: "MCP 网络服务", desc: "内嵌搜索引擎 · 学术检索 · 网页抓取（移植自 websearch-mcpserver）" },
  { icon: Activity, color: "#56d364", title: "状态行用量追踪", desc: "内嵌 qwen-code-usage CLI，实时采集状态行 Token 用量" },
  { icon: Puzzle, color: "#f48771", title: "扩展管理", desc: "技能 · 子智能体 · MCP 服务器发现与管理" },
  { icon: BookOpen, color: "#79c0ff", title: "记忆管理", desc: "可视化管理 .qwen/ 项目记忆文件" },
];

const deps = [
  { name: "only-cc-lite", url: "https://github.com/daidaiJ/only-cc-lite" },
  { name: "websearch-mcpserver", url: "https://github.com/daidaiJ/websearch-mcpserver" },
  { name: "qwen-code-usage", url: "https://github.com/daidaiJ/qwen-code-usage" },
  { name: "Tauri", url: "https://tauri.app" },
  { name: "axum", url: "https://github.com/tokio-rs/axum" },
];

export function AboutPanel() {
  return (
    <div className="h-full overflow-auto p-6 font-mono">
      <div className="max-w-2xl mx-auto space-y-5">
        {/* Header block — terminal style */}
        <div className="p-4 rounded-lg bg-[var(--bg-sidebar)] border border-[var(--border)]">
          <div className="flex items-baseline gap-3">
            <span className="text-[var(--accent)] text-lg font-bold tracking-tight">QWenMaid</span>
            <span className="text-xs text-[var(--text-muted)]">v{APP_VERSION}</span>
          </div>
          <p className="text-[var(--text-secondary)] text-xs mt-1.5 leading-relaxed">
            <a href="https://github.com/QwenLM/qwen-code" target="_blank" rel="noopener noreferrer" className="text-[var(--accent)] hover:underline">Qwen Code</a> 的配套管理工具 — 供应商配置 / 代理转发 / 用量统计 / 会话分析
          </p>
          <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-[10px] text-[var(--text-muted)]">
            <span><span className="text-[var(--accent)]">platform</span>=win32 · macos · linux</span>
            <span><span className="text-[var(--accent)]">runtime</span>=tauri-2 + rust</span>
            <span><span className="text-[var(--accent)]">ui</span>=react-19 + vite-8 + tailwind-4</span>
          </div>
        </div>

        {/* Star — minimal */}
        <a
          href={REPO_URL}
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-2 py-2 px-3 rounded bg-[var(--bg-sidebar)] border border-[var(--border)] text-xs text-[var(--text-muted)] hover:text-[var(--accent)] hover:border-[var(--accent)]/40 transition-colors group"
        >
          <Star size={13} className="text-amber-500 group-hover:fill-amber-500 transition-all" />
          <span className="font-mono">⭐ {REPO_URL.replace("https://", "")}</span>
          <ExternalLink size={11} className="ml-auto opacity-50" />
        </a>

        {/* Features — compact list */}
        <section>
          <h2 className="text-[10px] uppercase tracking-[0.15em] text-[var(--text-muted)] mb-2 flex items-center gap-1.5">
            <Layers size={12} /> features
          </h2>
          <div className="grid grid-cols-2 gap-1.5">
            {features.map((f) => (
              <div
                key={f.title}
                className="flex items-start gap-2.5 py-2 px-2.5 rounded bg-[var(--bg-sidebar)] border border-transparent hover:border-[var(--border)] transition-colors"
              >
                <f.icon size={14} style={{ color: f.color }} className="shrink-0 mt-0.5" />
                <div className="min-w-0">
                  <div className="text-xs font-medium">{f.title}</div>
                  <div className="text-[11px] text-[var(--text-muted)] leading-snug mt-0.5">{f.desc}</div>
                </div>
              </div>
            ))}
          </div>
        </section>

        {/* Tech — inline */}
        <section>
          <h2 className="text-[10px] uppercase tracking-[0.15em] text-[var(--text-muted)] mb-2 flex items-center gap-1.5">
            <Code2 size={12} /> stack
          </h2>
          <div className="p-3 rounded bg-[var(--bg-sidebar)] border border-[var(--border)] text-xs text-[var(--text-secondary)] leading-relaxed">
            <div><span className="text-[var(--accent)]">frontend</span> — React 19 · TypeScript · Vite 8 · Tailwind CSS 4 · shadcn/ui</div>
            <div><span className="text-[var(--accent)]">backend</span> — Rust · Tauri 2.x · SQLite (rusqlite) · axum</div>
            <div><span className="text-[var(--accent)]">compress</span> — only-cc-lite (零 ML 依赖上下文压缩)</div>
          </div>
        </section>

        {/* Dependencies */}
        <section>
          <h2 className="text-[10px] uppercase tracking-[0.15em] text-[var(--text-muted)] mb-2">dependencies</h2>
          <div className="space-y-1">
            {deps.map((d) => (
              <a
                key={d.name}
                href={d.url}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 py-1.5 px-3 rounded bg-[var(--bg-sidebar)] border border-transparent hover:border-[var(--border)] transition-colors group"
              >
                <ExternalLink size={10} className="text-[var(--text-muted)] opacity-0 group-hover:opacity-100 transition-opacity" />
                <span className="text-xs text-[var(--accent)] group-hover:underline">{d.name}</span>
              </a>
            ))}
          </div>
        </section>

        {/* Author */}
        <section>
          <h2 className="text-[10px] uppercase tracking-[0.15em] text-[var(--text-muted)] mb-2">author</h2>
          <div className="p-3 rounded bg-[var(--bg-sidebar)] border border-[var(--border)] text-xs">
            <div className="flex items-baseline gap-3 mb-2">
              <span className="text-[var(--accent)] font-bold">daidaiJ</span>
              <span className="text-[var(--text-muted)]">🧊 冰可乐爱好者</span>
            </div>
            <div className="space-y-1 text-[var(--text-muted)]">
              <div>
                <span className="text-[var(--text-secondary)] w-16 inline-block">github</span>
                <a href={AUTHOR_URL} target="_blank" rel="noopener noreferrer" className="text-[var(--accent)] hover:underline">{AUTHOR_URL.replace("https://", "")}</a>
              </div>
              <div>
                <span className="text-[var(--text-secondary)] w-16 inline-block">repo</span>
                <a href={REPO_URL} target="_blank" rel="noopener noreferrer" className="text-[var(--accent)] hover:underline">{REPO_URL.replace("https://", "")}</a>
              </div>
              <div>
                <span className="text-[var(--text-secondary)] w-16 inline-block">issues</span>
                <a href={`${REPO_URL}/issues`} target="_blank" rel="noopener noreferrer" className="text-[var(--accent)] hover:underline">{REPO_URL.replace("https://", "")}/issues</a>
              </div>
            </div>
          </div>
        </section>

        {/* Footer */}
        <div className="text-center text-[10px] text-[var(--text-muted)] py-3">
          &copy; {new Date().getFullYear()} daidaiJ · MIT License
        </div>
      </div>
    </div>
  );
}
