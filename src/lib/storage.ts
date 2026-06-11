// localStorage 类型安全读写辅助

export const ls = {
  get: (k: string, fallback: string): string =>
    localStorage.getItem(k) ?? fallback,

  oneOf: <T extends string>(
    k: string,
    allowed: readonly T[],
    fallback: T,
  ): T => {
    const v = localStorage.getItem(k);
    return v != null && (allowed as readonly string[]).includes(v)
      ? (v as T)
      : fallback;
  },

  num: (
    k: string,
    fallback: number,
    min: number,
    max: number,
  ): number => {
    const v = localStorage.getItem(k);
    if (v == null) return fallback;
    const n = Number(v);
    if (!Number.isFinite(n)) return fallback;
    return Math.min(max, Math.max(min, n));
  },

  bool: (k: string, fallback: boolean): boolean => {
    const v = localStorage.getItem(k);
    return v == null ? fallback : v === "1";
  },

  set: (k: string, v: string | number | boolean): void =>
    localStorage.setItem(
      k,
      typeof v === "boolean" ? (v ? "1" : "0") : String(v),
    ),
};
