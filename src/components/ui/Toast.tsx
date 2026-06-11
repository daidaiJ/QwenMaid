// Toast 通知组件 — 底部居中药丸，支持 error 样式和 Undo action

import { useEffect } from "react";
import { useToasts } from "@/store";
import { AlertCircle, X, Undo2 } from "lucide-react";

export function ToastHost() {
  const current = useToasts((s) => s.current);
  const dismiss = useToasts((s) => s.dismiss);

  useEffect(() => {
    if (!current) return;
    const id = current.id;
    const timer = window.setTimeout(
      () => useToasts.getState().dismiss(id),
      current.duration,
    );
    return () => window.clearTimeout(timer);
  }, [current]);

  if (!current) return null;

  const isError = current.tone === "error";

  return (
    <div
      role="status"
      aria-live="polite"
      className="fixed bottom-6 left-1/2 z-[300]"
      style={{
        animation: "toast-in 0.26s cubic-bezier(.22,.61,.36,1)",
      }}
    >
      <div
        className={`
          flex items-center gap-2.5 pl-4 pr-2 py-2 rounded-full text-[12.5px] font-medium
          shadow-lg max-w-[min(460px,calc(100vw-32px))]
          ${isError
            ? "bg-[var(--color-error)] text-white"
            : "bg-[var(--text-primary)] text-[var(--text-inverse)]"
          }
        `}
        style={{ transform: "translateX(-50%)" }}
      >
        {isError && (
          <AlertCircle size={14} className="shrink-0 opacity-80" />
        )}
        <span className="truncate">{current.text}</span>
        {current.action && (
          <button
            onClick={() => {
              current.action!.run();
              dismiss(current.id);
            }}
            className="flex items-center gap-1 shrink-0 px-3 py-1 rounded-full text-[12px] font-semibold
              transition-colors hover:bg-white/15"
          >
            <Undo2 size={12} />
            {current.action.label}
          </button>
        )}
        {(isError || current.action) && (
          <button
            onClick={() => dismiss(current.id)}
            className="shrink-0 w-[22px] h-[22px] flex items-center justify-center rounded-full
              transition-colors hover:bg-white/15 opacity-60 hover:opacity-100"
            aria-label="关闭"
          >
            <X size={13} />
          </button>
        )}
      </div>
    </div>
  );
}
