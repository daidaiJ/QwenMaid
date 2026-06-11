// 浮层关闭 — 点击外部 / Escape / Tab 出界

import { useEffect, type RefObject } from "react";

interface Options {
  onFocusOut?: boolean;
}

export function useDismiss(
  ref: RefObject<HTMLElement | null>,
  onClose: () => void,
  { onFocusOut = false }: Options = {},
) {
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    const onBlur = (e: FocusEvent) => {
      const next = e.relatedTarget as Node | null;
      if (next && !ref.current?.contains(next)) onClose();
    };
    const tm = window.setTimeout(() => {
      document.addEventListener("mousedown", onDown);
      window.addEventListener("keydown", onKey);
      if (onFocusOut) document.addEventListener("focusout", onBlur);
    }, 0);
    return () => {
      window.clearTimeout(tm);
      document.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
      if (onFocusOut) document.removeEventListener("focusout", onBlur);
    };
  }, [ref, onClose, onFocusOut]);
}
