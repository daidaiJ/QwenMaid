use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// 获取 ~/.qwen 根目录
fn qwen_home() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".qwen")
}

// ════════════════════════════════════════════════════════════
// 增量同步：只处理新增/变更的 JSONL 文件
// ════════════════════════════════════════════════════════════

/// 扫描所有项目的 JSONL 文件，增量更新 session_stats 表
/// 返回本次新增/更新的会话数
pub fn sync_session_stats(conn: &Connection) -> Result<usize, String> {
    let projects_dir = qwen_home().join("projects");
    if !projects_dir.is_dir() {
        return Ok(0);
    }

    // 读取已有记录的 (project, session_id) → (file_size, file_mtime) 映射
    let existing = load_existing_stats(conn)?;

    // 找出 skill_calls_json 或 agent_calls_json 为空的记录，强制重新同步
    let mut force_resync = HashSet::new();
    {
        let mut stmt = conn
            .prepare("SELECT project, session_id FROM session_stats WHERE skill_calls_json = '[]' OR agent_calls_json = '[]'")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| {
            Ok(format!("{}:{}", row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }).map_err(|e| e.to_string())?;
        for row in rows.flatten() {
            force_resync.insert(row);
        }
    }

    let mut synced = 0usize;

    for entry in fs::read_dir(&projects_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }
        let project = entry.file_name().to_string_lossy().to_string();
        let chats_dir = entry.path().join("chats");
        if !chats_dir.is_dir() {
            continue;
        }

        for chat_entry in fs::read_dir(&chats_dir).map_err(|e| e.to_string())? {
            let chat_entry = chat_entry.map_err(|e| e.to_string())?;
            let path = chat_entry.path();
            if path.extension().map_or(true, |ext| ext != "jsonl") {
                continue;
            }

            let session_id = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
            let file_size = meta.len();
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH).ok()
                })
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default();

            // 检查是否需要更新
            let key = format!("{}:{}", project, session_id);
            let needs_resync = force_resync.contains(&key);
            if !needs_resync {
                if let Some((old_size, old_mtime)) = existing.get(&key) {
                    if *old_size == file_size && *old_mtime == mtime {
                        continue; // 文件未变更且无需补数据，跳过
                    }
                }
            }

            // 流式解析 JSONL
            let stats = parse_session_stats(&path);

            // UPSERT session_stats（新列容错）
            let insert_result = conn.execute(
                "INSERT INTO session_stats (
                    project, session_id, file_path, file_size, file_mtime,
                    message_count, user_messages, assistant_msgs,
                    input_tokens, output_tokens, cache_read_tokens,
                    models, tool_calls_json, skill_calls_json, agent_calls_json,
                    started_at, ended_at, duration_ms, synced_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, datetime('now'))
                ON CONFLICT(project, session_id) DO UPDATE SET
                    file_size = excluded.file_size,
                    file_mtime = excluded.file_mtime,
                    message_count = excluded.message_count,
                    user_messages = excluded.user_messages,
                    assistant_msgs = excluded.assistant_msgs,
                    input_tokens = excluded.input_tokens,
                    output_tokens = excluded.output_tokens,
                    cache_read_tokens = excluded.cache_read_tokens,
                    models = excluded.models,
                    tool_calls_json = excluded.tool_calls_json,
                    skill_calls_json = excluded.skill_calls_json,
                    agent_calls_json = excluded.agent_calls_json,
                    started_at = excluded.started_at,
                    ended_at = excluded.ended_at,
                    duration_ms = excluded.duration_ms,
                    synced_at = datetime('now')",
                rusqlite::params![
                    project, session_id, path.to_string_lossy(), file_size, mtime,
                    stats.message_count, stats.user_messages, stats.assistant_msgs,
                    stats.input_tokens, stats.output_tokens, stats.cache_read_tokens,
                    stats.models, stats.tool_calls_json, stats.skill_calls_json, stats.agent_calls_json,
                    stats.started_at, stats.ended_at, stats.duration_ms,
                ],
            );

            // 如果新列不存在，回退到旧版 INSERT
            if insert_result.is_err() {
                conn.execute(
                    "INSERT INTO session_stats (
                        project, session_id, file_path, file_size, file_mtime,
                        message_count, user_messages, assistant_msgs,
                        input_tokens, output_tokens, cache_read_tokens,
                        models, tool_calls_json,
                        started_at, ended_at, duration_ms, synced_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, datetime('now'))
                    ON CONFLICT(project, session_id) DO UPDATE SET
                        file_size = excluded.file_size,
                        file_mtime = excluded.file_mtime,
                        message_count = excluded.message_count,
                        user_messages = excluded.user_messages,
                        assistant_msgs = excluded.assistant_msgs,
                        input_tokens = excluded.input_tokens,
                        output_tokens = excluded.output_tokens,
                        cache_read_tokens = excluded.cache_read_tokens,
                        models = excluded.models,
                        tool_calls_json = excluded.tool_calls_json,
                        started_at = excluded.started_at,
                        ended_at = excluded.ended_at,
                        duration_ms = excluded.duration_ms,
                        synced_at = datetime('now')",
                    rusqlite::params![
                        project, session_id, path.to_string_lossy(), file_size, mtime,
                        stats.message_count, stats.user_messages, stats.assistant_msgs,
                        stats.input_tokens, stats.output_tokens, stats.cache_read_tokens,
                        stats.models, stats.tool_calls_json,
                        stats.started_at, stats.ended_at, stats.duration_ms,
                    ],
                ).map_err(|e| e.to_string())?;
            }

            // UPSERT model_daily_stats
            for entry in &stats.model_entries {
                conn.execute(
                    "INSERT INTO model_daily_stats (date, model, session_count, message_count, input_tokens, output_tokens, cache_read)
                     VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)
                     ON CONFLICT(date, model) DO UPDATE SET
                        session_count = session_count + 1,
                        message_count = message_count + excluded.message_count,
                        input_tokens = input_tokens + excluded.input_tokens,
                        output_tokens = output_tokens + excluded.output_tokens,
                        cache_read = cache_read + excluded.cache_read",
                    rusqlite::params![
                        entry.date, entry.model, entry.msg_count,
                        entry.input_tokens, entry.output_tokens, entry.cache_read,
                    ],
                ).map_err(|e| e.to_string())?;
            }

            synced += 1;
        }
    }

    Ok(synced)
}

fn load_existing_stats(conn: &Connection) -> Result<HashMap<String, (u64, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT project, session_id, file_size, file_mtime FROM session_stats")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? as u64,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut map = HashMap::new();
    for row in rows {
        let (project, session_id, size, mtime) = row.map_err(|e| e.to_string())?;
        map.insert(format!("{}:{}", project, session_id), (size, mtime));
    }
    Ok(map)
}

struct ParsedStats {
    message_count: usize,
    user_messages: usize,
    assistant_msgs: usize,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    models: String,
    tool_calls_json: String,
    skill_calls_json: String,
    agent_calls_json: String,
    started_at: String,
    ended_at: String,
    duration_ms: i64,
    /// 按模型的日统计 (model, date, input, output, cache_read, msg_count)
    model_entries: Vec<ModelEntry>,
}

struct ModelEntry {
    model: String,
    date: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read: u64,
    msg_count: usize,
}

fn parse_session_stats(path: &std::path::Path) -> ParsedStats {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return empty_stats(),
    };

    let reader = BufReader::new(file);
    let mut message_count = 0usize;
    let mut user_messages = 0usize;
    let mut assistant_msgs = 0usize;
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut cache_read_tokens = 0u64;
    let mut model_set = std::collections::HashSet::<String>::new();
    let mut tool_map = HashMap::<String, usize>::new();
    let mut skill_map = HashMap::<String, usize>::new();
    let mut agent_map = HashMap::<String, usize>::new();
    let mut first_ts = String::new();
    let mut last_ts = String::new();
    // 按 (model, date) 聚合
    let mut model_daily: HashMap<(String, String), ModelEntry> = HashMap::new();

    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let json: Value = match serde_json::from_str(&line) {
            Ok(v) => v, Err(_) => continue,
        };

        message_count += 1;
        let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp = json.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();

        if first_ts.is_empty() { first_ts = timestamp.clone(); }
        if !timestamp.is_empty() { last_ts = timestamp.clone(); }
        let date = timestamp.get(..10).unwrap_or("").to_string();

        match msg_type {
            "user" => user_messages += 1,
            "assistant" => {
                assistant_msgs += 1;
                let model = json.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                model_set.insert(model.clone());

                let mut msg_in = 0u64;
                let mut msg_out = 0u64;
                let mut msg_cache = 0u64;
                if let Some(usage) = json.get("usageMetadata") {
                    msg_in = usage.get("promptTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
                    msg_out = usage.get("candidatesTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
                    msg_cache = usage.get("cachedContentTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
                    input_tokens += msg_in;
                    output_tokens += msg_out;
                    cache_read_tokens += msg_cache;
                }

                // 按模型日统计
                if !date.is_empty() {
                    let key = (model.clone(), date.clone());
                    let entry = model_daily.entry(key).or_insert_with(|| ModelEntry {
                        model, date, input_tokens: 0, output_tokens: 0, cache_read: 0, msg_count: 0,
                    });
                    entry.input_tokens += msg_in;
                    entry.output_tokens += msg_out;
                    entry.cache_read += msg_cache;
                    entry.msg_count += 1;
                }

                // 统计工具调用、技能调用、子智能体调用
                if let Some(parts) = json.pointer("/message/parts").and_then(|v| v.as_array()) {
                    for part in parts {
                        if let Some(name) = part.pointer("/functionCall/name").and_then(|v| v.as_str()) {
                            match name {
                                "skill" => {
                                    if let Some(skill_name) = part.pointer("/functionCall/args/skill").and_then(|v| v.as_str()) {
                                        *skill_map.entry(skill_name.to_string()).or_insert(0) += 1;
                                    }
                                }
                                "agent" => {
                                    let agent_type = part.pointer("/functionCall/args/subagent_type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("general-purpose");
                                    *agent_map.entry(agent_type.to_string()).or_insert(0) += 1;
                                }
                                _ => {
                                    *tool_map.entry(name.to_string()).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut models: Vec<String> = model_set.into_iter().collect();
    models.sort();

    let tool_calls_vec: Vec<Value> = tool_map.into_iter()
        .map(|(name, count)| serde_json::json!({"name": name, "count": count}))
        .collect();

    let skill_calls_vec: Vec<Value> = skill_map.into_iter()
        .map(|(name, count)| serde_json::json!({"name": name, "count": count}))
        .collect();

    let agent_calls_vec: Vec<Value> = agent_map.into_iter()
        .map(|(name, count)| serde_json::json!({"name": name, "count": count}))
        .collect();

    let duration_ms = if !first_ts.is_empty() && !last_ts.is_empty() {
        chrono::NaiveDateTime::parse_from_str(&last_ts, "%Y-%m-%dT%H:%M:%S%.fZ").ok()
            .and_then(|end| {
                chrono::NaiveDateTime::parse_from_str(&first_ts, "%Y-%m-%dT%H:%M:%S%.fZ").ok()
                    .map(|start| (end - start).num_milliseconds().max(0))
            }).unwrap_or(0)
    } else { 0 };

    ParsedStats {
        message_count, user_messages, assistant_msgs,
        input_tokens, output_tokens, cache_read_tokens,
        models: models.join(", "),
        tool_calls_json: serde_json::to_string(&tool_calls_vec).unwrap_or_else(|_| "[]".into()),
        skill_calls_json: serde_json::to_string(&skill_calls_vec).unwrap_or_else(|_| "[]".into()),
        agent_calls_json: serde_json::to_string(&agent_calls_vec).unwrap_or_else(|_| "[]".into()),
        started_at: first_ts, ended_at: last_ts, duration_ms,
        model_entries: model_daily.into_values().collect(),
    }
}

fn empty_stats() -> ParsedStats {
    ParsedStats {
        message_count: 0, user_messages: 0, assistant_msgs: 0,
        input_tokens: 0, output_tokens: 0, cache_read_tokens: 0,
        models: String::new(), tool_calls_json: "[]".into(), skill_calls_json: "[]".into(), agent_calls_json: "[]".into(),
        started_at: String::new(), ended_at: String::new(), duration_ms: 0,
        model_entries: Vec::new(),
    }
}

// ════════════════════════════════════════════════════════════
// 分析汇总查询
// ════════════════════════════════════════════════════════════

#[derive(Debug, Serialize)]
pub struct AnalyticsSummary {
    pub total_sessions: usize,
    pub total_messages: usize,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read: u64,
    pub active_days: usize,
    pub top_models: Vec<ModelRanking>,
    pub top_tools: Vec<NameCount>,
    pub top_skills: Vec<NameCount>,
    pub top_agents: Vec<NameCount>,
    pub project_stats: Vec<ProjectStats>,
    pub daily: Vec<DailyStats>,
    pub model_daily: Vec<ModelDailyRow>,
}

#[derive(Debug, Serialize)]
pub struct NameCount {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct ModelRanking {
    pub name: String,
    pub session_count: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read: u64,
    pub cache_hit_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct ProjectStats {
    pub project: String,
    pub session_count: usize,
    pub total_messages: usize,
    pub total_tokens: u64,
}

#[derive(Debug, Serialize)]
pub struct DailyStats {
    pub date: String,
    pub session_count: usize,
    pub message_count: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Serialize)]
pub struct ModelDailyRow {
    pub date: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read: u64,
    pub message_count: usize,
}

pub fn get_analytics_summary(conn: &Connection) -> Result<AnalyticsSummary, String> {
    // 总计
    let (total_sessions, total_messages, total_input, total_output, total_cache): (usize, usize, u64, u64, u64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(message_count), 0), COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0), COALESCE(SUM(cache_read_tokens), 0) FROM session_stats",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .map_err(|e| e.to_string())?;

    let active_days: usize = conn
        .query_row(
            "SELECT COUNT(DISTINCT date(started_at)) FROM session_stats WHERE started_at IS NOT NULL",
            [], |row| row.get(0),
        ).map_err(|e| e.to_string())?;

    // 按项目统计
    let mut stmt = conn.prepare(
        "SELECT project, COUNT(*), COALESCE(SUM(message_count), 0), COALESCE(SUM(input_tokens + output_tokens), 0)
         FROM session_stats GROUP BY project ORDER BY COUNT(*) DESC LIMIT 5",
    ).map_err(|e| e.to_string())?;
    let project_stats: Vec<ProjectStats> = stmt.query_map([], |row| {
        Ok(ProjectStats { project: row.get(0)?, session_count: row.get(1)?, total_messages: row.get(2)?, total_tokens: row.get(3)? })
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // 按天统计（最近 60 天）
    let mut stmt = conn.prepare(
        "SELECT date(started_at), COUNT(*), COALESCE(SUM(message_count), 0),
                COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
         FROM session_stats WHERE started_at IS NOT NULL
         GROUP BY date(started_at) ORDER BY date(started_at) DESC LIMIT 60",
    ).map_err(|e| e.to_string())?;
    let daily: Vec<DailyStats> = stmt.query_map([], |row| {
        Ok(DailyStats { date: row.get(0)?, session_count: row.get(1)?, message_count: row.get(2)?, input_tokens: row.get(3)?, output_tokens: row.get(4)? })
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // 模型排名（从 model_daily_stats 聚合）
    let mut stmt = conn.prepare(
        "SELECT model, SUM(session_count), SUM(input_tokens), SUM(output_tokens), SUM(cache_read)
         FROM model_daily_stats GROUP BY model ORDER BY SUM(input_tokens + output_tokens) DESC LIMIT 10",
    ).map_err(|e| e.to_string())?;
    let top_models: Vec<ModelRanking> = stmt.query_map([], |row| {
        let model: String = row.get(0)?;
        let sessions: i64 = row.get(1)?;
        let inp: i64 = row.get(2)?;
        let out: i64 = row.get(3)?;
        let cache: i64 = row.get(4)?;
        let total = inp + out;
        let hit_rate = if total > 0 { cache as f64 / (inp as f64).max(1.0) } else { 0.0 };
        Ok(ModelRanking {
            name: model, session_count: sessions as usize,
            input_tokens: inp as u64, output_tokens: out as u64,
            cache_read: cache as u64, cache_hit_rate: hit_rate,
        })
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // 模型日统计（最近 30 天）
    let mut stmt = conn.prepare(
        "SELECT date, model, input_tokens, output_tokens, cache_read, message_count
         FROM model_daily_stats ORDER BY date DESC LIMIT 200",
    ).map_err(|e| e.to_string())?;
    let model_daily: Vec<ModelDailyRow> = stmt.query_map([], |row| {
        Ok(ModelDailyRow {
            date: row.get(0)?, model: row.get(1)?,
            input_tokens: row.get::<_, i64>(2)? as u64,
            output_tokens: row.get::<_, i64>(3)? as u64,
            cache_read: row.get::<_, i64>(4)? as u64,
            message_count: row.get::<_, i64>(5)? as usize,
        })
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // Top 工具
    let all_tools: Vec<String> = conn
        .prepare("SELECT tool_calls_json FROM session_stats WHERE tool_calls_json != '[]'")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut tool_map = HashMap::<String, usize>::new();
    for tj in &all_tools {
        if let Ok(arr) = serde_json::from_str::<Vec<Value>>(tj) {
            for item in arr {
                if let (Some(name), Some(count)) = (
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("count").and_then(|v| v.as_u64()),
                ) {
                    *tool_map.entry(name.to_string()).or_insert(0) += count as usize;
                }
            }
        }
    }
    let mut top_tools: Vec<NameCount> = tool_map.into_iter()
        .map(|(name, count)| NameCount { name, count }).collect();
    top_tools.sort_by(|a, b| b.count.cmp(&a.count));
    top_tools.truncate(10);

    // Top 技能（容错：列可能不存在）
    let all_skills: Vec<String> = conn
        .prepare("SELECT skill_calls_json FROM session_stats WHERE skill_calls_json != '[]'")
        .and_then(|mut stmt| {
            Ok(stmt.query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let mut skill_map = HashMap::<String, usize>::new();
    for sj in &all_skills {
        if let Ok(arr) = serde_json::from_str::<Vec<Value>>(sj) {
            for item in arr {
                if let (Some(name), Some(count)) = (item.get("name").and_then(|v| v.as_str()), item.get("count").and_then(|v| v.as_u64())) {
                    *skill_map.entry(name.to_string()).or_insert(0) += count as usize;
                }
            }
        }
    }
    let mut top_skills: Vec<NameCount> = skill_map.into_iter()
        .map(|(name, count)| NameCount { name, count }).collect();
    top_skills.sort_by(|a, b| b.count.cmp(&a.count));
    top_skills.truncate(10);

    // Top 子智能体（容错：列可能不存在）
    let all_agents: Vec<String> = conn
        .prepare("SELECT agent_calls_json FROM session_stats WHERE agent_calls_json != '[]'")
        .and_then(|mut stmt| {
            Ok(stmt.query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let mut agent_map = HashMap::<String, usize>::new();
    for aj in &all_agents {
        if let Ok(arr) = serde_json::from_str::<Vec<Value>>(aj) {
            for item in arr {
                if let (Some(name), Some(count)) = (item.get("name").and_then(|v| v.as_str()), item.get("count").and_then(|v| v.as_u64())) {
                    *agent_map.entry(name.to_string()).or_insert(0) += count as usize;
                }
            }
        }
    }
    let mut top_agents: Vec<NameCount> = agent_map.into_iter()
        .map(|(name, count)| NameCount { name, count }).collect();
    top_agents.sort_by(|a, b| b.count.cmp(&a.count));
    top_agents.truncate(10);

    Ok(AnalyticsSummary {
        total_sessions, total_messages,
        total_input_tokens: total_input, total_output_tokens: total_output,
        total_cache_read: total_cache, active_days,
        top_models, top_tools, top_skills, top_agents, project_stats, daily, model_daily,
    })
}
