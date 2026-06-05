use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

fn usage_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".qwen/usage/usage.db")
}

#[derive(Debug, Serialize)]
pub struct UsageDbInfo {
    pub exists: bool,
    pub tables: Vec<String>,
    pub call_records_columns: Vec<String>,
    pub call_records_count: i64,
    pub sample_row: Option<String>,
}

#[tauri::command]
pub fn check_usage_db() -> UsageDbInfo {
    let path = usage_db_path();
    if !path.exists() {
        return UsageDbInfo {
            exists: false,
            tables: vec![],
            call_records_columns: vec![],
            call_records_count: 0,
            sample_row: None,
        };
    }
    let conn = match Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(e) => return UsageDbInfo {
            exists: true,
            tables: vec![format!("error opening: {}", e)],
            call_records_columns: vec![],
            call_records_count: 0,
            sample_row: None,
        },
    };

    // 列出所有表
    let tables: Vec<String> = {
        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").unwrap();
        stmt.query_map([], |row| row.get::<_, String>(0)).unwrap().filter_map(|r| r.ok()).collect()
    };

    // 检查 call_records 表的列
    let (columns, count, sample) = if tables.iter().any(|t| t == "call_records") {
        let cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(call_records)").unwrap();
            stmt.query_map([], |row| row.get::<_, String>(1)).unwrap().filter_map(|r| r.ok()).collect()
        };
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM call_records", [], |row| row.get(0)).unwrap_or(0);
        let sample = conn.query_row("SELECT * FROM call_records LIMIT 1", [], |row| {
            let mut map = serde_json::Map::new();
            for (i, col) in cols.iter().enumerate() {
                if let Ok(val) = row.get::<_, String>(i) {
                    map.insert(col.clone(), serde_json::Value::String(val));
                } else if let Ok(val) = row.get::<_, f64>(i) {
                    map.insert(col.clone(), serde_json::json!(val));
                } else if let Ok(val) = row.get::<_, i64>(i) {
                    map.insert(col.clone(), serde_json::json!(val));
                }
            }
            Ok(serde_json::to_string_pretty(&map).unwrap_or_default())
        }).ok();
        (cols, count, sample)
    } else {
        (vec![], 0, None)
    };

    UsageDbInfo {
        exists: true,
        tables,
        call_records_columns: columns,
        call_records_count: count,
        sample_row: sample,
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ModelMeta {
    pub model: String,
    pub total_requests: i64,
    pub total_input: i64,
    pub total_output: i64,
    pub total_cache: i64,
    pub avg_tps: f64,
    pub p50_latency: f64,
    pub p95_latency: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ModelDailyDetail {
    pub date: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read: i64,
    pub uncached_input: i64,
    pub avg_tps: f64,
    pub avg_latency: f64,
    pub p50_latency: f64,
    pub p95_latency: f64,
    pub request_count: i64,
}

#[derive(Debug, Serialize)]
pub struct ModelDetailData {
    pub models: Vec<ModelMeta>,
    pub daily: Vec<ModelDailyDetail>,
}

/// 从 request_logs 读取代理层 token 数据
fn query_request_logs(
    conn: &Connection,
    days: u32,
) -> Result<Vec<(String, String, i64, i64, i64, i64)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT date(timestamp), model_id,
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(cache_read_tokens), 0),
                    COUNT(*)
             FROM request_logs
             WHERE timestamp >= datetime('now', '-' || ?1 || ' days')
               AND model_id IS NOT NULL AND model_id != ''
             GROUP BY date(timestamp), model_id",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![days], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// 单日单模型的 call_records 聚合数据
struct CallRecordDay {
    latencies: Vec<f64>,
    tps_sum: f64,
    tps_count: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    cached_tokens: i64,
}

/// 从 usage.db 的 call_records 读取性能+Token 数据
fn query_call_records(days: u32) -> Result<HashMap<(String, String), CallRecordDay>, String> {
    let path = usage_db_path();
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let uconn =
        Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("open usage.db: {}", e))?;

    // 探测 call_records 表是否存在
    let table_exists: bool = uconn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='call_records'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !table_exists {
        return Ok(HashMap::new());
    }

    // 探测实际列名
    let columns: Vec<String> = {
        let mut stmt = uconn.prepare("PRAGMA table_info(call_records)").map_err(|e| e.to_string())?;
        let mut cols = Vec::new();
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            if let Ok(name) = row.get::<_, String>(1) {
                cols.push(name);
            }
        }
        cols
    };

    // 找到最匹配的列名
    let date_col = find_col(&columns, &["recorded_at", "created_at", "timestamp", "date"]);
    let model_col = find_col(&columns, &["model_name", "model", "model_id"]);
    let latency_col = find_col(&columns, &["latency_ms", "latency", "duration_ms", "duration"]);
    let total_tok_col = find_col(&columns, &["total_tokens", "tokens"]);
    let prompt_tok_col = find_col(&columns, &["prompt_tokens", "input_tokens"]);
    let completion_tok_col = find_col(&columns, &["completion_tokens", "output_tokens"]);
    let cached_tok_col = find_col(&columns, &["cached_tokens", "cache_read_tokens"]);

    // 构建动态查询
    let date_expr = match date_col.as_deref() {
        Some("recorded_at") => "SUBSTR(recorded_at, 1, 10)".to_string(),
        Some(c) => format!("DATE({})", c),
        None => "NULL".to_string(),
    };
    let model_expr = model_col.as_deref().unwrap_or("NULL").to_string();
    let latency_expr = latency_col.as_deref().unwrap_or("NULL").to_string();
    let tps_expr = match (&total_tok_col, &latency_col) {
        (Some(tok), Some(lat)) => format!("{} * 1000.0 / MAX({}, 1)", tok, lat),
        _ => "0".to_string(),
    };
    let prompt_expr = prompt_tok_col.as_deref().unwrap_or("0").to_string();
    let completion_expr = completion_tok_col.as_deref().unwrap_or("0").to_string();
    let cached_expr = cached_tok_col.as_deref().unwrap_or("0").to_string();

    let sql = format!(
        "SELECT {}, {}, {}, {}, {}, {}, {} FROM call_records WHERE SUBSTR({}, 1, 10) >= SUBSTR(datetime('now', '-' || ?1 || ' days'), 1, 10)",
        date_expr, model_expr, latency_expr, tps_expr,
        prompt_expr, completion_expr, cached_expr,
        date_col.as_deref().unwrap_or("recorded_at")
    );

    let mut stmt = uconn.prepare(&sql).map_err(|e| format!("prepare: {} (cols: {:?})", e, columns))?;

    let mut result: HashMap<(String, String), CallRecordDay> = HashMap::new();

    let rows = stmt
        .query_map(rusqlite::params![days], |row| {
            Ok((
                row.get::<_, String>(0).unwrap_or_default(),
                row.get::<_, String>(1).unwrap_or_default(),
                row.get::<_, f64>(2).unwrap_or(0.0),
                row.get::<_, f64>(3).unwrap_or(0.0),
                row.get::<_, f64>(4).unwrap_or(0.0) as i64,
                row.get::<_, f64>(5).unwrap_or(0.0) as i64,
                row.get::<_, f64>(6).unwrap_or(0.0) as i64,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows.flatten() {
        let (date, model, latency, tps, prompt, completion, cached) = row;
        if date.is_empty() || model.is_empty() { continue; }
        let key = (date, model);
        let entry = result.entry(key).or_insert(CallRecordDay {
            latencies: Vec::new(), tps_sum: 0.0, tps_count: 0,
            prompt_tokens: 0, completion_tokens: 0, cached_tokens: 0,
        });
        if latency > 0.0 { entry.latencies.push(latency); }
        entry.tps_sum += tps;
        entry.tps_count += 1;
        entry.prompt_tokens += prompt;
        entry.completion_tokens += completion;
        entry.cached_tokens += cached;
    }

    Ok(result)
}

/// 从列名列表中找到第一个匹配的列
fn find_col(columns: &[String], candidates: &[&str]) -> Option<String> {
    for c in candidates {
        if columns.iter().any(|col| col.eq_ignore_ascii_case(c)) {
            return Some(c.to_string());
        }
    }
    None
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64) * p / 100.0).floor() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[tauri::command]
pub fn get_model_detail_stats(
    state: tauri::State<'_, super::AppState>,
    days: u32,
) -> Result<ModelDetailData, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // 1. request_logs 数据
    let rl_data = query_request_logs(&db, days)?;

    // 2. usage.db 性能数据
    let perf_data = query_call_records(days)?;

    // 3. 合并：按 (date, model) 构建 daily 列表
    let mut daily_map: HashMap<(String, String), ModelDailyDetail> = HashMap::new();

    for (date, model, inp, out, cache, count) in &rl_data {
        let key = (date.clone(), model.clone());
        let entry = daily_map.entry(key).or_insert(ModelDailyDetail {
            date: date.clone(),
            model: model.clone(),
            input_tokens: 0,
            output_tokens: 0,
            cache_read: 0,
            uncached_input: 0,
            avg_tps: 0.0,
            avg_latency: 0.0,
            p50_latency: 0.0,
            p95_latency: 0.0,
            request_count: 0,
        });
        entry.input_tokens = *inp;
        entry.output_tokens = *out;
        entry.cache_read = *cache;
        entry.uncached_input = (inp - cache).max(0);
        entry.request_count = *count;
    }

    // 填充性能+Token数据（来自 usage.db）
    for ((date, model), cr) in &perf_data {
        let key = (date.clone(), model.clone());
        let entry = daily_map.entry(key).or_insert(ModelDailyDetail {
            date: date.clone(),
            model: model.clone(),
            input_tokens: 0, output_tokens: 0, cache_read: 0, uncached_input: 0,
            avg_tps: 0.0, avg_latency: 0.0, p50_latency: 0.0, p95_latency: 0.0,
            request_count: 0,
        });
        // Token 数据：仅当 request_logs 没提供时才用 usage.db 的
        if entry.input_tokens == 0 && entry.output_tokens == 0 {
            entry.input_tokens = cr.prompt_tokens as i64;
            entry.output_tokens = cr.completion_tokens as i64;
            entry.cache_read = cr.cached_tokens as i64;
            entry.uncached_input = (cr.prompt_tokens - cr.cached_tokens).max(0) as i64;
        }
        if entry.request_count == 0 {
            entry.request_count = cr.tps_count;
        }
        entry.avg_tps = if cr.tps_count > 0 { cr.tps_sum / cr.tps_count as f64 } else { 0.0 };
        if !cr.latencies.is_empty() {
            let mut sorted = cr.latencies.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            entry.avg_latency = sorted.iter().sum::<f64>() / sorted.len() as f64;
            entry.p50_latency = percentile(&sorted, 50.0);
            entry.p95_latency = percentile(&sorted, 95.0);
        }
    }

    // 4. 聚合 ModelMeta
    let mut meta_map: HashMap<String, ModelMeta> = HashMap::new();
    for d in daily_map.values() {
        let entry = meta_map.entry(d.model.clone()).or_insert(ModelMeta {
            model: d.model.clone(),
            total_requests: 0,
            total_input: 0,
            total_output: 0,
            total_cache: 0,
            avg_tps: 0.0,
            p50_latency: 0.0,
            p95_latency: 0.0,
        });
        entry.total_requests += d.request_count;
        entry.total_input += d.input_tokens;
        entry.total_output += d.output_tokens;
        entry.total_cache += d.cache_read;
    }

    // 性能汇总
    let mut model_latencies: HashMap<String, Vec<f64>> = HashMap::new();
    let mut model_tps: HashMap<String, Vec<f64>> = HashMap::new();
    for d in daily_map.values() {
        if d.p50_latency > 0.0 {
            model_latencies
                .entry(d.model.clone())
                .or_default()
                .push(d.avg_latency);
        }
        if d.avg_tps > 0.0 {
            model_tps
                .entry(d.model.clone())
                .or_default()
                .push(d.avg_tps);
        }
    }
    for (model, meta) in meta_map.iter_mut() {
        if let Some(lats) = model_latencies.get(model) {
            let mut sorted = lats.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            meta.p50_latency = percentile(&sorted, 50.0);
            meta.p95_latency = percentile(&sorted, 95.0);
        }
        if let Some(tps_vals) = model_tps.get(model) {
            meta.avg_tps = tps_vals.iter().sum::<f64>() / tps_vals.len() as f64;
        }
    }

    let mut models: Vec<ModelMeta> = meta_map.into_values().collect();
    models.sort_by(|a, b| {
        (b.total_input + b.total_output).cmp(&(a.total_input + a.total_output))
    });

    let mut daily: Vec<ModelDailyDetail> = daily_map.into_values().collect();
    daily.sort_by(|a, b| b.date.cmp(&a.date).then(a.model.cmp(&b.model)));

    Ok(ModelDetailData { models, daily })
}

/// 从 request_logs 查询代理层详情数据（含延迟 + TPS）
#[tauri::command]
pub fn get_proxy_detail_stats(
    state: tauri::State<'_, super::AppState>,
    days: u32,
) -> Result<ModelDetailData, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // 查询 request_logs 的 token + 延迟数据
    let mut stmt = db
        .prepare(
            "SELECT date(timestamp), model_id,
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(cache_read_tokens), 0),
                    COUNT(*),
                    COALESCE(SUM(duration_ms), 0),
                    GROUP_CONCAT(duration_ms)
             FROM request_logs
             WHERE timestamp >= datetime('now', '-' || ?1 || ' days')
               AND model_id IS NOT NULL AND model_id != ''
               AND status_code >= 200 AND status_code < 400
             GROUP BY date(timestamp), model_id",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<(String, String, i64, i64, i64, i64, i64, Option<String>)> = stmt
        .query_map(rusqlite::params![days], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut daily_map: HashMap<(String, String), ModelDailyDetail> = HashMap::new();

    for (date, model, inp, out, cache, count, total_duration, durations_str) in &rows {
        let key = (date.clone(), model.clone());
        let entry = daily_map.entry(key).or_insert(ModelDailyDetail {
            date: date.clone(),
            model: model.clone(),
            input_tokens: 0,
            output_tokens: 0,
            cache_read: 0,
            uncached_input: 0,
            avg_tps: 0.0,
            avg_latency: 0.0,
            p50_latency: 0.0,
            p95_latency: 0.0,
            request_count: 0,
        });
        entry.input_tokens = *inp;
        entry.output_tokens = *out;
        entry.cache_read = *cache;
        entry.uncached_input = (inp - cache).max(0);
        entry.request_count = *count;

        // TPS: output_tokens / total_duration * 1000
        if *total_duration > 0 && *out > 0 {
            entry.avg_tps = (*out as f64) / (*total_duration as f64) * 1000.0;
        }

        // 延迟统计：从 GROUP_CONCAT 的 duration_ms 列表计算 P50/P95
        if let Some(dur_str) = durations_str {
            let mut latencies: Vec<f64> = dur_str
                .split(',')
                .filter_map(|s| s.parse::<f64>().ok())
                .filter(|&v| v > 0.0)
                .collect();
            if !latencies.is_empty() {
                latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                entry.avg_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
                entry.p50_latency = percentile(&latencies, 50.0);
                entry.p95_latency = percentile(&latencies, 95.0);
            }
        }
    }

    // 聚合 ModelMeta
    let mut meta_map: HashMap<String, ModelMeta> = HashMap::new();
    for d in daily_map.values() {
        let entry = meta_map.entry(d.model.clone()).or_insert(ModelMeta {
            model: d.model.clone(),
            total_requests: 0,
            total_input: 0,
            total_output: 0,
            total_cache: 0,
            avg_tps: 0.0,
            p50_latency: 0.0,
            p95_latency: 0.0,
        });
        entry.total_requests += d.request_count;
        entry.total_input += d.input_tokens;
        entry.total_output += d.output_tokens;
        entry.total_cache += d.cache_read;
    }

    // 性能汇总
    let mut model_latencies: HashMap<String, Vec<f64>> = HashMap::new();
    let mut model_tps: HashMap<String, Vec<f64>> = HashMap::new();
    for d in daily_map.values() {
        if d.p50_latency > 0.0 {
            model_latencies.entry(d.model.clone()).or_default().push(d.avg_latency);
        }
        if d.avg_tps > 0.0 {
            model_tps.entry(d.model.clone()).or_default().push(d.avg_tps);
        }
    }
    for (model, meta) in meta_map.iter_mut() {
        if let Some(lats) = model_latencies.get(model) {
            let mut sorted = lats.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            meta.p50_latency = percentile(&sorted, 50.0);
            meta.p95_latency = percentile(&sorted, 95.0);
        }
        if let Some(tps_vals) = model_tps.get(model) {
            meta.avg_tps = tps_vals.iter().sum::<f64>() / tps_vals.len() as f64;
        }
    }

    let mut models: Vec<ModelMeta> = meta_map.into_values().collect();
    models.sort_by(|a, b| (b.total_input + b.total_output).cmp(&(a.total_input + a.total_output)));

    let mut daily: Vec<ModelDailyDetail> = daily_map.into_values().collect();
    daily.sort_by(|a, b| b.date.cmp(&a.date).then(a.model.cmp(&b.model)));

    Ok(ModelDetailData { models, daily })
}
