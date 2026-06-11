// 全局 UI 状态 — theme + toast

import { create } from "zustand";
import { ls } from "./lib/storage";

export type Theme = "light" | "dark";

interface UiState {
  theme: Theme;
  setTheme: (t: Theme) => void;
}

export const useUi = create<UiState>((set) => ({
  theme: ls.oneOf<Theme>("ab:theme", ["light", "dark"], "light"),
  setTheme: (theme) => {
    ls.set("ab:theme", theme);
    document.documentElement.dataset.theme = theme;
    set({ theme });
  },
}));

// ── Toast ────────────────────────────────────────────────

export type ToastTone = "default" | "error";

export interface ToastAction {
  label: string;
  run: () => void;
}

export interface ToastItem {
  id: number;
  text: string;
  tone: ToastTone;
  action?: ToastAction;
  duration: number;
}

interface ToastState {
  current: ToastItem | null;
  push: (t: Omit<ToastItem, "id">) => number;
  dismiss: (id?: number) => void;
}

let seq = 0;

export const useToasts = create<ToastState>((set, get) => ({
  current: null,
  push: (t) => {
    const id = ++seq;
    set({ current: { ...t, id } });
    return id;
  },
  dismiss: (id) => {
    const cur = get().current;
    if (cur && (id === undefined || cur.id === id)) set({ current: null });
  },
}));

const SUCCESS_MS = 2000;
const ERROR_MS = 6000;
const UNDO_MS = 6000;

export const toast = {
  show: (text: string) =>
    useToasts.getState().push({ text, tone: "default", duration: SUCCESS_MS }),
  error: (text: string) =>
    useToasts.getState().push({ text, tone: "error", duration: ERROR_MS }),
};

export function reportError(e: unknown): void {
  const msg = e instanceof Error ? e.message : String(e);
  toast.error(msg);
}

export function withUndo(opts: {
  text: string;
  apply: () => void;
  commit: () => void;
  revert: () => void;
}): void {
  opts.apply();
  let settled = false;
  const timer = window.setTimeout(() => {
    if (settled) return;
    settled = true;
    opts.commit();
  }, UNDO_MS);
  useToasts.getState().push({
    text: opts.text,
    tone: "default",
    duration: UNDO_MS,
    action: {
      label: "撤销",
      run: () => {
        if (settled) return;
        settled = true;
        window.clearTimeout(timer);
        opts.revert();
      },
    },
  });
}
