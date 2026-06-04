import { useState, useEffect, useRef } from "react";
import { Eye, EyeOff, X, FolderOpen } from "lucide-react";
import { revealInExplorer } from "@/lib/tauri";

// ── 输入框通用样式 ───────────────────────────────────────
const inputCls =
  "h-9 bg-[var(--bg-input)] border border-[var(--border)] rounded-md px-3 text-[13px] text-[var(--text-primary)] placeholder:text-[var(--text-muted)] focus:border-[var(--accent)] focus:shadow-[0_0_0_2px_rgba(124,58,237,0.15)] outline-none transition-all shadow-sm";

// ── Toggle 开关 ──────────────────────────────────────────

export function Toggle({
  value,
  onChange,
  disabled,
}: {
  value: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={value}
      disabled={disabled}
      onClick={() => onChange(!value)}
      className={`relative inline-flex h-[22px] w-10 shrink-0 cursor-pointer items-center rounded-full border transition-colors ${
        value
          ? "bg-[var(--accent)] border-[var(--accent)]"
          : "bg-[var(--border-strong)] border-[var(--border-strong)]"
      } ${disabled ? "opacity-50 cursor-not-allowed" : ""}`}
    >
      <span
        className={`pointer-events-none block h-3.5 w-3.5 rounded-full bg-white shadow transition-transform ${
          value ? "translate-x-[18px]" : "translate-x-[3px]"
        }`}
      />
    </button>
  );
}

// ── Select 下拉 ──────────────────────────────────────────

export function Select({
  value,
  onChange,
  options,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
  placeholder?: string;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className={`${inputCls} min-w-[140px]`}
    >
      {placeholder && (
        <option value="" className="text-[var(--text-muted)]">
          {placeholder}
        </option>
      )}
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

// ── TextInput 文本输入 ────────────────────────────────────

export function TextInput({
  value,
  onChange,
  placeholder,
  type = "text",
  disabled,
  mono,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  type?: "text" | "password";
  disabled?: boolean;
  mono?: boolean;
}) {
  return (
    <input
      type={type}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      disabled={disabled}
      className={`${inputCls} w-full ${mono ? "font-mono text-xs" : ""} ${
        disabled ? "opacity-50 cursor-not-allowed" : ""
      }`}
    />
  );
}

// ── SecretInput 密码输入（带显示/隐藏） ───────────────────

export function SecretInput({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  const [visible, setVisible] = useState(false);

  return (
    <div className="relative">
      <input
        type={visible ? "text" : "password"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={`${inputCls} w-full pr-8 font-mono text-xs`}
      />
      <button
        type="button"
        onClick={() => setVisible(!visible)}
        className="absolute right-1.5 top-1/2 -translate-y-1/2 text-[var(--text-muted)] hover:text-[var(--text-primary)]"
      >
        {visible ? <EyeOff size={14} /> : <Eye size={14} />}
      </button>
    </div>
  );
}

// ── NumberInput 数字输入 ──────────────────────────────────

export function NumberInput({
  value,
  onChange,
  min,
  max,
  step,
  placeholder,
  unit,
}: {
  value: number | undefined;
  onChange: (v: number | undefined) => void;
  min?: number;
  max?: number;
  step?: number;
  placeholder?: string;
  unit?: string;
}) {
  const [str, setStr] = useState(value !== undefined ? String(value) : "");

  useEffect(() => {
    setStr(value !== undefined ? String(value) : "");
  }, [value]);

  const commit = () => {
    if (str === "") {
      onChange(undefined);
    } else {
      const n = Number(str);
      if (!isNaN(n)) onChange(n);
    }
  };

  return (
    <div className="flex items-center gap-1.5">
      <input
        type="number"
        value={str}
        onChange={(e) => setStr(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
        min={min}
        max={max}
        step={step}
        placeholder={placeholder}
        className={`${inputCls} w-28 font-mono text-xs`}
      />
      {unit && <span className="text-[11px] text-[var(--text-muted)]">{unit}</span>}
    </div>
  );
}

// ── TagInput 标签输入 ────────────────────────────────────

export function TagInput({
  value,
  onChange,
  placeholder,
  suggestions,
}: {
  value: string[];
  onChange: (v: string[]) => void;
  placeholder?: string;
  suggestions?: string[];
}) {
  const [input, setInput] = useState("");
  const [showSug, setShowSug] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const addTag = (tag: string) => {
    const t = tag.trim();
    if (t && !value.includes(t)) onChange([...value, t]);
    setInput("");
    setShowSug(false);
  };

  const removeTag = (tag: string) => {
    onChange(value.filter((v) => v !== tag));
  };

  const filtered = (suggestions ?? []).filter(
    (s) => s.toLowerCase().includes(input.toLowerCase()) && !value.includes(s)
  );

  return (
    <div ref={ref} className="relative">
      <div className="flex flex-wrap gap-1.5 p-2 bg-[var(--bg-input)] border border-[var(--border)] rounded-md min-h-[36px] focus-within:border-[var(--accent)] focus-within:shadow-[0_0_0_2px_rgba(124,58,237,0.15)] transition-all shadow-sm">
        {value.map((tag) => (
          <span
            key={tag}
            className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-[var(--tag-bg)] text-[12px] text-[var(--text-primary)] rounded-sm"
          >
            <span className="font-mono text-[11px]">{tag}</span>
            <button
              type="button"
              onClick={() => removeTag(tag)}
              className="text-[var(--text-muted)] hover:text-[var(--color-error)]"
            >
              <X size={10} />
            </button>
          </span>
        ))}
        <input
          value={input}
          onChange={(e) => { setInput(e.target.value); setShowSug(true); }}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === ",") { e.preventDefault(); addTag(input); }
            if (e.key === "Backspace" && !input && value.length > 0) removeTag(value[value.length - 1]);
          }}
          onFocus={() => setShowSug(true)}
          onBlur={() => setTimeout(() => setShowSug(false), 150)}
          placeholder={value.length === 0 ? placeholder : ""}
          className="flex-1 min-w-[80px] bg-transparent text-[12px] text-[var(--text-primary)] placeholder:text-[var(--text-muted)] outline-none"
        />
      </div>
      {showSug && filtered.length > 0 && (
        <div className="absolute z-10 top-full left-0 right-0 mt-0.5 bg-[var(--bg-panel)] border border-[var(--border-strong)] rounded-sm shadow-lg max-h-[160px] overflow-auto">
          {filtered.map((s) => (
            <button
              key={s}
              type="button"
              onMouseDown={(e) => { e.preventDefault(); addTag(s); }}
              className="w-full text-left px-2 py-1.5 text-[12px] text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
            >
              {s}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

// ── Field 包装器 ─────────────────────────────────────────

export function Field({
  label,
  description,
  children,
  inline,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
  inline?: boolean;
}) {
  if (inline) {
    return (
      <div className="flex items-center justify-between py-2 px-1 group">
        <div className="flex-1 min-w-0 mr-4">
          <label className="text-[13px] text-[var(--text-primary)]">{label}</label>
          {description && (
            <p className="text-[11px] text-[var(--text-muted)] mt-0.5 leading-relaxed">{description}</p>
          )}
        </div>
        <div className="shrink-0">{children}</div>
      </div>
    );
  }
  return (
    <div className="space-y-1.5 py-2 px-1">
      <label className="block text-[13px] text-[var(--text-primary)]">{label}</label>
      {description && (
        <p className="text-[11px] text-[var(--text-muted)] leading-relaxed">{description}</p>
      )}
      {children}
    </div>
  );
}

// ── Section 分隔标题 ─────────────────────────────────────

export function Section({ title, description }: { title: string; description?: string }) {
  return (
    <div className="pt-4 pb-1">
      <h3 className="text-xs font-semibold uppercase tracking-wider text-[var(--text-secondary)]">{title}</h3>
      {description && <p className="text-[11px] text-[var(--text-muted)] mt-0.5">{description}</p>}
      <div className="mt-2 border-t border-[var(--border)]" />
    </div>
  );
}

// ── OpenDirButton — 在系统文件管理器中打开路径 ───────────

export function OpenDirButton({
  path,
  title,
  className,
}: {
  path: string;
  title?: string;
  className?: string;
}) {
  const [error, setError] = useState<string | null>(null);

  const handleClick = async () => {
    try {
      setError(null);
      await revealInExplorer(path);
    } catch (e) {
      setError(String(e));
      setTimeout(() => setError(null), 3000);
    }
  };

  return (
    <div className="relative inline-flex items-center">
      <button
        type="button"
        onClick={handleClick}
        title={title ?? `打开: ${path}`}
        className={`inline-flex items-center gap-1 text-[var(--text-muted)] hover:text-[var(--accent)] transition-colors ${className ?? ""}`}
      >
        <FolderOpen size={13} />
      </button>
      {error && (
        <span className="absolute bottom-full left-0 mb-1 whitespace-nowrap text-[10px] text-[var(--color-error)] bg-[var(--color-error-bg)] px-1.5 py-0.5 rounded-sm shadow-lg z-10">
          {error}
        </span>
      )}
    </div>
  );
}

// ── FilePathField — 带「打开目录」按钮的路径输入框 ────────

export function FilePathField({
  value,
  onChange,
  placeholder,
  revealPath,
  browseHint,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  revealPath?: string;
  browseHint?: string;
}) {
  return (
    <div className="space-y-1">
      <div className="flex items-center gap-1.5">
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className={`${inputCls} flex-1 font-mono text-xs`}
        />
        {revealPath && (
          <OpenDirButton
            path={revealPath}
            title="在资源管理器中打开"
            className="h-8 w-8 justify-center bg-[var(--bg-input)] border border-[var(--border)] rounded-sm hover:border-[var(--accent)] hover:bg-[var(--bg-hover)]"
          />
        )}
      </div>
      {browseHint && <p className="text-[10px] text-[var(--text-muted)]">{browseHint}</p>}
    </div>
  );
}

// ── QuickPathNav — 常用目录快捷入口 ─────────────────────

export interface PathEntry {
  label: string;
  path: string;
  icon?: React.ReactNode;
}

export function QuickPathNav({ entries }: { entries: PathEntry[] }) {
  return (
    <div className="flex flex-wrap gap-1.5 mb-4">
      {entries.map((e) => (
        <button
          key={e.path}
          onClick={() => revealInExplorer(e.path).catch(() => {})}
          className="inline-flex items-center gap-1 px-2.5 py-1.5 bg-[var(--bg-card)] border border-[var(--border)] rounded-md text-[11px] text-[var(--text-secondary)] hover:text-[var(--accent)] hover:border-[var(--accent)] hover:shadow-[var(--shadow-sm)] transition-all shadow-[var(--shadow-card)]"
          title={e.path}
        >
          {e.icon ?? <FolderOpen size={11} />}
          {e.label}
        </button>
      ))}
    </div>
  );
}
