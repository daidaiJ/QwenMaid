use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
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
fn parse_frontmatter(content: &str) -> (Option<String>, String) {
    let mut name = None;
    let mut description = String::new();

    if !content.starts_with("---") {
        return (None, description);
    }

    let after_first = &content[3..];
    let end = after_first.find("---").unwrap_or(return (None, description));
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

/// 会话详情（用户点击时流式解析单个 JSONL）
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
    pub messages: Vec<SessionMessage>,
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

/// 扫描项目下的会话列表（轻量：只读文件名 + 前几行快速提取标题/时间）
/// 先增量同步 session_stats，再过滤 input_tokens == 0 的空会话
#[tauri::command]
pub fn list_sessions(project: String, state: tauri::State<'_, AppState>) -> Result<Vec<SessionInfo>, String> {
    let chats_dir = qwen_home().join("projects").join(&project).join("chats");
    if !chats_dir.is_dir() {
        return Ok(vec![]);
    }

    // 先增量同步，确保 session_stats 数据是最新的（增量，跳过未变更文件）
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let _ = super::analytics::sync_session_stats(&db);
    }

    // 查询该项目下 input_tokens == 0 的会话 ID
    let zero_token_ids: HashSet<String> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db
            .prepare("SELECT session_id FROM session_stats WHERE project = ?1 AND input_tokens = 0")
            .map_err(|e| e.to_string())?;
        let mut ids = HashSet::new();
        let mut rows = stmt.query(rusqlite::params![project]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let id: String = row.get(0).map_err(|e| e.to_string())?;
            ids.insert(id);
        }
        ids
    };

    let mut sessions = Vec::new();
    for entry in fs::read_dir(&chats_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "jsonl") {
            continue;
        }
        let id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // 已同步且确认 input_tokens == 0 的会话，跳过
        if zero_token_ids.contains(&id) {
            continue;
        }

        let (title, started_at, msg_count) = quick_scan_header(&path);

        sessions.push(SessionInfo {
            id,
            title,
            started_at,
            message_count: msg_count,
            file_path: path.to_string_lossy().to_string(),
        });
    }

    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(sessions)
}

/// 快速扫描前 20 行提取标题和起始时间，消息数用文件大小估算
fn quick_scan_header(path: &std::path::Path) -> (String, String, usize) {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return (String::new(), String::new(), 0),
    };
    let est_count = fs::metadata(path)
        .map(|m| (m.len() / 500).max(1) as usize)
        .unwrap_or(1);

    let reader = BufReader::new(file);
    let mut title = String::new();
    let mut started_at = String::new();

    for line in reader.lines().take(20) {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let json: Value = match serde_json::from_str(&line) {
            Ok(v) => v, Err(_) => continue,
        };
        if started_at.is_empty() {
            if let Some(ts) = json.get("timestamp").and_then(|v| v.as_str()) {
                started_at = ts.to_string();
            }
        }
        let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if msg_type == "system"
            && json.get("subtype").and_then(|v| v.as_str()) == Some("custom_title")
        {
            if let Some(t) = json.get("title").and_then(|v| v.as_str()) {
                title = t.to_string();
            }
        }
        if title.is_empty() && msg_type == "user" {
            if let Some(text) = json.pointer("/message/parts/0/text").and_then(|v| v.as_str()) {
                title = text.chars().take(80).collect();
            }
        }
    }
    (title, started_at, est_count)
}

/// 用户点击会话时触发：流式解析单个 JSONL，返回结构化消息列表 + 统计
#[tauri::command]
pub fn get_session_detail(project: String, session_id: String) -> Result<SessionDetail, String> {
    let path = qwen_home()
        .join("projects")
        .join(&project)
        .join("chats")
        .join(format!("{}.jsonl", session_id));

    let file = fs::File::open(&path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let mut messages = Vec::new();
    let mut model_set = std::collections::HashSet::<String>::new();
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut tool_map = std::collections::HashMap::<String, usize>::new();
    let mut skill_map = std::collections::HashMap::<String, usize>::new();
    let mut agent_map = std::collections::HashMap::<String, usize>::new();
    let mut first_ts = String::new();
    let mut last_ts = String::new();

    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let json: Value = match serde_json::from_str(&line) {
            Ok(v) => v, Err(_) => continue,
        };

        let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let uuid = json.get("uuid").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let timestamp = json.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();

        if first_ts.is_empty() { first_ts = timestamp.clone(); }
        if !timestamp.is_empty() { last_ts = timestamp.clone(); }

        // 提取文本和 thinking
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
            if let Some(m) = &model { model_set.insert(m.clone()); }

            if let Some(usage) = json.get("usageMetadata") {
                msg_input_tokens = usage.get("promptTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
                msg_output_tokens = usage.get("candidatesTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
                input_tokens += msg_input_tokens;
                output_tokens += msg_output_tokens;
            }

            if let Some(parts) = json.pointer("/message/parts").and_then(|v| v.as_array()) {
                for part in parts {
                    if let Some(thought) = part.get("thought").and_then(|v| v.as_bool()) {
                        if thought {
                            if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                                thinking = Some(t.to_string());
                            }
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
                            // 提取输入预览
                            if let Some(input) = part.pointer("/functionCall/input") {
                                let preview = match input {
                                    Value::Object(map) => {
                                        if let Some(cmd) = map.get("command").and_then(|v| v.as_str()) {
                                            format!("$ {}", cmd.chars().take(120).collect::<String>())
                                        } else if let Some(fp) = map.get("file_path").or_else(|| map.get("path")).and_then(|v| v.as_str()) {
                                            fp.to_string()
                                        } else if let Some(desc) = map.get("description").and_then(|v| v.as_str()) {
                                            desc.chars().take(100).collect()
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

        messages.push(SessionMessage {
            uuid,
            msg_type,
            timestamp,
            model,
            text,
            thinking,
            has_tool_use,
            tool_name,
            tool_input_preview,
            input_tokens: msg_input_tokens,
            output_tokens: msg_output_tokens,
        });
    }

    // 计算时长
    let duration = if !first_ts.is_empty() && !last_ts.is_empty() {
        let ms = chrono::NaiveDateTime::parse_from_str(&last_ts, "%Y-%m-%dT%H:%M:%S%.fZ")
            .ok()
            .and_then(|end| {
                chrono::NaiveDateTime::parse_from_str(&first_ts, "%Y-%m-%dT%H:%M:%S%.fZ")
                    .ok()
                    .map(|start| (end - start).num_milliseconds().max(0))
            })
            .unwrap_or(0);
        let mins = ms / 60000;
        if mins >= 60 { format!("{}h {}m", mins / 60, mins % 60) } else { format!("{}m", mins) }
    } else {
        String::new()
    };

    let mut models: Vec<String> = model_set.into_iter().collect();
    models.sort();

    let mut tool_calls: Vec<ToolCallStat> = tool_map.into_iter()
        .map(|(name, count)| ToolCallStat { name, count })
        .collect();
    tool_calls.sort_by(|a, b| b.count.cmp(&a.count));

    let mut skill_calls: Vec<ToolCallStat> = skill_map.into_iter()
        .map(|(name, count)| ToolCallStat { name, count })
        .collect();
    skill_calls.sort_by(|a, b| b.count.cmp(&a.count));

    let mut agent_calls: Vec<ToolCallStat> = agent_map.into_iter()
        .map(|(name, count)| ToolCallStat { name, count })
        .collect();
    agent_calls.sort_by(|a, b| b.count.cmp(&a.count));

    Ok(SessionDetail {
        message_count: messages.len(),
        models: models.join(", "),
        input_tokens,
        output_tokens,
        duration,
        tool_calls,
        skill_calls,
        agent_calls,
        messages,
    })
}

/// 分页消息加载结果
#[derive(Debug, Clone, Serialize)]
pub struct PagedMessages {
    pub messages: Vec<SessionMessage>,
    pub total_count: usize,
    pub has_older: bool,
    pub has_newer: bool,
}

/// 分页加载会话消息（支持从最新往前加载）
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
    let reader = BufReader::new(file);

    // 先全量解析所有消息（JSONL 必须顺序读取）
    let all_messages: Vec<SessionMessage> = reader.lines()
        .filter_map(|line| line.ok())
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .filter_map(|json| parse_message(&json))
        .collect();

    let total = all_messages.len();
    // 从末尾开始取（最新消息优先）
    let rev_offset = total.saturating_sub(offset + limit);
    let rev_end = total.saturating_sub(offset);
    let page: Vec<SessionMessage> = all_messages[rev_offset..rev_end].to_vec();

    Ok(PagedMessages {
        messages: page,
        total_count: total,
        has_older: rev_offset > 0,
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

fn extract_frontmatter_field(content: &str, key: &str) -> Option<String> {
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
    let (fm_end, fm, body) = split_frontmatter(&content);
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

/// 一次性返回全局记忆 + 所有项目的索引统计（避免前端多次 IPC）
/// 先增量同步 session_stats，再过滤掉所有会话 input_tokens 均为 0 的项目
#[tauri::command]
pub fn get_index(state: tauri::State<'_, AppState>) -> Result<GlobalIndex, String> {
    let qwen = qwen_home();

    // 先增量同步 session_stats（增量，跳过未变更文件，首次后几乎无开销）
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let _ = super::analytics::sync_session_stats(&db);
    }

    // 查询有非零 token 会话的项目集合
    let projects_with_tokens: HashSet<String> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db
            .prepare("SELECT DISTINCT project FROM session_stats WHERE input_tokens > 0")
            .map_err(|e| e.to_string())?;
        let mut set = HashSet::new();
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let name: String = row.get(0).map_err(|e| e.to_string())?;
            set.insert(name);
        }
        set
    };

    // 全局记忆
    let mut global_memories = Vec::new();
    if qwen.is_dir() {
        for entry in fs::read_dir(&qwen).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if !path.is_file() || path.extension().map_or(true, |ext| ext != "md") {
                continue;
            }
            if path.file_name().map_or(false, |n| n == "MEMORY.md") {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap_or_default();
            let (name, desc) = parse_frontmatter(&content);
            let mem_type = extract_frontmatter_field(&content, "type");
            global_memories.push(MemoryFile {
                name: name.unwrap_or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()),
                memory_type: mem_type.unwrap_or_else(|| "unknown".into()),
                description: desc,
                path: path.to_string_lossy().to_string(),
            });
        }
    }

    // 项目索引
    let mut projects = Vec::new();
    let projects_dir = qwen.join("projects");
    if projects_dir.is_dir() {
        for entry in fs::read_dir(&projects_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.path().is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();

            // 同步后该项目没有任何非零 token 的会话，跳过
            if !projects_with_tokens.contains(&name) {
                continue;
            }

            // 会话统计
            let chats_dir = entry.path().join("chats");
            let (session_count, latest_session) = if chats_dir.is_dir() {
                let mut sessions: Vec<(String, std::time::SystemTime)> = Vec::new();
                for c in fs::read_dir(&chats_dir).map_err(|e| e.to_string())? {
                    let c = c.map_err(|e| e.to_string())?;
                    let p = c.path();
                    if p.extension().map_or(false, |ext| ext == "jsonl") {
                        let meta = fs::metadata(&p).ok();
                        let mtime = meta.and_then(|m| m.modified().ok());
                        if let Some(t) = mtime {
                            sessions.push((p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), t));
                        }
                    }
                }
                sessions.sort_by(|a, b| b.1.cmp(&a.1));
                let latest = sessions.first().map(|s| s.0.clone());
                (sessions.len(), latest)
            } else {
                (0, None)
            };

            // 记忆统计（排除 MEMORY.md，与 list_memories 保持一致）
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
            } else {
                0
            };

            projects.push(ProjectIndex {
                name,
                session_count,
                memory_count,
                latest_session,
            });
        }
    }

    Ok(GlobalIndex {
        memories: global_memories,
        projects,
    })
}
