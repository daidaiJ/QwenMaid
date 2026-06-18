import { useState, useEffect, useCallback, useRef } from "react";
import { listConfiguredModelIds } from "@/lib/tauri";

/**
 * 模块级缓存：跨组件共享，避免重复调用后端
 * 后端也有缓存，这里是前端二级缓存，减少 IPC 开销
 */
let cachedIds: string[] | null = null;
let cacheTime = 0;
const CACHE_TTL_MS = 5 * 60 * 1000; // 5 分钟

/** 手动使前端缓存失效（sync/detect 后调用） */
export function invalidateModelIdsCache() {
  cachedIds = null;
  cacheTime = 0;
}

/** 获取已配置 model IDs（带前端缓存） */
async function fetchModelIds(): Promise<string[]> {
  const now = Date.now();
  if (cachedIds && now - cacheTime < CACHE_TTL_MS) {
    return cachedIds;
  }
  const ids = await listConfiguredModelIds();
  cachedIds = ids;
  cacheTime = now;
  return ids;
}

/**
 * 获取 settings.json 中已配置的 model ID 列表
 * 用于下拉选择器等场景，自带前端缓存
 */
export function useConfiguredModels() {
  const [models, setModels] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const mountedRef = useRef(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const ids = await fetchModelIds();
      if (mountedRef.current) setModels(ids);
    } finally {
      if (mountedRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    refresh();
    return () => { mountedRef.current = false; };
  }, [refresh]);

  return { models, loading, refresh };
}
