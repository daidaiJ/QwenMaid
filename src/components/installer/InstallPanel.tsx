import { useState, useEffect, useCallback, useRef } from "react";
import {
  detectNodeVersion,
  detectNpmVersion,
  detectQwenVersion,
  checkLatestQwenVersion,
  installQwenCode,
  updateQwenCode,
  configureNpmMirror,
  getNpmMirror,
} from "@/lib/tauri";
import { listen } from "@tauri-apps/api/event";
import {
  Download,
  CheckCircle,
  AlertCircle,
  RefreshCw,
  Globe,
  Terminal,
  Loader2,
  Package,
  ArrowUpCircle,
  ChevronDown,
  ChevronRight,
} from "lucide-react";

// ── 版本比较 ─────────────────────────────────────────────

function parseVersion(v: string): string {
  const match = v.match(/(\d+\.\d+\.\d+)/);
  return match ? match[1] : v.trim();
}

function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) - (pb[i] || 0);
  }
  return 0;
}

// ── 镜像源预设 ───────────────────────────────────────────

const MIRRORS = [
  { label: "npm 官方", value: "https://registry.npmjs.org" },
  { label: "npmmirror (淘宝)", value: "https://registry.npmmirror.com" },
];

// ── 状态指示 ─────────────────────────────────────────────

function StatusDot({ ok }: { ok: boolean | null }) {
  if (ok === null)
    return <Loader2 size={14} className="text-[var(--text-muted)] animate-spin" />;
  return ok ? (
    <CheckCircle size={14} className="text-[#3fb950] shrink-0" />
  ) : (
    <AlertCircle size={14} className="text-[#f44747] shrink-0" />
  );
}

function Section({
  icon,
  title,
  children,
  defaultOpen = true,
}: {
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="border border-[var(--border)] rounded-lg overflow-hidden">
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-4 h-10 bg-[var(--bg-sidebar)] hover:bg-[var(--bg-hover)] transition-colors text-left"
      >
        {open ? <ChevronDown size={14} className="text-[var(--text-muted)] shrink-0" /> : <ChevronRight size={14} className="text-[var(--text-muted)] shrink-0" />}
        {icon}
        <span className="text-[13px] font-medium text-[var(--text-primary)]">{title}</span>
      </button>
      {open && <div className="px-4 py-3">{children}</div>}
    </div>
  );
}

// ── 模块级缓存（组件卸载重挂载时不丢失） ─────────────────

const DETECT_CACHE_KEY = "qwenmaid:installDetectCache";
const CACHE_TTL = 5 * 60 * 1000; // 5 分钟

interface DetectCacheData {
  nodeVer: string | null;
  nodePath: string | null;
  npmVer: string | null;
  localVer: string | null;
  latestVer: string | null;
  mirror: string;
}

function getCachedDetect(): DetectCacheData | null {
  try {
    const raw = sessionStorage.getItem(DETECT_CACHE_KEY);
    if (!raw) return null;
    const { data, ts } = JSON.parse(raw);
    if (Date.now() - ts > CACHE_TTL) return null;
    return data;
  } catch {
    return null;
  }
}

function setCachedDetect(data: DetectCacheData) {
  try {
    sessionStorage.setItem(DETECT_CACHE_KEY, JSON.stringify({ data, ts: Date.now() }));
  } catch {}
}

// ── 主面板 ───────────────────────────────────────────────

export function InstallPanel() {
  // 环境检测
  const [nodeVer, setNodeVer] = useState<string | null>(null);
  const [nodePath, setNodePath] = useState<string | null>(null);
  const [npmVer, setNpmVer] = useState<string | null>(null);
  const [localVer, setLocalVer] = useState<string | null>(null);
  const [latestVer, setLatestVer] = useState<string | null>(null);
  const [loading, setLoading] = useState(() => !getCachedDetect());

  // 操作状态
  const [running, setRunning] = useState(false);
  const [logLines, setLogLines] = useState<string[]>([]);
  const logRef = useRef<HTMLPreElement>(null);

  // 监听 install-progress 事件
  useEffect(() => {
    const unlisten = listen<{ line: string; source: string }>(
      "install-progress",
      (ev) => {
        setLogLines((prev) => [...prev, ev.payload.line]);
        requestAnimationFrame(() => {
          logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
        });
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // 镜像源
  const [mirror, setMirror] = useState("");
  const [customMirror, setCustomMirror] = useState("");

  const detect = useCallback(async (force = false) => {
    // 命中缓存且未过期
    if (!force) {
      const cached = getCachedDetect();
      if (cached) {
        setNodeVer(cached.nodeVer);
        setNodePath(cached.nodePath);
        setNpmVer(cached.npmVer);
        setLocalVer(cached.localVer);
        setLatestVer(cached.latestVer);
        setMirror(cached.mirror);
        setLoading(false);
        return;
      }
    }

    setLoading(true);
    try {
      const [node, npm, local, latest, currentMirror] = await Promise.all([
        detectNodeVersion().catch(() => null),
        detectNpmVersion().catch(() => null),
        detectQwenVersion().catch(() => null),
        checkLatestQwenVersion().catch(() => null),
        getNpmMirror().catch(() => ""),
      ]);
      const result: DetectCacheData = {
        nodeVer: node?.version ?? null,
        nodePath: node?.path ?? null,
        npmVer: npm?.version ?? null,
        localVer: local ? parseVersion(local) : null,
        latestVer: latest ? parseVersion(latest) : null,
        mirror: currentMirror,
      };
      setNodeVer(result.nodeVer);
      setNodePath(result.nodePath);
      setNpmVer(result.npmVer);
      setLocalVer(result.localVer);
      setLatestVer(result.latestVer);
      setMirror(result.mirror);
      setCachedDetect(result);
    } finally {
      setLoading(false);
    }
  }, []);

  // 首次挂载检测
  useEffect(() => { detect(); }, [detect]);

  const installed = localVer !== null;
  const canUpdate =
    installed && latestVer && localVer && compareVersions(latestVer, localVer) > 0;
  const nodeOk = nodeVer !== null;
  const npmOk = npmVer !== null;

  // 整体状态
  const allGood = nodeOk && npmOk && installed && !canUpdate;

  const handleInstall = async () => {
    setRunning(true);
    setLogLines(["开始安装 @qwen-code/qwen-code …"]);
    installQwenCode(mirror || customMirror || undefined)
      .then(() => {
        setLogLines((prev) => [...prev, "✓ 安装完成"]);
        sessionStorage.removeItem(DETECT_CACHE_KEY);
        detect(true);
      })
      .catch((e) => setLogLines((prev) => [...prev, `✗ 安装失败: ${e}`]))
      .finally(() => setRunning(false));
  };

  const handleUpdate = async () => {
    setRunning(true);
    setLogLines(["开始更新 @qwen-code/qwen-code …"]);
    updateQwenCode(mirror || customMirror || undefined)
      .then(() => {
        setLogLines((prev) => [...prev, "✓ 更新完成"]);
        sessionStorage.removeItem(DETECT_CACHE_KEY);
        detect(true);
      })
      .catch((e) => setLogLines((prev) => [...prev, `✗ 更新失败: ${e}`]))
      .finally(() => setRunning(false));
  };

  const handleSetMirror = async (url: string) => {
    try {
      await configureNpmMirror(url);
      setMirror(url);
    } catch (e) {
      setLogLines((prev) => [...prev, `✗ 设置镜像失败: ${e}`]);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-auto">
      {/* 顶部状态栏 */}
      <div className="flex items-center justify-between px-5 h-11 border-b border-[var(--border)] shrink-0">
        <div className="flex items-center gap-2">
          {loading ? (
            <Loader2 size={14} className="text-[var(--text-muted)] animate-spin" />
          ) : allGood ? (
            <CheckCircle size={14} className="text-[#3fb950]" />
          ) : (
            <AlertCircle size={14} className="text-[#d29922]" />
          )}
          <span className="text-[13px] font-medium text-[var(--text-primary)]">
            {loading
              ? "检测环境中…"
              : allGood
              ? "环境就绪"
              : "需要配置"}
          </span>
        </div>
        <button
          onClick={() => detect(true)}
          disabled={loading}
          className="flex items-center gap-1.5 px-2.5 h-7 text-[11px] text-[var(--text-secondary)] border border-[var(--border)] rounded-md hover:bg-[var(--bg-hover)] disabled:opacity-40 transition-colors"
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
          刷新
        </button>
      </div>

      {/* 内容区 */}
      <div className="flex-1 p-5 space-y-4 overflow-auto">

        {/* ① 前置依赖 */}
        <Section
          icon={<Terminal size={14} className="text-[var(--text-muted)]" />}
          title="前置依赖"
          defaultOpen={!nodeOk || !npmOk}
        >
          <div className="space-y-2">
            <EnvRow
              label="Node.js"
              version={nodeVer}
              loading={loading}
              hint={nodePath ?? undefined}
              ok={nodeOk}
            />
            <EnvRow
              label="npm"
              version={npmVer}
              loading={loading}
              ok={npmOk}
            />
          </div>
          {!nodeOk && !loading && (
            <div className="mt-3 flex items-start gap-2 p-2.5 rounded-md bg-[#f4474710] border border-[#f4474730]">
              <AlertCircle size={14} className="text-[#f44747] shrink-0 mt-0.5" />
              <div className="text-[11px] text-[#f44747] leading-relaxed">
                需要先安装 Node.js 20+。
                <a
                  href="https://nodejs.org"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="ml-1 underline hover:text-[#f44747]/80"
                >
                  前往下载 →
                </a>
              </div>
            </div>
          )}
        </Section>

        {/* ② Qwen Code 安装 / 更新 */}
        <Section
          icon={<Package size={14} className="text-[var(--text-muted)]" />}
          title="Qwen Code"
          defaultOpen={!installed || !!canUpdate}
        >
          {loading ? (
            <div className="flex items-center justify-center py-6">
              <Loader2 size={18} className="text-[var(--text-muted)] animate-spin" />
            </div>
          ) : !installed ? (
            /* 未安装 */
            <div className="space-y-3">
              <p className="text-[12px] text-[var(--text-muted)]">
                Qwen Code 尚未安装。点击下方按钮一键安装。
              </p>
              <button
                onClick={handleInstall}
                disabled={running || !npmOk}
                className="flex items-center justify-center gap-2 w-full h-9 text-[12px] font-medium bg-[var(--accent)] text-white rounded-md hover:bg-[var(--accent-hover)] disabled:opacity-40 transition-colors"
              >
                {running ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Download size={14} />
                )}
                {running ? "安装中…" : "安装 Qwen Code"}
              </button>
            </div>
          ) : (
            /* 已安装 */
            <div className="space-y-3">
              <div className="flex items-center gap-4">
                <div className="flex items-center gap-2">
                  <StatusDot ok={true} />
                  <span className="text-[12px] text-[var(--text-muted)]">当前</span>
                  <span className="text-[14px] font-mono font-medium text-[var(--text-primary)]">
                    {localVer}
                  </span>
                </div>
                {latestVer && (
                  <>
                    <span className="text-[var(--text-muted)]">→</span>
                    <div className="flex items-center gap-2">
                      <span className="text-[12px] text-[var(--text-muted)]">最新</span>
                      <span
                        className={`text-[14px] font-mono font-medium ${
                          canUpdate ? "text-[#d29922]" : "text-[#3fb950]"
                        }`}
                      >
                        {latestVer}
                      </span>
                    </div>
                  </>
                )}
              </div>

              {canUpdate ? (
                <button
                  onClick={handleUpdate}
                  disabled={running}
                  className="flex items-center justify-center gap-2 w-full h-9 text-[12px] font-medium bg-[#d29922] text-white rounded-md hover:bg-[#d29922]/80 disabled:opacity-40 transition-colors"
                >
                  {running ? (
                    <Loader2 size={14} className="animate-spin" />
                  ) : (
                    <ArrowUpCircle size={14} />
                  )}
                  {running ? "更新中…" : "更新到最新版本"}
                </button>
              ) : (
                <div className="flex items-center gap-1.5 text-[12px] text-[#3fb950]">
                  <CheckCircle size={13} />
                  已是最新版本
                </div>
              )}
            </div>
          )}
        </Section>

        {/* ③ npm 镜像源 */}
        <Section
          icon={<Globe size={14} className="text-[var(--text-muted)]" />}
          title="npm 镜像源"
          defaultOpen={false}
        >
          <div className="space-y-3">
            <div className="flex items-center gap-2">
              <span className="text-[11px] text-[var(--text-muted)] shrink-0">当前：</span>
              <span className="text-[11px] font-mono text-[var(--text-primary)] truncate">
                {mirror || "未配置（使用官方源）"}
              </span>
            </div>

            <div className="flex flex-wrap gap-1.5">
              {MIRRORS.map((m) => (
                <button
                  key={m.value}
                  onClick={() => handleSetMirror(m.value)}
                  className={`px-2.5 py-1.5 text-[11px] rounded-md border transition-colors ${
                    mirror === m.value
                      ? "border-[var(--accent)] bg-[var(--accent-light)] text-[var(--accent)]"
                      : "border-[var(--border)] hover:bg-[var(--bg-hover)] text-[var(--text-secondary)]"
                  }`}
                >
                  {m.label}
                </button>
              ))}
            </div>

            <div className="flex gap-1.5">
              <input
                value={customMirror}
                onChange={(e) => setCustomMirror(e.target.value)}
                placeholder="自定义镜像 URL"
                className="flex-1 h-7 bg-[var(--bg-input)] border border-[var(--border)] rounded-md px-2 text-[11px] text-[var(--text-primary)] placeholder:text-[var(--text-muted)] outline-none focus:border-[var(--accent)] transition-colors"
              />
              <button
                onClick={() => customMirror && handleSetMirror(customMirror)}
                className="px-3 h-7 text-[11px] bg-[var(--accent)] text-white rounded-md hover:bg-[var(--accent-hover)] transition-colors shrink-0"
              >
                应用
              </button>
            </div>
          </div>
        </Section>

        {/* ④ 操作日志 */}
        {logLines.length > 0 && (
          <Section
            icon={<Terminal size={14} className="text-[var(--text-muted)]" />}
            title="操作日志"
            defaultOpen={true}
          >
            <pre
              ref={logRef}
              className="bg-[var(--bg-input)] border border-[var(--border)] rounded-md p-3 text-[11px] font-mono text-[var(--text-primary)] overflow-auto max-h-48 whitespace-pre-wrap"
            >
              {logLines.join("\n")}
            </pre>
          </Section>
        )}

        {/* 底部链接 */}
        <div className="flex items-center gap-4 pt-2">
          <a
            href="https://github.com/QwenLM/qwen-code/releases"
            target="_blank"
            rel="noopener noreferrer"
            className="text-[11px] text-[var(--accent)] hover:underline"
          >
            GitHub Releases
          </a>
          <a
            href="https://github.com/QwenLM/qwen-code"
            target="_blank"
            rel="noopener noreferrer"
            className="text-[11px] text-[var(--accent)] hover:underline"
          >
            文档
          </a>
        </div>
      </div>
    </div>
  );
}

// ── 环境行 ───────────────────────────────────────────────

function EnvRow({
  label,
  version,
  loading,
  hint,
  ok,
}: {
  label: string;
  version: string | null;
  loading: boolean;
  hint?: string;
  ok: boolean;
}) {
  return (
    <div className="flex items-center gap-2.5 h-9 px-2 rounded-md hover:bg-[var(--bg-hover)]">
      <StatusDot ok={loading ? null : ok} />
      <span className="text-[12px] text-[var(--text-primary)] w-16 shrink-0">{label}</span>
      <span className="flex-1 text-[11px] font-mono text-[var(--text-muted)] truncate">
        {loading ? "检测中…" : version ?? "未安装"}
      </span>
      {hint && !loading && (
        <span className="text-[10px] text-[var(--text-muted)] truncate max-w-[200px]">
          {hint}
        </span>
      )}
    </div>
  );
}
