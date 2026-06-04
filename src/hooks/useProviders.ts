import { useState, useCallback, useEffect } from "react";
import * as api from "@/lib/tauri";
import type { Provider, Model } from "@/lib/tauri";

export function useProviders() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setProviders(await api.listProviders());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  return { providers, loading, error, refresh };
}

export function useModels(providerId?: number) {
  const [models, setModels] = useState<Model[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    setModels(await api.listModels(providerId));
    setLoading(false);
  }, [providerId]);

  useEffect(() => { refresh(); }, [refresh]);

  return { models, loading, refresh };
}
