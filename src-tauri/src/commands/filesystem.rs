use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Seek};
use std::path::PathBuf;

use super::AppState;

/// 获取 ~/.qwen 根目录
fn qwen_home() -> PathBuf {
    dirs_or_default().join(".qwen")
}

fn dirs_or_default() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

// ════════════════════════════════════════════════════════════
// Skills
// ════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub skill_type: String,
    pub path: String,
    pub source: String,
}

/// 扫描本地技能列表
#[tauri::command]
pub fn list_skills() -> Result<Vec<SkillInfo>, String> {
    let mut skills = Vec::new();
    let qwen = qwen_home();

    // 用户自定义技能
    let user_skills_dir = qwen.join("skills");
    if user_skills_dir.is_dir() {
        scan_skill_dir(&user_skills_dir, "user", &mut skills)?;
    }

    // 扩展提供的技能
    let ext_dir = qwen.join("extensions");
    if ext_dir.is_dir() {
        for entry in fs::read_dir(&ext_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let skills_sub = entry.path().join("skills");
            if skills_sub.is_dir() {
                let ext_name = entry.file_name().to_string_lossy().to_string();
                scan_skill_dir(&skills_sub, &format!("ext:{}", ext_name), &mut skills)?;
            }
        }
    }

    Ok(skills)
}

fn scan_skill_dir(dir: &PathBuf, source: &str, out: &mut Vec<SkillInfo>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // 查找 SKILL.md（大小写不敏感）
        let skill_md = find_skill_md(&path);
        if let Some(md_path) = skill_md {
            let (name, desc) = parse_skill_frontmatter(&md_path);
            out.push(SkillInfo {
                name: name.unwrap_or_else(|| entry.file_name().to_string_lossy().to_string()),
                description: desc,
                skill_type: source.to_string(),
                path: path.to_string_lossy().to_string(),
                source: source.to_string(),
            });
        }
    }
    Ok(())
}

fn find_skill_md(dir: &PathBuf) -> Option<PathBuf> {
    for name in &["SKILL.md", "skill.md", "SKILL.MD"] {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn parse_skill_frontmatter(path: &PathBuf) -> (Option<String>, String) {
    let content = fs::read_to_string(path).unwrap_or_default();
    parse_frontmatter(&content)
}

/// 解析 YAML frontmatter，提取 name 和 description
pub fn parse_frontmatter(content: &str) -> (Option<String>, String) {
    let mut name = None;
    let mut description = String::new();

    if !content.starts_with("---") {
        return (None, description);
    }

    let after_first = &content[3..];
    let Some(end) = after_first.find("---") else { return (None, description) };
    let fm = &after_first[..end];

    for line in fm.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name:") {
            name = Some(val.trim().trim_matches('"').to_string());
        } else if let Some(val) = line.strip_prefix("description:") {
            description = val.trim().trim_matches('"').to_string();
        }
    }

    (name, description)
}

/// 读取 SKILL.md 内容
#[tauri::command]
pub fn read_skill_content(path: String) -> Result<String, String> {
    let dir = PathBuf::from(&path);
    let md = find_skill_md(&dir).ok_or("SKILL.md not found")?;
    fs::read_to_string(&md).map_err(|e| e.to_string())
}

/// 删除技能目录（仅限用户自定义技能）
#[tauri::command]
pub fn delete_skill(path: String) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err("目录不存在".into());
    }
    // 安全检查：只允许删除 ~/.qwen/skills/ 下的目录
    let user_skills = qwen_home().join("skills");
    if !dir.starts_with(&user_skills) {
        return Err("只能删除用户自定义技能".into());
    }
    fs::remove_dir_all(&dir).map_err(|e| e.to_string())
}

/// 写入技能 SKILL.md 内容
#[tauri::command]
pub fn write_skill(path: String, content: String) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    let md = find_skill_md(&dir).ok_or("SKILL.md not found")?;
    fs::write(&md, &content).map_err(|e| e.to_string())
}

/// 写入扩展的上下文文件（QWEN.md 等）
#[tauri::command]
pub fn write_extension_context(name: String, content: String) -> Result<(), String> {
    let ext_dir = qwen_home().join("extensions").join(&name);
    if !ext_dir.is_dir() {
        return Err("扩展目录不存在".into());
    }
    // 找到上下文文件（QWEN.md / GEMINI.md / CLAUDE.md）
    let candidates = ["QWEN.md", "GEMINI.md", "CLAUDE.md"];
    for c in &candidates {
        let p = ext_dir.join(c);
        if p.exists() {
            fs::write(&p, &content).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    // 如果都没有，创建 QWEN.md
    fs::write(ext_dir.join("QWEN.md"), &content).map_err(|e| e.to_string())
}

// ════════════════════════════════════════════════════════════
// Projects & Sessions
// ════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub session_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub message_count: usize,
    pub file_path: String,
}

/// 会话详情（只返回统计信息，不含消息列表）
#[derive(Debug, Clone, Serialize)]
pub struct SessionDetail {
    pub message_count: usize,
    pub models: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub duration: String,
    pub tool_calls: Vec<ToolCallStat>,
    pub skill_calls: Vec<ToolCallStat>,
    pub agent_calls: Vec<ToolCallStat>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolCallStat {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionMessage {
    pub uuid: String,
    pub msg_type: String,
    pub timestamp: String,
    pub model: Option<String>,
    pub text: String,
    pub thinking: Option<String>,
    pub has_tool_use: bool,
    pub tool_name: Option<String>,
    pub tool_input_preview: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// 扫描 ~/.qwen/projects/ 下的项目列表
#[tauri::command]
pub fn list_projects() -> Result<Vec<ProjectInfo>, String> {
    let projects_dir = qwen_home().join("projects");
    if !projects_dir.is_dir() {
        return Ok(vec![]);
    }

    let mut projects = Vec::new();
    for entry in fs::read_dir(&projects_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }
        let chats_dir = entry.path().join("chats");
        let session_count = if chats_dir.is_dir() {
            fs::read_dir(&chats_dir)
                .map(|rd| rd.filter(|e| e.as_ref().map(|e| e.path().extension().map_or(false, |ext| ext == "jsonl")).unwrap_or(false)).count())
                .unwrap_or(0)
        } else {
            0
        };

        projects.push(ProjectInfo {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            session_count,
        });
    }

    Ok(projects)
}

/// 扫描项目下的会话列表（文件系统为主，DB 补充 title/tokens）
#[tauri::command]
pub fn list_sessions(
    project: String,
    limit: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SessionInfo>, String> {
    let chats_dir = qwen_home().join("projects").join(&project).join("chats");
    if !chats_dir.is_dir() {
        return Ok(vec![]);
    }

    // 收集所有 JSONL 文件，按修改时间倒序
    let mut entries: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();
    for entry in fs::read_dir(&chats_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "jsonl") { continue; }
        let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok()).unwrap_or(std::time::UNIX_EPOCH);
        entries.push((path, mtime));
    }
    entries.sort_by(|a, b| b.1.cmp(&a.1));

    let scan_limit = limit.unwrap_or(100);
    let mut sessions = Vec::with_capacity(scan_limit.min(entries.len()));

    // 尝试从 DB 补充 title/tokens（不阻塞，失败就用文件系统数据）
    let db_cache: std::collections::HashMap<String, (String, i64)> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut cache = std::collections::HashMap::new();
        if let Ok(mut stmt) = db.prepare(
            "SELECT session_id, COALESCE(title, ''), input_tokens FROM session_stats WHERE project = ?1"
        ) {
            if let Ok(rows) = stmt.query_map(rusqlite::params![project], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
            }) {
                for row in rows.flatten() {
                    cache.insert(row.0, (row.1, row.2));
                }
            }
        }
        cache
    };

    for (path, _) in entries.into_iter().take(scan_limit) {
        let id = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let file_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        // 从 DB 缓存获取 title/tokens
        let (title, _input_tokens) = db_cache.get(&id)
            .map(|(t, tok)| (t.clone(), *tok))
            .unwrap_or_default();

        // 如果 DB 没有 title，从文件快速提取
        let title = if title.is_empty() {
            quick_extract_title(&path)
        } else { title };

        sessions.push(SessionInfo {
            id,
            title,
            started_at: String::new(), // 不阻塞，后续 stats-synced 事件补充
            message_count: (file_size / 500).max(1) as usize, // 估算
            file_path: path.to_string_lossy().to_string(),
        });
    }

    Ok(sessions)
}

/// 只读前几行提取标题（不做全量解析）
fn quick_extract_title(path: &std::path::Path) -> String {
    let file = match fs::File::open(path) { Ok(f) => f, Err(_) => return String::new() };
    let reader = BufReader::new(file);
    for line in reader.lines().take(20) {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let json: Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
        let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if msg_type == "system" && json.get("subtype").and_then(|v| v.as_str()) == Some("custom_title") {
            if let Some(t) = json.get("title").and_then(|v| v.as_str()) {
                return t.to_string();
            }
        }
        if msg_type == "user" {
            if let Some(text) = json.pointer("/message/parts").and_then(|v| v.as_array())
                .and_then(|arr| arr.iter().find_map(|p| p.get("text").and_then(|v| v.as_str())))
            {
                return text.chars().take(80).collect();
            }
        }
    }
    String::new()
}

/// 从 DB 缓存读取会话统计详情（不解析 JSONL）
#[tauri::command]
pub fn get_session_detail(
    project: String, session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<SessionDetail, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let result = db.query_row(
        "SELECT message_count, models, input_tokens, output_tokens, duration_ms,
                tool_calls_json, skill_calls_json, agent_calls_json
         FROM session_stats WHERE project = ?1 AND session_id = ?2",
        rusqlite::params![project, session_id],
        |row| {
            let msg_count: i64 = row.get(0)?;
            let models: String = row.get(1)?;
            let inp: i64 = row.get(2)?;
            let out: i64 = row.get(3)?;
            let dur_ms: i64 = row.get(4)?;
            let tool_json: String = row.get(5)?;
            let skill_json: String = row.get(6)?;
            let agent_json: String = row.get(7)?;
            Ok((msg_count, models, inp, out, dur_ms, tool_json, skill_json, agent_json))
        },
    );

    match result {
        Ok((msg_count, models, inp, out, dur_ms, tool_json, skill_json, agent_json)) => {
            let duration = format_duration(dur_ms);
            let tool_calls = parse_name_count_json(&tool_json);
            let skill_calls = parse_name_count_json(&skill_json);
            let agent_calls = parse_name_count_json(&agent_json);
            Ok(SessionDetail {
                message_count: msg_count as usize, models,
                input_tokens: inp as u64, output_tokens: out as u64,
                duration, tool_calls, skill_calls, agent_calls,
            })
        }
        Err(_) => {
            // DB 无记录（尚未同步），返回空
            Ok(SessionDetail {
                message_count: 0, models: String::new(),
                input_tokens: 0, output_tokens: 0,
                duration: String::new(),
                tool_calls: vec![], skill_calls: vec![], agent_calls: vec![],
            })
        }
    }
}

fn format_duration(ms: i64) -> String {
    let mins = ms / 60000;
    if mins >= 60 { format!("{}h {}m", mins / 60, mins % 60) } else { format!("{}m", mins) }
}

fn parse_name_count_json(json_str: &str) -> Vec<ToolCallStat> {
    let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) else { return vec![] };
    arr.into_iter().filter_map(|item| {
        let name = item.get("name")?.as_str()?.to_string();
        let count = item.get("count")?.as_u64()? as usize;
        Some(ToolCallStat { name, count })
    }).collect()
}

/// 分页消息加载结果
#[derive(Debug, Clone, Serialize)]
pub struct PagedMessages {
    pub messages: Vec<SessionMessage>,
    pub total_count: usize,
    pub has_older: bool,
    pub has_newer: bool,
}

/// 流式分页加载会话消息（不将全部行加载到内存）
/// 从末尾取（最新消息优先）：先统计总行数，再只解析需要的切片
#[tauri::command]
pub fn get_session_messages_paged(
    project: String,
    session_id: String,
    offset: usize,
    limit: usize,
) -> Result<PagedMessages, String> {
    let path = qwen_home()
        .join("projects")
        .join(&project)
        .join("chats")
        .join(format!("{}.jsonl", session_id));

    let file = fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file);

    // 第一遍：只数行数（不解析 JSON，只读字节）
    let total = {
        let mut count = 0usize;
        let mut buf = String::new();
        loop {
            buf.clear();
            match reader.read_line(&mut buf) {
                Ok(0) => break,
                Ok(_) => count += 1,
                Err(_) => break,
            }
        }
        count
    };

    // 计算需要的行范围（从末尾取）
    let rev_start = total.saturating_sub(offset + limit);
    let rev_end = total.saturating_sub(offset);

    // 第二遍：只读取需要的行并解析
    reader.rewind().map_err(|e| e.to_string())?;
    let mut line_num = 0usize;
    let mut messages = Vec::new();
    let mut buf = String::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,
            Ok(_) => {
                if line_num >= rev_start && line_num < rev_end {
                    if let Ok(json) = serde_json::from_str::<Value>(&buf) {
                        if let Some(msg) = parse_message(&json) {
                            messages.push(msg);
                        }
                    }
                }
                line_num += 1;
                if line_num >= rev_end { break; }
            }
            Err(_) => break,
        }
    }

    Ok(PagedMessages {
        messages,
        total_count: total,
        has_older: rev_start > 0,
        has_newer: offset > 0,
    })
}

/// 从 JSON 解析单条消息（提取公共逻辑）
fn parse_message(json: &Value) -> Option<SessionMessage> {
    let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let uuid = json.get("uuid").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let timestamp = json.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let mut text = String::new();
    let mut thinking = None;
    let mut has_tool_use = false;
    let mut tool_name = None;
    let mut tool_input_preview = None;
    let mut msg_input_tokens = 0u64;
    let mut msg_output_tokens = 0u64;
    let mut model = None;

    if msg_type == "assistant" {
        model = json.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
        if let Some(usage) = json.get("usageMetadata") {
            msg_input_tokens = usage.get("promptTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
            msg_output_tokens = usage.get("candidatesTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
        }
        if let Some(parts) = json.pointer("/message/parts").and_then(|v| v.as_array()) {
            for part in parts {
                if let Some(thought) = part.get("thought").and_then(|v| v.as_bool()) {
                    if thought {
                        thinking = part.get("text").and_then(|v| v.as_str()).map(|s| s.to_string());
                        continue;
                    }
                }
                if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() { text.push('\n'); }
                    text.push_str(t);
                }
                if part.get("functionCall").is_some() {
                    has_tool_use = true;
                    if let Some(name) = part.pointer("/functionCall/name").and_then(|v| v.as_str()) {
                        tool_name = Some(name.to_string());
                        if let Some(input) = part.pointer("/functionCall/input") {
                            let preview = match input {
                                Value::Object(map) => {
                                    if let Some(cmd) = map.get("command").and_then(|v| v.as_str()) {
                                        format!("$ {}", cmd.chars().take(120).collect::<String>())
                                    } else if let Some(fp) = map.get("file_path").or_else(|| map.get("path")).and_then(|v| v.as_str()) {
                                        fp.to_string()
                                    } else {
                                        serde_json::to_string(input).unwrap_or_default().chars().take(100).collect()
                                    }
                                }
                                _ => serde_json::to_string(input).unwrap_or_default().chars().take(100).collect(),
                            };
                            tool_input_preview = Some(preview);
                        }
                    }
                }
            }
        }
    } else if msg_type == "user" {
        if let Some(parts) = json.pointer("/message/parts").and_then(|v| v.as_array()) {
            for part in parts {
                if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() { text.push('\n'); }
                    text.push_str(t);
                }
            }
        }
    } else if msg_type == "system" {
        text = json.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    }

    Some(SessionMessage {
        uuid, msg_type, timestamp, model, text, thinking,
        has_tool_use, tool_name, tool_input_preview,
        input_tokens: msg_input_tokens,
        output_tokens: msg_output_tokens,
    })
}

/// 读取会话 JSONL（原始格式，兼容旧调用）
#[tauri::command]
pub fn read_session(project: String, session_id: String) -> Result<Vec<Value>, String> {
    let path = qwen_home()
        .join("projects")
        .join(&project)
        .join("chats")
        .join(format!("{}.jsonl", session_id));

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let messages: Vec<Value> = content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    Ok(messages)
}

// ════════════════════════════════════════════════════════════
// Memory
// ════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
pub struct MemoryFile {
    pub name: String,
    pub memory_type: String,
    pub description: String,
    pub path: String,
}

/// 扫描记忆文件列表
#[tauri::command]
pub fn list_memories(project: Option<String>) -> Result<Vec<MemoryFile>, String> {
    let mem_dir = match &project {
        Some(p) => qwen_home().join("projects").join(p).join("memory"),
        None => qwen_home(),
    };

    if !mem_dir.is_dir() {
        return Ok(vec![]);
    }

    let mut memories = Vec::new();
    for entry in fs::read_dir(&mem_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().map_or(true, |ext| ext != "md") {
            continue;
        }
        if path.file_name().map_or(false, |n| n == "MEMORY.md") {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        let (name, desc) = parse_frontmatter(&content);
        let mem_type = extract_frontmatter_field(&content, "type");

        memories.push(MemoryFile {
            name: name.unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            }),
            memory_type: mem_type.unwrap_or_else(|| "unknown".into()),
            description: desc,
            path: path.to_string_lossy().to_string(),
        });
    }

    Ok(memories)
}

pub fn extract_frontmatter_field(content: &str, key: &str) -> Option<String> {
    if !content.starts_with("---") {
        return None;
    }
    let after_first = &content[3..];
    let end = after_first.find("---")?;
    let fm = &after_first[..end];
    for line in fm.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix(&format!("{}:", key)) {
            return Some(val.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// 读取记忆文件（分离 frontmatter 和正文）
#[tauri::command]
pub fn read_memory(path: String) -> Result<Value, String> {
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let (_fm_end, fm, body) = split_frontmatter(&content);
    Ok(json!({
        "frontmatter": fm,
        "content": body,
    }))
}

fn split_frontmatter(content: &str) -> (usize, String, String) {
    if !content.starts_with("---") {
        return (0, String::new(), content.to_string());
    }
    let after_first = &content[3..];
    if let Some(end) = after_first.find("---") {
        let fm = after_first[..end].trim().to_string();
        let body = after_first[end + 3..].trim().to_string();
        (end + 6, fm, body)
    } else {
        (0, String::new(), content.to_string())
    }
}

/// 写回记忆文件
#[tauri::command]
pub fn write_memory(path: String, content: String) -> Result<(), String> {
    // 备份
    let p = PathBuf::from(&path);
    if p.exists() {
        let backup = p.with_extension("md.bak");
        fs::copy(&p, &backup).ok();
    }
    fs::write(&path, &content).map_err(|e| e.to_string())
}

/// 删除记忆文件
#[tauri::command]
pub fn delete_memory(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err("文件不存在".into());
    }
    fs::remove_file(&p).map_err(|e| e.to_string())
}

// ════════════════════════════════════════════════════════════
// Agents (SubAgents)
// ════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
pub struct AgentDef {
    pub name: String,
    pub description: String,
    pub model: String,
    pub path: String,
}

/// 扫描 ~/.qwen/agents/ 下的 agent 定义
#[tauri::command]
pub fn list_agents() -> Result<Vec<AgentDef>, String> {
    let agents_dir = qwen_home().join("agents");
    if !agents_dir.is_dir() {
        return Ok(vec![]);
    }

    let mut agents = Vec::new();
    for entry in fs::read_dir(&agents_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "md") {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        let (name, desc) = parse_frontmatter(&content);
        let model = extract_frontmatter_field(&content, "model").unwrap_or_default();

        agents.push(AgentDef {
            name: name.unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            }),
            description: desc,
            model,
            path: path.to_string_lossy().to_string(),
        });
    }

    Ok(agents)
}

/// 读取 agent 定义（分离 frontmatter 和正文）
#[tauri::command]
pub fn read_agent(name: String) -> Result<Value, String> {
    let path = qwen_home().join("agents").join(format!("{}.md", name));
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let (_, fm, body) = split_frontmatter(&content);
    Ok(json!({
        "frontmatter": fm,
        "content": body,
    }))
}

/// 写回 agent 定义
#[tauri::command]
pub fn write_agent(name: String, content: String) -> Result<(), String> {
    let path = qwen_home().join("agents").join(format!("{}.md", name));
    if path.exists() {
        let backup = path.with_extension("md.bak");
        fs::copy(&path, &backup).ok();
    }
    fs::write(&path, &content).map_err(|e| e.to_string())
}

/// 删除 agent 定义
#[tauri::command]
pub fn delete_agent(name: String) -> Result<(), String> {
    let path = qwen_home().join("agents").join(format!("{}.md", name));
    if !path.exists() {
        return Err("文件不存在".into());
    }
    fs::remove_file(&path).map_err(|e| e.to_string())
}

// ════════════════════════════════════════════════════════════
// Extensions
// ════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
pub struct ExtensionInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub path: String,
    pub has_skills: bool,
    pub has_hooks: bool,
    pub has_commands: bool,
    pub has_agents: bool,
}

/// 扫描 ~/.qwen/extensions/ 下的扩展列表
#[tauri::command]
pub fn list_extensions() -> Result<Vec<ExtensionInfo>, String> {
    let ext_dir = qwen_home().join("extensions");
    if !ext_dir.is_dir() {
        return Ok(vec![]);
    }

    // 读取启用配置
    let enablement = read_enablement();

    let mut extensions = Vec::new();
    for entry in fs::read_dir(&ext_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();

        // 读取 qwen-extension.json
        let config_path = path.join("qwen-extension.json");
        let (version, description) = if config_path.is_file() {
            let content = fs::read_to_string(&config_path).unwrap_or_else(|_| "{}".into());
            let json: Value = serde_json::from_str(&content).unwrap_or(json!({}));
            (
                json.get("version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                json.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            )
        } else {
            (String::new(), String::new())
        };

        let enabled = enablement.get(&name).map_or(true, |v| {
            // 如果有 overrides 配置，视为启用
            v.get("overrides").is_some()
        });

        extensions.push(ExtensionInfo {
            name,
            version,
            description,
            enabled,
            path: path.to_string_lossy().to_string(),
            has_skills: path.join("skills").is_dir(),
            has_hooks: path.join("hooks").is_dir(),
            has_commands: path.join("commands").is_dir(),
            has_agents: path.join("agents").is_dir(),
        });
    }

    Ok(extensions)
}

fn read_enablement() -> serde_json::Map<String, Value> {
    let path = qwen_home().join("extensions").join("extension-enablement.json");
    if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        serde_json::Map::new()
    }
}

/// 读取扩展详情
#[tauri::command]
pub fn read_extension_detail(name: String) -> Result<Value, String> {
    let ext_path = qwen_home().join("extensions").join(&name);
    if !ext_path.is_dir() {
        return Err("扩展不存在".into());
    }

    let config_path = ext_path.join("qwen-extension.json");
    let config: Value = if config_path.is_file() {
        let content = fs::read_to_string(&config_path).unwrap_or_else(|_| "{}".into());
        serde_json::from_str(&content).unwrap_or(json!({}))
    } else {
        json!({})
    };

    // 读取上下文文件
    let context = if ext_path.join("QWEN.md").is_file() {
        fs::read_to_string(ext_path.join("QWEN.md")).ok()
    } else if ext_path.join("GEMINI.md").is_file() {
        fs::read_to_string(ext_path.join("GEMINI.md")).ok()
    } else {
        None
    };

    Ok(json!({
        "config": config,
        "context": context,
        "path": ext_path.to_string_lossy(),
    }))
}

/// 启用/禁用扩展
#[tauri::command]
pub fn toggle_extension(name: String, enabled: bool) -> Result<(), String> {
    let path = qwen_home().join("extensions").join("extension-enablement.json");
    let mut enablement: serde_json::Map<String, Value> = if path.is_file() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        serde_json::Map::new()
    };

    if enabled {
        enablement.insert(name, json!({ "overrides": ["*"] }));
    } else {
        enablement.remove(&name);
    }

    let content = serde_json::to_string_pretty(&enablement).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())
}

/// 删除扩展
#[tauri::command]
pub fn delete_extension(name: String) -> Result<(), String> {
    let path = qwen_home().join("extensions").join(&name);
    if !path.is_dir() {
        return Err("扩展不存在".into());
    }
    fs::remove_dir_all(&path).map_err(|e| e.to_string())
}

// ════════════════════════════════════════════════════════════
// Index (加速列表加载)
// ════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
pub struct GlobalIndex {
    pub memories: Vec<MemoryFile>,
    pub projects: Vec<ProjectIndex>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectIndex {
    pub name: String,
    pub session_count: usize,
    pub memory_count: usize,
    pub latest_session: Option<String>,
}

/// 扫描文件系统返回项目列表 + 全局记忆（不依赖 DB，即时返回）
#[tauri::command]
pub fn get_index(state: tauri::State<'_, AppState>, limit: Option<usize>, offset: Option<usize>) -> Result<GlobalIndex, String> {
    let global_memories = state.global_memory_cache.lock().unwrap().clone();
    let qwen = qwen_home();
    let projects_dir = qwen.join("projects");

    let mut all_projects: Vec<ProjectIndex> = Vec::new();
    if projects_dir.is_dir() {
        for entry in fs::read_dir(&projects_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.path().is_dir() { continue; }
            let name = entry.file_name().to_string_lossy().to_string();

            // 会话：扫描 chats/ 目录下的 JSONL 文件
            let chats_dir = entry.path().join("chats");
            let (session_count, latest_mtime) = if chats_dir.is_dir() {
                let mut count = 0usize;
                let mut latest = std::time::UNIX_EPOCH;
                for c in fs::read_dir(&chats_dir).map_err(|e| e.to_string())? {
                    let c = c.map_err(|e| e.to_string())?;
                    let p = c.path();
                    if p.extension().map_or(false, |ext| ext == "jsonl") {
                        count += 1;
                        if let Ok(meta) = fs::metadata(&p) {
                            if let Ok(t) = meta.modified() {
                                if t > latest { latest = t; }
                            }
                        }
                    }
                }
                (count, latest)
            } else {
                (0, std::time::UNIX_EPOCH)
            };

            if session_count == 0 { continue; }

            // 记忆：扫描 memory/ 目录
            let memory_dir = entry.path().join("memory");
            let memory_count = if memory_dir.is_dir() {
                fs::read_dir(&memory_dir)
                    .map(|rd| rd.filter(|e| {
                        e.as_ref().map(|e| {
                            let p = e.path();
                            p.extension().map_or(false, |ext| ext == "md")
                                && !p.file_name().map_or(false, |n| n == "MEMORY.md")
                        }).unwrap_or(false)
                    }).count())
                    .unwrap_or(0)
            } else { 0 };

            // 最新会话 ID
            let latest_session = if latest_mtime > std::time::UNIX_EPOCH {
                // 找到最新文件的 session ID
                let mut newest_id = String::new();
                let mut newest_time = std::time::UNIX_EPOCH;
                if let Ok(rd) = fs::read_dir(&chats_dir) {
                    for c in rd.flatten() {
                        let p = c.path();
                        if p.extension().map_or(false, |ext| ext == "jsonl") {
                            if let Ok(t) = fs::metadata(&p).and_then(|m| m.modified()) {
                                if t >= newest_time {
                                    newest_time = t;
                                    newest_id = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                                }
                            }
                        }
                    }
                }
                if newest_id.is_empty() { None } else { Some(newest_id) }
            } else { None };

            all_projects.push(ProjectIndex { name, session_count, memory_count, latest_session });
        }
    }

    // 按最新会话时间排序
    // 用 latest_session 字段排序（session ID 就是 UUID，但文件系统 mtime 更准）
    // 简化：直接按目录名排序，前端刷新时会更新
    all_projects.sort_by(|a, b| b.session_count.cmp(&a.session_count));

    // 分页
    let off = offset.unwrap_or(0);
    let lim = limit.unwrap_or(20);
    let projects: Vec<ProjectIndex> = all_projects.into_iter().skip(off).take(lim).collect();

    Ok(GlobalIndex { memories: global_memories, projects })
}
