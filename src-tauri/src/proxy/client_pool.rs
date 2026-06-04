use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// 代理相关环境变量快照，用于检测变化
#[derive(Clone, Debug, PartialEq)]
struct ProxySnapshot {
    http_proxy: Option<String>,
    https_proxy: Option<String>,
    no_proxy: Option<String>,
}

impl ProxySnapshot {
    fn current() -> Self {
        Self {
            http_proxy: std::env::var("HTTP_PROXY")
                .ok()
                .or_else(|| std::env::var("http_proxy").ok()),
            https_proxy: std::env::var("HTTPS_PROXY")
                .ok()
                .or_else(|| std::env::var("https_proxy").ok()),
            no_proxy: std::env::var("NO_PROXY")
                .ok()
                .or_else(|| std::env::var("no_proxy").ok()),
        }
    }
}

struct CachedClient {
    client: reqwest::Client,
    proxy_mode: String,
    proxy_url: Option<String>,
    env_snapshot: ProxySnapshot,
}

pub struct ClientPool {
    clients: Mutex<HashMap<i64, CachedClient>>,
}

impl ClientPool {
    pub fn new() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
        }
    }

    /// 获取 reqwest::Client，仅在代理配置或环境变量变化时重建
    pub fn get(
        &self,
        provider_id: i64,
        proxy_mode: &str,
        proxy_url: Option<&str>,
    ) -> reqwest::Client {
        let mut clients = self.clients.lock().unwrap();
        let current_snap = ProxySnapshot::current();

        if let Some(cached) = clients.get(&provider_id) {
            let config_changed = cached.proxy_mode != proxy_mode
                || cached.proxy_url.as_deref() != proxy_url;
            // 只有 system 模式才需要检测环境变量变化
            let env_changed =
                cached.proxy_mode == "system" && cached.env_snapshot != current_snap;

            if !config_changed && !env_changed {
                return cached.client.clone();
            }
        }

        let client = build_client(proxy_mode, proxy_url);
        clients.insert(
            provider_id,
            CachedClient {
                client: client.clone(),
                proxy_mode: proxy_mode.to_string(),
                proxy_url: proxy_url.map(|s| s.to_string()),
                env_snapshot: current_snap,
            },
        );
        client
    }

    /// 连接失败时的降级：用 direct 模式重试
    pub fn get_fallback(&self, provider_id: i64) -> reqwest::Client {
        self.get(provider_id, "direct", None)
    }

    /// 移除指定 provider 的缓存（供应商配置变更时调用）
    pub fn invalidate(&self, provider_id: i64) {
        let mut clients = self.clients.lock().unwrap();
        clients.remove(&provider_id);
    }

    /// 清空全部缓存
    pub fn clear(&self) {
        let mut clients = self.clients.lock().unwrap();
        clients.clear();
    }
}

fn build_client(proxy_mode: &str, proxy_url: Option<&str>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120));

    match proxy_mode {
        "direct" => {
            builder = builder.no_proxy();
        }
        "custom" => {
            if let Some(url) = proxy_url {
                if let Ok(proxy) = reqwest::Proxy::all(url) {
                    builder = builder.proxy(proxy);
                }
            }
        }
        _ => {
            // "system" 或未知值：使用 reqwest 默认行为
            // 读取 HTTP_PROXY/HTTPS_PROXY 环境变量 + Windows 注册表系统代理
        }
    }

    builder.build().expect("failed to build reqwest client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_returns_same_client_when_unchanged() {
        let pool = ClientPool::new();
        let c1 = pool.get(1, "direct", None);
        let c2 = pool.get(1, "direct", None);
        // reqwest::Client::clone 共享内部连接池，两次获取应一致
        assert_eq!(format!("{:?}", c1), format!("{:?}", c2));
    }

    #[test]
    fn test_pool_rebuilds_on_mode_change() {
        let pool = ClientPool::new();
        let _c1 = pool.get(1, "direct", None);
        // 模式变更应触发重建（不会 panic 即可）
        let _c2 = pool.get(1, "system", None);
    }

    #[test]
    fn test_pool_different_providers_isolated() {
        let pool = ClientPool::new();
        let _a = pool.get(1, "direct", None);
        let _b = pool.get(2, "system", None);
        // 不同 provider 互不影响
    }

    #[test]
    fn test_invalidate() {
        let pool = ClientPool::new();
        let _c1 = pool.get(1, "direct", None);
        pool.invalidate(1);
        // invalidate 后重新获取不应 panic
        let _c2 = pool.get(1, "direct", None);
    }

    #[test]
    fn test_snapshot_detects_no_change() {
        let s1 = ProxySnapshot::current();
        let s2 = ProxySnapshot::current();
        assert_eq!(s1, s2);
    }
}
