use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

fn qwen_home() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".qwen")
}

// ════════════════════════════════════════════════════════════
// 增量同步：只处理文件大小有变化的 JSONL，且只解析新增行
// ════════════════════════════════════════════════════════════

/// 同步入口：逐文件解析+写入，每写完一个文件就释放锁，前端随时可读
pub fn sync_session_stats(conn: &Connection) -> Result<usize, String> {
    let projects_dir = qwen_home().join("projects");
    if !projects_dir.is_dir() {
        return Ok(0);
    }

    let existing = load_existing_stats(conn)?;

    let mut force_resync = HashSet::new();
    {
        let mut stmt = conn
            .prepare("SELECT project, session_id FROM session_stats WHERE input_tokens = 0 OR skill_calls_json = '[]' OR agent_calls_json = '[]'")
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
        if !entry.path().is_dir() { continue; }
        let project = entry.file_name().to_string_lossy().to_string();
        let chats_dir = entry.path().join("chats");
        if !chats_dir.is_dir() { continue; }

        for chat_entry in fs::read_dir(&chats_dir).map_err(|e| e.to_string())? {
            let chat_entry = chat_entry.map_err(|e| e.to_string())?;
            let path = chat_entry.path();
            if path.extension().map_or(true, |ext| ext != "jsonl") { continue; }

            let session_id = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
            let file_size = meta.len();
            let mtime = meta.modified().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default();

            let key = format!("{}:{}", project, session_id);
            let needs_resync = force_resync.contains(&key);

            // 判断是否需要解析
            let should_parse = if needs_resync {
                true
            } else if let Some((old_size, old_mtime, _old_parsed_lines, _)) = existing.get(&key) {
                if *old_size == file_size && *old_mtime == mtime {
                    false
                } else {
                    true
                }
            } else {
                true
            };

            if !should_parse { continue; }

            // 解析不持锁（文件 IO）
            let old_parsed = existing.get(&key).and_then(|(_, _, pl, _)| if file_size > 0 { Some(*pl) } else { None });
            let stats = if let Some(pl) = old_parsed {
                if file_size > pl as u64 * 100 { // 粗略判断：文件确实增长了
                    incremental_parse(&path, pl)
                } else {
                    parse_session_stats(&path, 0)
                }
            } else {
                parse_session_stats(&path, 0)
            };

            // 写入 DB（持锁，但每个文件单独写，很快释放）
            upsert_session(conn, &project, &session_id, &path, file_size, &mtime, &stats);
            synced += 1;
        }
    }

    Ok(synced)
}

/// 增量解析：只解析 skip_lines 之后的新增行，统计数据与已有记录合并
fn incremental_parse(path: &std::path::Path, skip_lines: usize) -> ParsedStats {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return empty_stats(),
    };
    let mut reader = BufReader::new(file);

    // 跳过已解析的行（只读字节，不做 JSON 解析）
    let mut skipped = 0usize;
    let mut skip_buf = String::new();
    while skipped < skip_lines {
        skip_buf.clear();
        match reader.read_line(&mut skip_buf) {
            Ok(0) => return empty_stats(), // EOF
            Ok(_) => skipped += 1,
            Err(_) => return empty_stats(),
        }
    }

    // 只解析新增行
    let mut stats = empty_stats();
    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let json: Value = match serde_json::from_str(&line) {
            Ok(v) => v, Err(_) => continue,
        };
        parse_one_line(&json, &mut stats);
    }
    // title 和 first_line 在增量模式下不需要重新提取
    stats.parsed_lines = skip_lines + (stats.message_count);
    stats
}

/// 将统计结果写入/更新 DB
fn upsert_session(
    conn: &Connection, project: &str, session_id: &str,
    path: &std::path::Path, file_size: u64, mtime: &str, stats: &ParsedStats,
) {
    // 主 UPSERT（包含新列）
    let r = conn.execute(
        "INSERT INTO session_stats (
            project, session_id, file_path, file_size, file_mtime,
            message_count, user_messages, assistant_msgs,
            input_tokens, output_tokens, cache_read_tokens,
            models, tool_calls_json, skill_calls_json, agent_calls_json,
            started_at, ended_at, duration_ms, synced_at,
            parsed_lines, title
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,datetime('now'),?19,?20)
        ON CONFLICT(project, session_id) DO UPDATE SET
            file_size=excluded.file_size, file_mtime=excluded.file_mtime,
            message_count=excluded.message_count, user_messages=excluded.user_messages,
            assistant_msgs=excluded.assistant_msgs, input_tokens=excluded.input_tokens,
            output_tokens=excluded.output_tokens, cache_read_tokens=excluded.cache_read_tokens,
            models=excluded.models, tool_calls_json=excluded.tool_calls_json,
            skill_calls_json=excluded.skill_calls_json, agent_calls_json=excluded.agent_calls_json,
            started_at=excluded.started_at, ended_at=excluded.ended_at,
            duration_ms=excluded.duration_ms, synced_at=datetime('now'),
            parsed_lines=excluded.parsed_lines,
            title=CASE WHEN excluded.title != '' THEN excluded.title ELSE title END",
        rusqlite::params![
            project, session_id, path.to_string_lossy(), file_size, mtime,
            stats.message_count, stats.user_messages, stats.assistant_msgs,
            stats.input_tokens, stats.output_tokens, stats.cache_read_tokens,
            stats.models, stats.tool_calls_json, stats.skill_calls_json, stats.agent_calls_json,
            stats.started_at, stats.ended_at, stats.duration_ms,
            stats.parsed_lines, stats.title,
        ],
    );

    // 旧版 DB 回退（无 parsed_lines/title 列）
    if r.is_err() {
        let _ = conn.execute(
            "INSERT INTO session_stats (
                project, session_id, file_path, file_size, file_mtime,
                message_count, user_messages, assistant_msgs,
                input_tokens, output_tokens, cache_read_tokens,
                models, tool_calls_json, skill_calls_json, agent_calls_json,
                started_at, ended_at, duration_ms, synced_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,datetime('now'))
            ON CONFLICT(project, session_id) DO UPDATE SET
                file_size=excluded.file_size, file_mtime=excluded.file_mtime,
                message_count=excluded.message_count, user_messages=excluded.user_messages,
                assistant_msgs=excluded.assistant_msgs, input_tokens=excluded.input_tokens,
                output_tokens=excluded.output_tokens, cache_read_tokens=excluded.cache_read_tokens,
                models=excluded.models, tool_calls_json=excluded.tool_calls_json,
                skill_calls_json=excluded.skill_calls_json, agent_calls_json=excluded.agent_calls_json,
                started_at=excluded.started_at, ended_at=excluded.ended_at,
                duration_ms=excluded.duration_ms, synced_at=datetime('now')",
            rusqlite::params![
                project, session_id, path.to_string_lossy(), file_size, mtime,
                stats.message_count, stats.user_messages, stats.assistant_msgs,
                stats.input_tokens, stats.output_tokens, stats.cache_read_tokens,
                stats.models, stats.tool_calls_json, stats.skill_calls_json, stats.agent_calls_json,
                stats.started_at, stats.ended_at, stats.duration_ms,
            ],
        );
    }

    // model_daily_stats
    for entry in &stats.model_entries {
        let _ = conn.execute(
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
        );
    }
}

fn load_existing_stats(conn: &Connection) -> Result<HashMap<String, (u64, String, usize, String)>, String> {
    // 尝试带 parsed_lines/title 的查询
    let result = conn.prepare(
        "SELECT project, session_id, file_size, file_mtime, COALESCE(parsed_lines, 0), COALESCE(title, '') FROM session_stats"
    );
    let mut stmt = match result {
        Ok(s) => s,
        Err(_) => {
            // 旧版 DB 回退
            let mut s = conn.prepare(
                "SELECT project, session_id, file_size, file_mtime FROM session_stats"
            ).map_err(|e| e.to_string())?;
            let rows = s.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?, row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? as u64, row.get::<_, String>(3)?,
                    0usize, String::new(),
                ))
            }).map_err(|e| e.to_string())?;
            let mut map = HashMap::new();
            for row in rows {
                let (p, s, sz, mt, pl, t) = row.map_err(|e| e.to_string())?;
                map.insert(format!("{}:{}", p, s), (sz, mt, pl, t));
            }
            return Ok(map);
        }
    };
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?, row.get::<_, String>(1)?,
            row.get::<_, i64>(2)? as u64, row.get::<_, String>(3)?,
            row.get::<_, i64>(4)? as usize, row.get::<_, String>(5)?,
        ))
    }).map_err(|e| e.to_string())?;
    let mut map = HashMap::new();
    for row in rows {
        let (p, s, sz, mt, pl, t) = row.map_err(|e| e.to_string())?;
        map.insert(format!("{}:{}", p, s), (sz, mt, pl, t));
    }
    Ok(map)
}

// ════════════════════════════════════════════════════════════
// 解析核心：逐行提取统计信息（不反序列化整个结构）
// ════════════════════════════════════════════════════════════

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
    title: String,
    parsed_lines: usize,
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

/// 从单行 JSON 提取统计信息，累加到 stats
fn parse_one_line(json: &Value, stats: &mut ParsedStats) {
    stats.message_count += 1;
    let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let timestamp = json.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();

    if stats.started_at.is_empty() { stats.started_at = timestamp.clone(); }
    if !timestamp.is_empty() { stats.ended_at = timestamp.clone(); }
    let date = timestamp.get(..10).unwrap_or("").to_string();

    match msg_type {
        "user" => {
            stats.user_messages += 1;
            // 提取 title（第一条用户消息的前 80 字符）
            if stats.title.is_empty() {
                if let Some(text) = json.pointer("/message/parts").and_then(|v| v.as_array())
                    .and_then(|arr| arr.iter().find_map(|p| p.get("text").and_then(|v| v.as_str())))
                {
                    stats.title = text.chars().take(80).collect();
                }
            }
        }
        "assistant" => {
            stats.assistant_msgs += 1;
            let model = json.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            if !stats.models.contains(&model) {
                if stats.models.is_empty() { stats.models = model.clone(); }
                else { stats.models.push_str(", "); stats.models.push_str(&model); }
            }

            let mut msg_in = 0u64;
            let mut msg_out = 0u64;
            let mut msg_cache = 0u64;
            if let Some(usage) = json.get("usageMetadata") {
                msg_in = usage.get("promptTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
                msg_out = usage.get("candidatesTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
                msg_cache = usage.get("cachedContentTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
                stats.input_tokens += msg_in;
                stats.output_tokens += msg_out;
                stats.cache_read_tokens += msg_cache;
            }

            if !date.is_empty() {
                let entry = stats.model_entries.iter_mut().find(|e| e.model == model && e.date == date);
                if let Some(e) = entry {
                    e.input_tokens += msg_in; e.output_tokens += msg_out;
                    e.cache_read += msg_cache; e.msg_count += 1;
                } else {
                    stats.model_entries.push(ModelEntry {
                        model, date, input_tokens: msg_in, output_tokens: msg_out,
                        cache_read: msg_cache, msg_count: 1,
                    });
                }
            }

            if let Some(parts) = json.pointer("/message/parts").and_then(|v| v.as_array()) {
                for part in parts {
                    if let Some(name) = part.pointer("/functionCall/name").and_then(|v| v.as_str()) {
                        // 用简单的字符串标记存储，最后再汇总
                        match name {
                            "skill" => {
                                if let Some(sn) = part.pointer("/functionCall/args/skill").and_then(|v| v.as_str()) {
                                    let tag = format!("S:{}", sn);
                                    append_tag(&mut stats.skill_calls_json, &tag);
                                }
                            }
                            "agent" => {
                                let at = part.pointer("/functionCall/args/subagent_type")
                                    .and_then(|v| v.as_str()).unwrap_or("general-purpose");
                                let tag = format!("A:{}", at);
                                append_tag(&mut stats.agent_calls_json, &tag);
                            }
                            _ => {
                                let tag = format!("T:{}", name);
                                append_tag(&mut stats.tool_calls_json, &tag);
                            }
                        }
                    }
                }
            }
        }
        "system" => {
            // 提取自定义标题
            if json.get("subtype").and_then(|v| v.as_str()) == Some("custom_title") {
                if let Some(t) = json.get("title").and_then(|v| v.as_str()) {
                    if t.len() > stats.title.len() {
                        stats.title = t.to_string();
                    }
                }
            }
        }
        _ => {}
    }
}

/// 临时用逗号分隔存储调用标记，finalize 时转为 JSON
fn append_tag(field: &mut String, tag: &str) {
    if field.is_empty() || field == "[]" {
        field.clear();
        field.push_str(tag);
    } else {
        field.push(',');
        field.push_str(tag);
    }
}

/// 将临时标记字符串转为 [{name, count}] JSON
fn finalize_tags(raw: &str, prefix: char) -> String {
    if raw.is_empty() || raw == "[]" { return "[]".into(); }
    let mut map = HashMap::<String, usize>::new();
    for part in raw.split(',') {
        if part.starts_with(prefix) {
            let name = &part[2..];
            *map.entry(name.to_string()).or_insert(0) += 1;
        }
    }
    let vec: Vec<Value> = map.into_iter()
        .map(|(n, c)| serde_json::json!({"name": n, "count": c}))
        .collect();
    serde_json::to_string(&vec).unwrap_or_else(|_| "[]".into())
}

/// 全量解析 JSONL 文件，从第 skip_lines 行开始
fn parse_session_stats(path: &std::path::Path, skip_lines: usize) -> ParsedStats {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return empty_stats(),
    };
    let mut reader = BufReader::new(file);

    // 跳过已解析的行
    if skip_lines > 0 {
        let mut skipped = 0usize;
        let mut buf = String::new();
        while skipped < skip_lines {
            buf.clear();
            match reader.read_line(&mut buf) {
                Ok(0) => return empty_stats(),
                Ok(_) => skipped += 1,
                Err(_) => return empty_stats(),
            }
        }
    }

    let mut stats = empty_stats();
    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let json: Value = match serde_json::from_str(&line) {
            Ok(v) => v, Err(_) => continue,
        };
        parse_one_line(&json, &mut stats);
    }

    stats.parsed_lines = skip_lines + stats.message_count;

    // 计算时长
    stats.duration_ms = if !stats.started_at.is_empty() && !stats.ended_at.is_empty() {
        chrono::NaiveDateTime::parse_from_str(&stats.ended_at, "%Y-%m-%dT%H:%M:%S%.fZ").ok()
            .and_then(|end| {
                chrono::NaiveDateTime::parse_from_str(&stats.started_at, "%Y-%m-%dT%H:%M:%S%.fZ").ok()
                    .map(|start| (end - start).num_milliseconds().max(0))
            }).unwrap_or(0)
    } else { 0 };

    // 汇总 models 去重排序
    let model_set: HashSet<&str> = stats.models.split(", ").filter(|s| !s.is_empty()).collect();
    let mut models: Vec<&str> = model_set.into_iter().collect();
    models.sort();
    stats.models = models.join(", ");

    // 将临时标记转为正式 JSON
    let tool_raw = std::mem::take(&mut stats.tool_calls_json);
    let skill_raw = std::mem::take(&mut stats.skill_calls_json);
    let agent_raw = std::mem::take(&mut stats.agent_calls_json);
    stats.tool_calls_json = finalize_tags(&tool_raw, 'T');
    stats.skill_calls_json = finalize_tags(&skill_raw, 'S');
    stats.agent_calls_json = finalize_tags(&agent_raw, 'A');

    stats
}

fn empty_stats() -> ParsedStats {
    ParsedStats {
        message_count: 0, user_messages: 0, assistant_msgs: 0,
        input_tokens: 0, output_tokens: 0, cache_read_tokens: 0,
        models: String::new(), tool_calls_json: String::new(),
        skill_calls_json: String::new(), agent_calls_json: String::new(),
        started_at: String::new(), ended_at: String::new(), duration_ms: 0,
        title: String::new(), parsed_lines: 0, model_entries: Vec::new(),
    }
}

// ════════════════════════════════════════════════════════════
// 分析汇总查询（纯 DB 读取，不触发 JSONL 解析）
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

    let mut stmt = conn.prepare(
        "SELECT project, COUNT(*), COALESCE(SUM(message_count), 0), COALESCE(SUM(input_tokens + output_tokens), 0)
         FROM session_stats GROUP BY project ORDER BY COUNT(*) DESC LIMIT 5",
    ).map_err(|e| e.to_string())?;
    let project_stats: Vec<ProjectStats> = stmt.query_map([], |row| {
        Ok(ProjectStats { project: row.get(0)?, session_count: row.get(1)?, total_messages: row.get(2)?, total_tokens: row.get(3)? })
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut stmt = conn.prepare(
        "SELECT date(started_at), COUNT(*), COALESCE(SUM(message_count), 0),
                COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
         FROM session_stats WHERE started_at IS NOT NULL
         GROUP BY date(started_at) ORDER BY date(started_at) DESC LIMIT 60",
    ).map_err(|e| e.to_string())?;
    let daily: Vec<DailyStats> = stmt.query_map([], |row| {
        Ok(DailyStats { date: row.get(0)?, session_count: row.get(1)?, message_count: row.get(2)?, input_tokens: row.get(3)?, output_tokens: row.get(4)? })
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

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
        let hit_rate = if inp > 0 { cache as f64 / inp as f64 } else { 0.0 };
        Ok(ModelRanking {
            name: model, session_count: sessions as usize,
            input_tokens: inp as u64, output_tokens: out as u64,
            cache_read: cache as u64, cache_hit_rate: hit_rate,
        })
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

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

    Ok(AnalyticsSummary {
        total_sessions, total_messages,
        total_input_tokens: total_input, total_output_tokens: total_output,
        total_cache_read: total_cache, active_days,
        top_models, project_stats, daily, model_daily,
    })
}

/// 按需加载 top 工具/技能/智能体排行（需要反序列化 JSON，较重）
#[derive(Debug, Serialize)]
pub struct AnalyticsTopItems {
    pub top_tools: Vec<NameCount>,
    pub top_skills: Vec<NameCount>,
    pub top_agents: Vec<NameCount>,
}

pub fn get_analytics_top_items(conn: &Connection) -> Result<AnalyticsTopItems, String> {
    Ok(AnalyticsTopItems {
        top_tools: aggregate_json_column(conn, "tool_calls_json"),
        top_skills: aggregate_json_column(conn, "skill_calls_json"),
        top_agents: aggregate_json_column(conn, "agent_calls_json"),
    })
}

fn aggregate_json_column(conn: &Connection, column: &str) -> Vec<NameCount> {
    let sql = format!("SELECT {} FROM session_stats WHERE {} != '[]'", column, column);
    let all: Vec<String> = match conn.prepare(&sql) {
        Ok(mut stmt) => stmt.query_map([], |row| row.get::<_, String>(0))
            .map(|iter| iter.filter_map(|r| r.ok()).collect())
            .unwrap_or_default(),
        Err(_) => return vec![],
    };
    let mut map = HashMap::<String, usize>::new();
    for json_str in &all {
        if let Ok(arr) = serde_json::from_str::<Vec<Value>>(json_str) {
            for item in arr {
                if let (Some(name), Some(count)) = (
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("count").and_then(|v| v.as_u64()),
                ) {
                    *map.entry(name.to_string()).or_insert(0) += count as usize;
                }
            }
        }
    }
    let mut result: Vec<NameCount> = map.into_iter()
        .map(|(name, count)| NameCount { name, count }).collect();
    result.sort_by(|a, b| b.count.cmp(&a.count));
    result.truncate(10);
    result
}
