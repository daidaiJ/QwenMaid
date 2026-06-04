use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use crate::db::providers::ModelRoute;

/// 路由候选（同一 model_id 的多个供应商）
#[derive(Debug, Clone)]
pub struct RouteCandidate {
    pub route: ModelRoute,
}

/// 单个路由的实时指标（内部缓存）
#[derive(Debug, Clone)]
struct RouteMetricsInner {
    avg_latency_ms: f64,
    success_rate: f64,
    recent_failures: u32,
    disabled_until: Option<Instant>,
}

/// 加权路由器
pub struct WeightedRouter {
    /// (provider_id, model_db_id) → in-flight 计数
    in_flight: Mutex<HashMap<(i64, i64), AtomicU32>>,
    /// 路由指标缓存
    metrics: RwLock<HashMap<(i64, i64), RouteMetricsInner>>,
    /// 上次刷新时间
    last_refresh: Mutex<Instant>,
}

impl WeightedRouter {
    pub fn new() -> Self {
        Self {
            in_flight: Mutex::new(HashMap::new()),
            metrics: RwLock::new(HashMap::new()),
            last_refresh: Mutex::new(Instant::now() - Duration::from_secs(120)),
        }
    }

    /// 刷新指标缓存（从 DB 聚合最近 1 小时的数据）
    pub fn refresh_metrics(&self, conn: &Connection) {
        let mut last = self.last_refresh.lock().unwrap();
        if last.elapsed() < Duration::from_secs(60) {
            return;
        }
        *last = Instant::now();

        let mut stmt = match conn.prepare(
            "SELECT provider_id, model_id, model_id,
                    AVG(duration_ms),
                    SUM(CASE WHEN error_message IS NULL THEN 1.0 ELSE 0.0 END) / COUNT(*)
             FROM request_logs
             WHERE timestamp > datetime('now', '-1 hour')
             GROUP BY provider_id, model_id",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, f64>(3).unwrap_or(1000.0),
                    row.get::<_, f64>(4).unwrap_or(1.0),
                ))
            })
            .ok();

        if let Some(rows) = rows {
            let mut metrics = self.metrics.write().unwrap();
            for row in rows.flatten() {
                let key = (row.0, row.1);
                metrics.insert(
                    key,
                    RouteMetricsInner {
                        avg_latency_ms: row.2,
                        success_rate: row.3,
                        recent_failures: 0,
                        disabled_until: None,
                    },
                );
            }
        }
    }

    /// 记录请求开始（in_flight +1）
    pub fn request_start(&self, provider_id: i64, model_db_id: i64) {
        let mut map = self.in_flight.lock().unwrap();
        let counter = map
            .entry((provider_id, model_db_id))
            .or_insert_with(|| AtomicU32::new(0));
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录请求结束（in_flight -1）
    pub fn request_end(&self, provider_id: i64, model_db_id: i64) {
        let map = self.in_flight.lock().unwrap();
        if let Some(counter) = map.get(&(provider_id, model_db_id)) {
            counter.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// 记录请求失败（用于故障排除）
    pub fn record_failure(&self, provider_id: i64, model_db_id: i64) {
        let mut metrics = self.metrics.write().unwrap();
        let entry = metrics
            .entry((provider_id, model_db_id))
            .or_insert_with(|| RouteMetricsInner {
                avg_latency_ms: 1000.0,
                success_rate: 1.0,
                recent_failures: 0,
                disabled_until: None,
            });
        entry.recent_failures += 1;
        if entry.recent_failures >= 3 {
            entry.disabled_until = Some(Instant::now() + Duration::from_secs(60));
        }
    }

    /// 记录请求成功（重置故障计数）
    pub fn record_success(&self, provider_id: i64, model_db_id: i64) {
        let mut metrics = self.metrics.write().unwrap();
        if let Some(entry) = metrics.get_mut(&(provider_id, model_db_id)) {
            entry.recent_failures = 0;
            entry.disabled_until = None;
        }
    }

    /// 从候选列表中选择最优路由
    ///
    /// 优先级：billing_type(plan > pay_per_use) > 缓存亲和 > 负载均衡
    pub fn select_route<'a>(&self, candidates: &'a [ModelRoute]) -> Option<&'a ModelRoute> {
        if candidates.is_empty() {
            return None;
        }
        if candidates.len() == 1 {
            return Some(&candidates[0]);
        }

        // 第一层：按 billing_type 分组，plan 优先
        let plan_routes: Vec<&ModelRoute> = candidates
            .iter()
            .filter(|r| r.billing_type == "plan")
            .collect();

        let pool = if !plan_routes.is_empty() {
            &plan_routes
        } else {
            &candidates.iter().collect::<Vec<_>>()
        };

        // 第二层：在同组内，优先选有 last_success_at 的（缓存亲和）
        // 已由 SQL ORDER BY 处理，这里取第一个即可

        // 第三层：加权负载均衡
        let metrics = self.metrics.read().unwrap();
        let in_flight_map = self.in_flight.lock().unwrap();
        let now = Instant::now();

        let weights: Vec<f64> = pool
            .iter()
            .map(|c| {
                let key = (c.provider_id, c.model_db_id);

                if let Some(m) = metrics.get(&key) {
                    if let Some(until) = m.disabled_until {
                        if now < until {
                            return 0.0;
                        }
                    }
                }

                let in_flight = in_flight_map
                    .get(&key)
                    .map(|c| c.load(Ordering::Relaxed))
                    .unwrap_or(0) as f64;

                let (latency, success_rate) = match metrics.get(&key) {
                    Some(m) if m.avg_latency_ms > 0.0 => (m.avg_latency_ms, m.success_rate),
                    _ => (1000.0, 1.0),
                };

                let score =
                    success_rate * (1.0 / (latency + 1.0)) * (1.0 / (in_flight + 1.0));
                score.powi(2)
            })
            .collect();

        weighted_select(pool, &weights)
    }
}

/// 确定性加权选择（选权重最高的）
fn weighted_select<'a>(items: &[&'a ModelRoute], weights: &[f64]) -> Option<&'a ModelRoute> {
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return items.first().copied();
    }

    let best_idx = weights
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)?;

    items.get(best_idx).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_route(model_db_id: i64, provider_id: i64, billing_type: &str) -> ModelRoute {
        ModelRoute {
            model_db_id,
            provider_id,
            model_id: "test".into(),
            auth_type: r#"["openai"]"#.into(),
            is_default: false,
            config_json: None,
            provider_name: format!("p{}", provider_id),
            base_url: "https://api.test.com".into(),
            api_key_env: "KEY".into(),
            proxy_mode: "direct".into(),
            proxy_url: None,
            auth_header: None,
            billing_type: billing_type.into(),
        }
    }

    #[test]
    fn test_select_single() {
        let router = WeightedRouter::new();
        let candidates = vec![make_route(1, 1, "plan")];
        assert!(router.select_route(&candidates).is_some());
    }

    #[test]
    fn test_select_empty() {
        let router = WeightedRouter::new();
        assert!(router.select_route(&[]).is_none());
    }

    #[test]
    fn test_plan_over_pay_per_use() {
        let router = WeightedRouter::new();
        let candidates = vec![
            make_route(1, 1, "pay_per_use"),
            make_route(2, 2, "plan"),
        ];
        let selected = router.select_route(&candidates).unwrap();
        assert_eq!(selected.billing_type, "plan");
    }

    #[test]
    fn test_prefers_lower_latency() {
        let router = WeightedRouter::new();
        let candidates = vec![
            make_route(1, 1, "plan"),
            make_route(2, 2, "plan"),
        ];

        {
            let mut metrics = router.metrics.write().unwrap();
            metrics.insert(
                (1, 1),
                RouteMetricsInner {
                    avg_latency_ms: 100.0,
                    success_rate: 1.0,
                    recent_failures: 0,
                    disabled_until: None,
                },
            );
            metrics.insert(
                (2, 2),
                RouteMetricsInner {
                    avg_latency_ms: 2000.0,
                    success_rate: 1.0,
                    recent_failures: 0,
                    disabled_until: None,
                },
            );
        }

        let selected = router.select_route(&candidates).unwrap();
        assert_eq!(selected.provider_id, 1);
    }

    #[test]
    fn test_disabled_route_low_weight() {
        let router = WeightedRouter::new();
        let candidates = vec![
            make_route(1, 1, "plan"),
            make_route(2, 2, "plan"),
        ];

        {
            let mut metrics = router.metrics.write().unwrap();
            metrics.insert(
                (1, 1),
                RouteMetricsInner {
                    avg_latency_ms: 100.0,
                    success_rate: 1.0,
                    recent_failures: 3,
                    disabled_until: Some(Instant::now() + Duration::from_secs(60)),
                },
            );
            metrics.insert(
                (2, 2),
                RouteMetricsInner {
                    avg_latency_ms: 50.0,
                    success_rate: 1.0,
                    recent_failures: 0,
                    disabled_until: None,
                },
            );
        }

        let selected = router.select_route(&candidates).unwrap();
        assert_eq!(selected.provider_id, 2);
    }
}
