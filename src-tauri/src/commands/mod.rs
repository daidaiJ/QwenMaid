#![allow(non_snake_case)]

pub mod analytics;
pub mod filesystem;
pub mod installer;
pub mod metrics;
pub mod skill_marketplace;

use rusqlite::Connection;
use serde_json::{json, Map, Value};
use std::fs;
use std::sync::{Arc, Mutex};
use tauri::Manager;

use crate::config;
use crate::db::providers::{self, CreateModel, CreateProvider, Model, Provider, UpdateModel, UpdateProvider};
use crate::mcp;

/// Tauri 命令共享状态
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub mcp_shutdown: Arc<Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
    pub global_memory_cache: Arc<Mutex<Vec<filesystem::MemoryFile>>>,
    /// settings.json 中已配置的 model ID 缓存，sync 后清空
    pub configured_model_ids: Arc<Mutex<Option<Vec<String>>>>,
}

// ── Provider Commands ────────────────────────────────────

#[tauri::command]
pub fn list_providers(state: tauri::State<'_, AppState>) -> Result<Vec<Provider>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    providers::list_providers(&db)
}

#[tauri::command]
pub fn get_provider(state: tauri::State<'_, AppState>, id: i64) -> Result<Provider, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    providers::get_provider(&db, id)
}

#[tauri::command]
pub fn create_provider(
    state: tauri::State<'_, AppState>,
    name: String,
    baseUrl: String,
    apiKeyEnv: String,
    proxyMode: Option<String>,
    proxyUrl: Option<String>,
    authHeader: Option<String>,
    apiKeyValue: Option<String>,
    billingType: Option<String>,
    compressEnabled: Option<bool>,
) -> Result<Provider, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    providers::create_provider(
        &db,
        &CreateProvider {
            name,
            base_url: baseUrl,
            api_key_env: apiKeyEnv,
            proxy_mode: proxyMode,
            proxy_url: proxyUrl,
            auth_header: authHeader,
            api_key_value: apiKeyValue,
            billing_type: billingType,
            compress_enabled: compressEnabled,
        },
    )
}

#[tauri::command]
pub fn update_provider(
    state: tauri::State<'_, AppState>,
    id: i64,
    name: Option<String>,
    baseUrl: Option<String>,
    apiKeyEnv: Option<String>,
    proxyMode: Option<String>,
    proxyUrl: Option<String>,
    authHeader: Option<String>,
    apiKeyValue: Option<String>,
    billingType: Option<String>,
    isActive: Option<bool>,
    compressEnabled: Option<bool>,
) -> Result<Provider, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    providers::update_provider(
        &db,
        id,
        &UpdateProvider {
            name,
            base_url: baseUrl,
            api_key_env: apiKeyEnv,
            proxy_mode: proxyMode,
            proxy_url: proxyUrl,
            auth_header: authHeader,
            api_key_value: apiKeyValue,
            billing_type: billingType,
            is_active: isActive,
            compress_enabled: compressEnabled,
        },
    )
}

#[tauri::command]
pub fn delete_provider(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    providers::delete_provider(&db, id)
}

// ── Model Commands ───────────────────────────────────────

#[tauri::command]
pub fn list_models(
    state: tauri::State<'_, AppState>,
    provider_id: Option<i64>,
) -> Result<Vec<Model>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    providers::list_models(&db, provider_id)
}

#[tauri::command]
pub fn get_model(state: tauri::State<'_, AppState>, id: i64) -> Result<Model, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    providers::get_model(&db, id)
}

#[tauri::command]
pub fn create_model(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
    model_id: String,
    display_name: Option<String>,
    auth_type: String,
    is_default: Option<bool>,
    config_json: Option<String>,
) -> Result<Model, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    providers::create_model(
        &db,
        &CreateModel {
            provider_id,
            model_id,
            display_name,
            auth_type,
            is_default,
            config_json,
        },
    )
}

#[tauri::command]
pub fn update_model(
    state: tauri::State<'_, AppState>,
    id: i64,
    display_name: Option<String>,
    auth_type: Option<String>,
    is_default: Option<bool>,
    config_json: Option<String>,
) -> Result<Model, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    providers::update_model(
        &db,
        id,
        &UpdateModel {
            display_name,
            auth_type,
            is_default,
            config_json,
        },
    )
}

#[tauri::command]
pub fn delete_model(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    providers::delete_model(&db, id)
}

// ── Config Commands ──────────────────────────────────────

/// 读取 Qwen Code settings.json
#[tauri::command]
pub fn read_settings() -> Result<Value, String> {
    let path = config::user_settings_path();
    config::read_settings(&path)
}

/// 将 DB 中的 providers/models 同步到 settings.json
/// 合并规则：相同 path 替换，其余保留
#[tauri::command]
pub fn sync_config_to_settings(state: tauri::State<'_, AppState>) -> Result<config::SyncResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = config::user_settings_path();
    let result = config::sync_providers_to_settings(&path, &db)?;
    // 只有校验通过且有 settings 时才写入
    if let Some(ref settings) = result.settings {
        config::write_settings(&path, settings)?;
        // 同时写入 .env 文件
        let env = settings.get("env").and_then(|v| v.as_object());
        if let Some(env_map) = env {
            if !env_map.is_empty() {
                config::write_env_file(env_map)?;
            }
        }
    }
    Ok(result)
}

/// 预览同步结果（不写入文件）
#[tauri::command]
pub fn preview_sync_config(state: tauri::State<'_, AppState>) -> Result<config::SyncResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = config::user_settings_path();
    config::sync_providers_to_settings(&path, &db)
}

/// 按路径写入单个字段到 settings.json（自动备份）
///
/// path 支持点号分隔的嵌套路径，如 "model.name" 或 "ui.statusLine.type"。
/// value 传 null 删除该字段。
#[tauri::command]
pub fn write_settings_field(path: String, value: Value) -> Result<Value, String> {
    let settings_path = config::user_settings_path();
    let mut settings = if settings_path.exists() {
        config::read_settings(&settings_path)?
    } else {
        json!({})
    };

    config::set_by_path(&mut settings, &path, value);
    config::write_settings(&settings_path, &settings)?;
    Ok(settings)
}

/// 获取 env 对象（API Keys 脱敏）
#[tauri::command]
pub fn get_env_vars() -> Result<Value, String> {
    let path = config::user_settings_path();
    let settings = if path.exists() {
        config::read_settings(&path)?
    } else {
        json!({})
    };

    let env = settings.get("env").cloned().unwrap_or(json!({}));
    let map = env.as_object().ok_or("env is not an object")?;

    let masked: Map<String, Value> = map
        .iter()
        .map(|(k, v)| {
            let masked_val = match v.as_str() {
                Some(s) if s.len() > 8 => {
                    format!("{}…{}", &s[..4], &s[s.len() - 4..])
                }
                _ => "••••".to_string(),
            };
            (k.clone(), Value::String(masked_val))
        })
        .collect();

    Ok(Value::Object(masked))
}

// ── File System Commands ─────────────────────────────────

/// 在系统文件管理器中打开路径并选中文件
///
/// - 文件路径：打开所在目录并选中该文件（Windows: explorer /select）
/// - 目录路径：直接打开该目录
/// - 路径不存在时返回错误
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);

    if !p.exists() {
        return Err(format!("路径不存在: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        if p.is_file() {
            std::process::Command::new("explorer")
                .args(["/select,", &path])
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            std::process::Command::new("explorer")
                .arg(&path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if p.is_file() {
            std::process::Command::new("open")
                .args(["-R", &path])
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            std::process::Command::new("open")
                .arg(&path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if p.is_file() {
            // xdg-open 打开所在目录
            if let Some(parent) = p.parent() {
                std::process::Command::new("xdg-open")
                    .arg(parent)
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
        } else {
            std::process::Command::new("xdg-open")
                .arg(&path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// 获取 Qwen Code 配置和数据目录路径
#[tauri::command]
pub fn get_qwen_paths() -> Value {
    let home = dirs::home_dir().unwrap_or_default();
    let qwen_dir = home.join(".qwen");
    json!({
        "home": home.to_string_lossy(),
        "qwenDir": qwen_dir.to_string_lossy(),
        "settingsFile": qwen_dir.join("settings.json").to_string_lossy(),
        "skillsDir": qwen_dir.join("skills").to_string_lossy(),
        "extensionsDir": qwen_dir.join("extensions").to_string_lossy(),
        "projectsDir": qwen_dir.join("projects").to_string_lossy(),
    })
}

// ── Analytics Commands ───────────────────────────────────

/// 增量同步所有项目的会话统计数据到 SQLite
/// 返回本次新增/更新的会话数
#[tauri::command]
pub fn sync_session_stats(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    analytics::sync_session_stats(&db)
}

/// 获取全局分析汇总（纯 SQL 聚合，不触发 JSONL 解析，不含 top 排行）
#[tauri::command]
pub fn get_analytics_summary(
    state: tauri::State<'_, AppState>,
) -> Result<analytics::AnalyticsSummary, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    analytics::get_analytics_summary(&db)
}

/// 按需加载 top 工具/技能/智能体排行（较重，前端懒加载）
#[tauri::command]
pub fn get_analytics_top_items(
    state: tauri::State<'_, AppState>,
) -> Result<analytics::AnalyticsTopItems, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    analytics::get_analytics_top_items(&db)
}

// ── Skill Marketplace Commands ───────────────────────────

#[tauri::command]
pub async fn search_skills_sh(
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<skill_marketplace::SkillSearchResult>, String> {
    skill_marketplace::search_skills_sh(&query, limit.unwrap_or(20), offset.unwrap_or(0)).await
}

#[tauri::command]
pub async fn install_skill_from_repo(
    owner: String,
    repo: String,
    branch: String,
    directory: String,
) -> Result<String, String> {
    skill_marketplace::install_skill_from_repo(&owner, &repo, &branch, &directory).await
}

#[tauri::command]
pub fn uninstall_skill(name: String) -> Result<(), String> {
    skill_marketplace::uninstall_skill(&name)
}

// ── MCP Commands ─────────────────────────────────────────

#[tauri::command]
pub fn get_mcp_config(state: tauri::State<'_, AppState>) -> Result<mcp::McpConfig, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    mcp::get_config(&db)
}

#[tauri::command]
pub fn save_mcp_config(
    state: tauri::State<'_, AppState>,
    port: u16,
    auto_inject: bool,
    smartsearch_enabled: bool,
    academicsearch_enabled: bool,
    cleanfetch_enabled: bool,
    search_mode: String,
    tavily_api_key: Option<String>,
    baidu_api_key: Option<String>,
    jina_api_key: Option<String>,
    proxy_url: Option<String>,
) -> Result<(), String> {
    let config = mcp::McpConfig {
        port,
        auto_inject,
        smartsearch_enabled,
        academicsearch_enabled,
        cleanfetch_enabled,
        search_mode,
        tavily_api_key,
        baidu_api_key,
        jina_api_key,
        proxy_url,
    };

    let db = state.db.lock().map_err(|e| e.to_string())?;
    mcp::save_config(&db, &config)?;
    drop(db);

    // 处理自动注入
    update_mcp_auto_inject(port, auto_inject)?;

    Ok(())
}

#[tauri::command]
pub async fn restart_mcp_server(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // 停止当前服务器
    {
        let mut handle = state.mcp_shutdown.lock().map_err(|e| e.to_string())?;
        if let Some(tx) = handle.take() {
            let _ = tx.send(true);
        }
    }

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // 读取配置
    let config = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        mcp::get_config(&db)?
    };

    if !config.smartsearch_enabled
        && !config.academicsearch_enabled
        && !config.cleanfetch_enabled
    {
        return Ok(());
    }

    // 先绑定端口——失败立即返回（端口冲突等）
    let listener = mcp::server::bind_port(config.port).await?;

    // 端口绑定成功，创建 shutdown channel 并启动 serve
    let (tx, rx) = tokio::sync::watch::channel(false);
    {
        let mut handle = state.mcp_shutdown.lock().map_err(|e| e.to_string())?;
        *handle = Some(tx);
    }

    let db = state.db.clone();
    tokio::spawn(async move {
        if let Err(e) = mcp::server::start_server(listener, db, rx).await {
            log::error!("MCP server error: {}", e);
        }
    });

    Ok(())
}

/// TCP 连通性检测：尝试连接 MCP 端口，判断服务器是否实际在运行
#[tauri::command]
pub async fn get_mcp_status(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let port = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        mcp::get_config(&db)?.port
    };
    let addr = format!("127.0.0.1:{}", port);
    Ok(tokio::net::TcpStream::connect(addr)
        .await
        .is_ok())
}

#[tauri::command]
pub fn get_mcp_stats(state: tauri::State<'_, AppState>) -> Result<mcp::McpStats, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    mcp::get_stats(&db)
}

/// 更新 Qwen Code settings.json 中的 mcpServers.websearch 配置
fn update_mcp_auto_inject(port: u16, enable: bool) -> Result<(), String> {
    let settings_path = config::user_settings_path();
    let mut settings = if settings_path.exists() {
        config::read_settings(&settings_path)?
    } else {
        json!({})
    };

    if enable {
        config::set_by_path(
            &mut settings,
            "mcpServers.websearch",
            json!({
                "httpUrl": format!("http://localhost:{}/mcp", port)
            }),
        );
    } else {
        config::set_by_path(&mut settings, "mcpServers.websearch", Value::Null);
    }

    config::write_settings(&settings_path, &settings)
}

/// 注入状态行成本追踪配置到 Qwen Code settings.json
#[tauri::command]
pub fn inject_statusline(app_handle: tauri::AppHandle) -> Result<(), String> {
    let exe_path = resolve_usage_exe(&app_handle)?;

    let settings_path = config::user_settings_path();
    let mut settings = if settings_path.exists() {
        config::read_settings(&settings_path)?
    } else {
        json!({})
    };

    // 去掉 Windows extended-length 前缀 \\?\
    let path_str = exe_path.to_string_lossy().replace('/', "\\");
    let path_str = path_str.strip_prefix("\\\\?\\").unwrap_or(&path_str);

    // 清理旧的根级 statusLine（遗留）
    if let Some(obj) = settings.as_object_mut() {
        obj.remove("statusLine");
    }

    // 写入 ui.statusLine — 直接调用 exe，不套 cmd /c（避免嵌套 cmd.exe 输出 Windows 版本 banner）
    let ui_obj = settings
        .as_object_mut()
        .ok_or("settings is not a JSON object")?;
    let ui = ui_obj
        .entry("ui")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("ui field is not an object")?;
    ui.insert(
        "statusLine".into(),
        json!({
            "type": "command",
            "command": format!("\"{}\" record", path_str)
        }),
    );

    config::write_settings(&settings_path, &settings)
}

/// 移除状态行配置
#[tauri::command]
pub fn remove_statusline() -> Result<(), String> {
    let settings_path = config::user_settings_path();
    let mut settings = if settings_path.exists() {
        config::read_settings(&settings_path)?
    } else {
        json!({})
    };

    // 清理根级 statusLine（遗留）
    if let Some(obj) = settings.as_object_mut() {
        obj.remove("statusLine");
    }
    // 清理 ui.statusLine
    if let Some(ui) = settings.get_mut("ui").and_then(|v| v.as_object_mut()) {
        ui.remove("statusLine");
    }
    config::write_settings(&settings_path, &settings)
}

/// 获取 qwen-usage.exe 路径
fn resolve_usage_exe(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e: tauri::Error| e.to_string())?;
    let exe_path = resource_dir
        .join("resources")
        .join("cli")
        .join("qwen-usage.exe");
    if !exe_path.exists() {
        return Err(format!("qwen-usage.exe not found at {:?}", exe_path));
    }
    Ok(exe_path)
}

/// 检测 qwen-usage 开机自启动状态
#[tauri::command]
pub fn check_usage_autostart(app_handle: tauri::AppHandle) -> Result<bool, String> {
    // 先确保 exe 存在
    let _ = resolve_usage_exe(&app_handle)?;

    // Windows Startup 目录
    let startup_dir = dirs::data_dir()
        .ok_or("Cannot resolve AppData path")?
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup");

    if !startup_dir.exists() {
        return Ok(false);
    }

    // 检查 Startup 目录下是否存在 qwen-usage 相关的文件（符号链接 / 快捷方式）
    let entries = std::fs::read_dir(&startup_dir)
        .map_err(|e| format!("Cannot read Startup dir: {}", e))?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains("qwen-usage") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// 设置 qwen-usage 开机自启动
#[tauri::command]
pub async fn set_usage_autostart(app_handle: tauri::AppHandle, enable: bool) -> Result<(), String> {
    let exe_path = resolve_usage_exe(&app_handle)?;

    let (cmd, args) = if enable {
        ("install", vec!["-a"])
    } else {
        ("uninstall", vec![])
    };

    let mut command = tokio::process::Command::new(&exe_path);
    command.arg(cmd).args(&args);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let output = command
        .output()
        .await
        .map_err(|e| format!("Failed to run qwen-usage {}: {}", cmd, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "qwen-usage {} failed: {}{}",
            cmd,
            stdout.trim(),
            stderr.trim()
        ));
    }
    Ok(())
}

// ── Provider Discovery ───────────────────────────────────

#[derive(serde::Serialize)]
pub struct DiscoveredModel {
    pub id: String,
    pub name: String,
    pub auth_type: Vec<String>,
    pub config_json: Option<String>,
    pub valid: bool,
    pub from_preset: bool,
}

#[derive(serde::Serialize)]
pub struct DiscoveredProvider {
    pub name: String,
    pub base_url: String,
    pub protocol: String,       // "openai" | "anthropic" | "gemini"
    pub env_key: String,
    pub has_key: bool,
    pub is_preset: bool,
    pub preset_name: Option<String>,
    pub models: Vec<DiscoveredModel>,
    pub valid: bool,
    pub error: Option<String>,
}

/// 发现 settings.json 中已有的供应商配置
///
/// 优先级：.env 文件 > settings.json env 对象 > 系统环境变量
#[tauri::command]
pub fn discover_existing_providers(_state: tauri::State<'_, AppState>) -> Result<Vec<DiscoveredProvider>, String> {
    let settings_path = config::user_settings_path();
    if !settings_path.exists() {
        return Ok(vec![]);
    }

    let settings = config::read_settings(&settings_path)?;
    let model_providers = match settings.get("modelProviders").and_then(|v| v.as_object()) {
        Some(mp) => mp,
        None => return Ok(vec![]),
    };

    // 构建 API Key 查找表（优先级：settings.json env > .env > 系统环境变量）
    let mut key_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // 1. 系统环境变量（最低优先级，先插入后覆盖）
    for (k, v) in std::env::vars() {
        key_map.insert(k, v);
    }

    // 2. .env 文件
    let env_path = config::env_file_path();
    if env_path.exists() {
        if let Ok(content) = fs::read_to_string(&env_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some((k, v)) = line.split_once('=') {
                    let key = k.trim().to_string();
                    let val = v.trim().to_string();
                    if !val.is_empty() {
                        key_map.insert(key, val);
                    }
                }
            }
        }
    }

    // 3. settings.json 的 env 对象（最高优先级）
    if let Some(env_obj) = settings.get("env").and_then(|v| v.as_object()) {
        for (k, v) in env_obj {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    key_map.insert(k.clone(), s.to_string());
                }
            }
        }
    }

    // 加载预设
    let data_dir = crate::db::db_path(&std::path::PathBuf::from("."))
        .parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let presets = crate::presets::load_presets(&data_dir);

    // 构建预设匹配索引：(域名, 协议) → preset
    let mut preset_index: std::collections::HashMap<(String, String), &crate::presets::ProviderPreset> =
        std::collections::HashMap::new();
    for preset in &presets {
        let host = extract_host(&preset.base_url);
        if host.is_empty() { continue; }
        let proto = preset.models.first()
            .and_then(|m| m.auth_type.first())
            .cloned()
            .unwrap_or_else(|| "openai".to_string());
        preset_index.insert((host, proto), preset);
    }

    let mut discovered = Vec::new();

    for (protocol, entries) in model_providers {
        let entries_arr = match entries.as_array() {
            Some(a) => a,
            None => continue,
        };

        // 按 baseUrl 分组（同一 baseUrl 的多个 model 合并为一个供应商）
        let mut groups: std::collections::HashMap<String, Vec<&Value>> = std::collections::HashMap::new();
        for entry in entries_arr {
            let url = entry.get("baseUrl").and_then(|v| v.as_str()).unwrap_or("");
            groups.entry(url.to_string()).or_default().push(entry);
        }

        for (base_url, group_entries) in groups {
            if base_url.is_empty() {
                continue;
            }

            // 解析域名用于预设匹配
            let host = extract_host(&base_url);

            let env_key = group_entries.first()
                .and_then(|e| e.get("envKey").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();

            // 匹配预设
            let matched_preset = preset_index.get(&(host.clone(), protocol.clone()));

            // 解析 models（settings.json 中已有的）
            let mut models = Vec::new();
            let mut seen_ids = std::collections::HashSet::new();
            for entry in &group_entries {
                let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if id.is_empty() || !seen_ids.insert(id.to_string()) {
                    continue;
                }
                let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or(id);
                let auth_types: Vec<String> = if let Some(preset) = matched_preset {
                    preset.models.iter()
                        .find(|m| m.id == id)
                        .map(|m| m.auth_type.clone())
                        .unwrap_or_else(|| vec![protocol.clone()])
                } else {
                    vec![protocol.clone()]
                };
                // 提取 generationConfig（含 customHeaders、contextWindowSize 等）
                let mut config_json: Option<String> = entry.get("generationConfig")
                    .filter(|v| v.is_object() && !v.as_object().unwrap().is_empty())
                    .map(|v| serde_json::to_string(v).ok())
                    .flatten();
                // 预设的 contextWindowSize / maxOutputTokens / inputModalities 优先级高于用户配置
                if let Some(preset) = matched_preset {
                    if let Some(pm) = preset.models.iter().find(|m| m.id == id) {
                        let mut gen: serde_json::Map<String, serde_json::Value> = config_json
                            .as_deref()
                            .and_then(|s| serde_json::from_str(s).ok())
                            .unwrap_or_default();
                        if let Some(cws) = pm.context_window_size {
                            gen.insert("contextWindowSize".to_string(), serde_json::json!(cws));
                        }
                        if let Some(mot) = pm.max_output_tokens {
                            gen.insert("samplingParams".to_string(), serde_json::json!({ "max_tokens": mot }));
                        }
                        if let Some(ref modalities) = pm.input_modalities {
                            let mods: serde_json::Map<String, serde_json::Value> = modalities.iter()
                                .map(|m| (m.clone(), serde_json::Value::Bool(true)))
                                .collect();
                            gen.insert("modalities".to_string(), serde_json::Value::Object(mods));
                        }
                        if let Some(ref thinking) = pm.thinking {
                            gen.insert("thinking".to_string(), thinking.clone());
                        }
                        if !gen.is_empty() {
                            config_json = serde_json::to_string(&gen).ok();
                        }
                    }
                }
                models.push(DiscoveredModel {
                    id: id.to_string(),
                    name: name.to_string(),
                    auth_type: auth_types,
                    config_json,
                    valid: true,
                    from_preset: false,
                });
            }

            // 补齐预设中已有但 settings.json 中缺失的模型
            if let Some(preset) = matched_preset {
                for pm in &preset.models {
                    if seen_ids.contains(&pm.id) {
                        continue;
                    }
                    // 只补齐协议匹配的预设模型
                    if !pm.auth_type.iter().any(|at| at == protocol) {
                        continue;
                    }
                    seen_ids.insert(pm.id.clone());
                    // 从预设构建 generationConfig（contextWindowSize、maxOutputTokens、modalities、thinking）
                    let preset_config = {
                        let mut gen = serde_json::Map::new();
                        if let Some(cws) = pm.context_window_size {
                            gen.insert("contextWindowSize".to_string(), serde_json::json!(cws));
                        }
                        if let Some(mot) = pm.max_output_tokens {
                            gen.insert("samplingParams".to_string(), serde_json::json!({ "max_tokens": mot }));
                        }
                        if let Some(ref modalities) = pm.input_modalities {
                            let mods: serde_json::Map<String, serde_json::Value> = modalities.iter()
                                .map(|m| (m.clone(), serde_json::Value::Bool(true)))
                                .collect();
                            gen.insert("modalities".to_string(), serde_json::Value::Object(mods));
                        }
                        if let Some(ref thinking) = pm.thinking {
                            gen.insert("thinking".to_string(), thinking.clone());
                        }
                        if gen.is_empty() { None }
                        else { serde_json::to_string(&gen).ok() }
                    };
                    models.push(DiscoveredModel {
                        id: pm.id.clone(),
                        name: pm.name.clone(),
                        auth_type: pm.auth_type.clone(),
                        config_json: preset_config,
                        valid: true,
                        from_preset: true,
                    });
                }
            }

            if models.is_empty() {
                continue;
            }

            // 检查 API Key
            let has_key = if let Some(custom_headers) = group_entries.first()
                .and_then(|e| e.pointer("/generationConfig/customHeaders"))
                .and_then(|v| v.as_object())
            {
                custom_headers.values().any(|v| {
                    v.as_str().map(|s| !s.is_empty()).unwrap_or(false)
                })
            } else {
                !env_key.is_empty() && key_map.get(&env_key).map(|v| !v.is_empty()).unwrap_or(false)
            };

            let name = if let Some(preset) = matched_preset {
                preset.name.clone()
            } else {
                // 用域名作为自定义供应商名
                host.clone()
            };

            discovered.push(DiscoveredProvider {
                name,
                base_url: base_url.clone(),
                protocol: protocol.clone(),
                env_key,
                has_key,
                is_preset: matched_preset.is_some(),
                preset_name: matched_preset.map(|p| p.name.clone()),
                models,
                valid: true,
                error: None,
            });
        }
    }

    Ok(discovered)
}

/// 将预设中已有但 settings.json 中缺失的模型补齐写入 settings.json
///
/// 匹配逻辑：按 baseUrl 域名 + authType 协议匹配预设
/// 返回补齐的模型数量
#[tauri::command]
pub fn sync_preset_models_to_settings(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let settings_path = config::user_settings_path();
    if !settings_path.exists() {
        return Err("settings.json 不存在".into());
    }

    let mut settings = config::read_settings(&settings_path)?;

    // 构建 env 查找表（优先级：settings.json env > .env > 系统环境变量）
    let mut env_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // 1. .env 文件（先插入，后被 settings.json env 覆盖）
    let env_path = crate::config::env_file_path();
    if env_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&env_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some((k, v)) = line.split_once('=') {
                    let key = k.trim().to_string();
                    let val = v.trim().to_string();
                    if !val.is_empty() {
                        env_map.insert(key, val);
                    }
                }
            }
        }
    }

    // 2. settings.json 的 env 对象（最高优先级，覆盖 .env）
    if let Some(env_obj) = settings.get("env").and_then(|e| e.as_object()) {
        for (k, v) in env_obj {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    env_map.insert(k.clone(), s.to_string());
                }
            }
        }
    }

    let model_providers = match settings.get_mut("modelProviders").and_then(|v| v.as_object_mut()) {
        Some(mp) => mp,
        None => return Ok(0),
    };

    // 加载预设
    let data_dir = crate::db::db_path(&std::path::PathBuf::from("."))
        .parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let presets = crate::presets::load_presets(&data_dir);

    // 构建预设匹配索引：(域名, 协议) → preset
    let mut preset_index: std::collections::HashMap<(String, String), &crate::presets::ProviderPreset> =
        std::collections::HashMap::new();
    for preset in &presets {
        let host = extract_host(&preset.base_url);
        if host.is_empty() { continue; }
        for m in &preset.models {
            for at in &m.auth_type {
                preset_index.insert((host.clone(), at.clone()), preset);
            }
        }
    }

    let mut added_count = 0usize;

    for (protocol, entries) in model_providers {
        let entries_arr = match entries.as_array_mut() {
            Some(a) => a,
            None => continue,
        };

        // 按 baseUrl 分组收集已有 model id
        let mut url_ids: std::collections::HashMap<String, std::collections::HashSet<String>> =
            std::collections::HashMap::new();
        for entry in entries_arr.iter() {
            let url = entry.get("baseUrl").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !url.is_empty() && !id.is_empty() {
                url_ids.entry(url).or_default().insert(id);
            }
        }

        for (base_url, existing_ids) in &url_ids {
            let host = extract_host(base_url);
            let key = (host, protocol.clone());
            let preset = match preset_index.get(&key) {
                Some(p) => p,
                None => continue,
            };

            // 找到该 baseUrl 下第一个条目作为模板
            let template = entries_arr.iter()
                .find(|e| e.get("baseUrl").and_then(|v| v.as_str()) == Some(base_url.as_str()));

            let template = match template {
                Some(t) => t.clone(),
                None => continue,
            };

            // 纠正已有模型的 generationConfig（预设数据覆盖内置错误值）
            for entry in entries_arr.iter_mut() {
                if entry.get("baseUrl").and_then(|v| v.as_str()) != Some(base_url.as_str()) {
                    continue;
                }
                let eid = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if eid.is_empty() { continue; }
                if let Some(pm) = preset.models.iter().find(|m| m.id == eid) {
                    // 仅当预设有显式字段时才覆盖，保留已有的 customHeaders 等
                    let has_preset_fields = pm.context_window_size.is_some()
                        || pm.max_output_tokens.is_some()
                        || pm.thinking.is_some()
                        || pm.input_modalities.is_some();
                    if !has_preset_fields { continue; }

                    let gen = entry
                        .as_object_mut().unwrap()
                        .entry("generationConfig")
                        .or_insert_with(|| serde_json::json!({}));
                    if let Some(gen_obj) = gen.as_object_mut() {
                        if let Some(cws) = pm.context_window_size {
                            gen_obj.insert("contextWindowSize".to_string(), serde_json::json!(cws));
                        }
                        if let Some(mot) = pm.max_output_tokens {
                            gen_obj.insert("samplingParams".to_string(), serde_json::json!({ "max_tokens": mot }));
                        }
                        if let Some(ref thinking) = pm.thinking {
                            gen_obj.insert("thinking".to_string(), thinking.clone());
                        }
                        if let Some(ref modalities) = pm.input_modalities {
                            let mods: serde_json::Map<String, serde_json::Value> = modalities.iter()
                                .map(|m| (m.clone(), serde_json::Value::Bool(true)))
                                .collect();
                            gen_obj.insert("modalities".to_string(), serde_json::Value::Object(mods));
                        }
                    }
                    added_count += 1; // 复用计数器标记有变更
                }
            }

            // 添加缺失的预设模型
            for pm in &preset.models {
                if existing_ids.contains(&pm.id) {
                    continue;
                }
                if !pm.auth_type.iter().any(|at| at == protocol) {
                    continue;
                }

                let mut new_entry = template.clone();
                new_entry["id"] = serde_json::Value::String(pm.id.clone());
                new_entry["name"] = serde_json::Value::String(pm.name.clone());

                // 从预设模型注入 generationConfig（contextWindowSize、maxOutputTokens、thinking、modalities）
                {
                    let gen = new_entry
                        .as_object_mut().unwrap()
                        .entry("generationConfig")
                        .or_insert_with(|| serde_json::json!({}));
                    if let Some(gen_obj) = gen.as_object_mut() {
                        if let Some(cws) = pm.context_window_size {
                            gen_obj.insert("contextWindowSize".to_string(), serde_json::json!(cws));
                        }
                        if let Some(mot) = pm.max_output_tokens {
                            gen_obj.insert("samplingParams".to_string(), serde_json::json!({ "max_tokens": mot }));
                        }
                        if let Some(ref thinking) = pm.thinking {
                            gen_obj.insert("thinking".to_string(), thinking.clone());
                        }
                        if let Some(ref modalities) = pm.input_modalities {
                            let mods: serde_json::Map<String, serde_json::Value> = modalities.iter()
                                .map(|m| (m.clone(), serde_json::Value::Bool(true)))
                                .collect();
                            gen_obj.insert("modalities".to_string(), serde_json::Value::Object(mods));
                        }
                    }
                }

                // 如果预设有非标准 authHeader（如 x-api-key），注入 generationConfig.customHeaders
                if let Some(ref auth_header) = preset.auth_header {
                    if auth_header.to_lowercase() != "authorization" {
                        // 从 settings.json 的 env 或 .env 获取实际 key 值
                        let env_key: String = new_entry.get("envKey")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&preset.env_prefix)
                            .to_string();
                        let key_value = env_map.get(&env_key).cloned();

                        if let Some(kv) = key_value {
                            if !kv.is_empty() {
                                let gen = new_entry
                                    .as_object_mut().unwrap()
                                    .entry("generationConfig")
                                    .or_insert_with(|| serde_json::json!({}));
                                if let Some(gen_obj) = gen.as_object_mut() {
                                    let headers = gen_obj
                                        .entry("customHeaders")
                                        .or_insert_with(|| serde_json::json!({}));
                                    if let Some(h) = headers.as_object_mut() {
                                        h.insert(auth_header.clone(), serde_json::json!(kv));
                                    }
                                }
                            }
                        }
                    }
                }

                entries_arr.push(new_entry);
                added_count += 1;
            }
        }
    }

    if added_count > 0 {
        config::write_settings(&settings_path, &settings)?;
        // 清空 model ID 缓存
        if let Ok(mut cache) = state.configured_model_ids.lock() {
            *cache = None;
        }
    }

    Ok(added_count)
}

// ── Configured Model IDs（settings.json 已配置模型列表） ──

/// 从 settings JSON 中提取所有去重排序的 model ID（纯函数，可测试）
pub fn extract_configured_model_ids(settings: &Value) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    if let Some(mp) = settings.get("modelProviders").and_then(|v| v.as_object()) {
        for (_protocol, entries) in mp {
            if let Some(arr) = entries.as_array() {
                for entry in arr {
                    if let Some(id) = entry.get("id").and_then(|v| v.as_str()) {
                        let id = id.to_string();
                        if !id.is_empty() && !ids.contains(&id) {
                            ids.push(id);
                        }
                    }
                }
            }
        }
    }
    ids.sort();
    ids
}

/// 从 settings.json 的 modelProviders 提取所有已配置的 model ID（带缓存）
#[tauri::command]
pub fn list_configured_model_ids(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    // 先查缓存
    if let Ok(cache) = state.configured_model_ids.lock() {
        if let Some(ref ids) = *cache {
            return Ok(ids.clone());
        }
    }

    let settings_path = config::user_settings_path();
    let settings = config::read_settings(&settings_path)
        .map_err(|e| format!("read settings failed: {e}"))?;

    let ids = extract_configured_model_ids(&settings);

    // 写入缓存
    if let Ok(mut cache) = state.configured_model_ids.lock() {
        *cache = Some(ids.clone());
    }

    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_state() -> AppState {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::init_db_with_conn(&conn);
        AppState {
            db: Arc::new(Mutex::new(conn)),
            mcp_shutdown: Arc::new(Mutex::new(None)),
            global_memory_cache: Arc::new(Mutex::new(Vec::new())),
            configured_model_ids: Arc::new(Mutex::new(None)),
        }
    }

    // ── extract_configured_model_ids 纯函数测试 ──────────

    #[test]
    fn extract_ids_from_normal_settings() {
        let settings: Value = serde_json::json!({
            "modelProviders": {
                "openai": [
                    { "id": "gpt-4o", "name": "GPT-4o", "baseUrl": "https://api.openai.com/v1" },
                    { "id": "gpt-4o-mini", "name": "GPT-4o Mini", "baseUrl": "https://api.openai.com/v1" }
                ],
                "anthropic": [
                    { "id": "claude-sonnet-4-20250514", "name": "Claude", "baseUrl": "https://api.anthropic.com" }
                ]
            }
        });
        let ids = extract_configured_model_ids(&settings);
        assert_eq!(ids, vec!["claude-sonnet-4-20250514", "gpt-4o", "gpt-4o-mini"]);
    }

    #[test]
    fn extract_ids_deduplicates_same_id_across_protocols() {
        let settings: Value = serde_json::json!({
            "modelProviders": {
                "openai": [
                    { "id": "mimo-v2.5", "name": "MiMo", "baseUrl": "https://api.xiaomimimo.com/v1" }
                ],
                "anthropic": [
                    { "id": "mimo-v2.5", "name": "MiMo", "baseUrl": "https://api.xiaomimimo.com" }
                ]
            }
        });
        let ids = extract_configured_model_ids(&settings);
        assert_eq!(ids, vec!["mimo-v2.5"]);
    }

    #[test]
    fn extract_ids_empty_when_no_model_providers() {
        let settings: Value = serde_json::json!({});
        let ids = extract_configured_model_ids(&settings);
        assert!(ids.is_empty());
    }

    #[test]
    fn extract_ids_empty_when_providers_empty() {
        let settings: Value = serde_json::json!({ "modelProviders": {} });
        let ids = extract_configured_model_ids(&settings);
        assert!(ids.is_empty());
    }

    #[test]
    fn extract_ids_skips_empty_id() {
        let settings: Value = serde_json::json!({
            "modelProviders": {
                "openai": [
                    { "id": "", "name": "empty" },
                    { "id": "gpt-4o", "name": "GPT-4o" },
                    { "name": "no-id-field" }
                ]
            }
        });
        let ids = extract_configured_model_ids(&settings);
        assert_eq!(ids, vec!["gpt-4o"]);
    }

    #[test]
    fn extract_ids_sorted_alphabetically() {
        let settings: Value = serde_json::json!({
            "modelProviders": {
                "openai": [
                    { "id": "zzz-last" },
                    { "id": "aaa-first" },
                    { "id": "mmm-middle" }
                ]
            }
        });
        let ids = extract_configured_model_ids(&settings);
        assert_eq!(ids, vec!["aaa-first", "mmm-middle", "zzz-last"]);
    }

    #[test]
    fn extract_ids_from_multiple_protocols() {
        let settings: Value = serde_json::json!({
            "modelProviders": {
                "openai": [
                    { "id": "deepseek-v4-pro" },
                    { "id": "kimi-k2.7" }
                ],
                "anthropic": [
                    { "id": "minimax-m3" },
                    { "id": "qwen3.7-max" }
                ]
            }
        });
        let ids = extract_configured_model_ids(&settings);
        assert_eq!(ids, vec!["deepseek-v4-pro", "kimi-k2.7", "minimax-m3", "qwen3.7-max"]);
    }

    // ── 缓存行为测试 ──────────────────────────────────────

    #[test]
    fn cache_initially_empty() {
        let state = make_state();
        let guard = state.configured_model_ids.lock().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn cache_populates_and_returns_same_data() {
        let state = make_state();
        // 手动填充缓存
        {
            let mut guard = state.configured_model_ids.lock().unwrap();
            *guard = Some(vec!["model-a".into(), "model-b".into()]);
        }
        // 读取
        {
            let guard = state.configured_model_ids.lock().unwrap();
            assert_eq!(*guard, Some(vec!["model-a".into(), "model-b".into()]));
        }
    }

    #[test]
    fn cache_clear_on_sync() {
        let state = make_state();
        // 填充
        {
            let mut guard = state.configured_model_ids.lock().unwrap();
            *guard = Some(vec!["old-model".into()]);
        }
        // 模拟 sync 后清空
        {
            let mut guard = state.configured_model_ids.lock().unwrap();
            *guard = None;
        }
        // 验证已清空
        {
            let guard = state.configured_model_ids.lock().unwrap();
            assert!(guard.is_none());
        }
    }

    // ── 临时文件集成测试：list_configured_model_ids 端到端 ─

    #[test]
    fn list_ids_from_temp_settings_file() {
        use std::io::Write;

        let dir = std::env::temp_dir().join("agentbox_test_model_ids");
        let _ = std::fs::create_dir_all(&dir);
        let settings_path = dir.join("settings.json");

        let settings_content = serde_json::json!({
            "modelProviders": {
                "openai": [
                    { "id": "gpt-4o", "baseUrl": "https://api.openai.com/v1" },
                    { "id": "glm-5.2", "baseUrl": "https://opencode.ai/zen/go/v1" }
                ]
            }
        });
        let mut f = std::fs::File::create(&settings_path).unwrap();
        write!(f, "{}", serde_json::to_string_pretty(&settings_content).unwrap()).unwrap();

        let settings = config::read_settings(&settings_path).unwrap();
        let ids = extract_configured_model_ids(&settings);
        assert_eq!(ids, vec!["glm-5.2", "gpt-4o"]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

// ── Proxy Status Commands ────────────────────────────────

const PROXY_PORT: u16 = 18900;

#[derive(Debug, serde::Serialize, Clone)]
pub struct ProxyStatus {
    pub running: bool,
    pub port: u16,
    pub uptime_hint: String,
}

/// 检测代理服务是否在运行（TCP 连通性）
#[tauri::command]
pub async fn get_proxy_status() -> Result<ProxyStatus, String> {
    let addr = format!("127.0.0.1:{}", PROXY_PORT);
    let running = tokio::net::TcpStream::connect(&addr).await.is_ok();
    Ok(ProxyStatus {
        running,
        port: PROXY_PORT,
        uptime_hint: if running { "运行中" } else { "未启动" }.to_string(),
    })
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ProviderModelStats {
    pub provider_id: i64,
    pub provider_name: String,
    pub base_url: String,
    pub model_id: String,
    pub call_count: i64,
    pub success_count: i64,
    pub failure_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub avg_duration_ms: f64,
    pub total_tokens_saved: i64,
    pub compressed_count: i64,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ProxyProviderStats {
    pub providers: Vec<ProviderModelStats>,
    pub total_calls: i64,
    pub total_failures: i64,
    pub total_tokens_saved: i64,
}

/// 获取代理服务的供应商调用统计
#[tauri::command]
pub fn get_proxy_provider_stats(
    state: tauri::State<'_, AppState>,
    days: Option<u32>,
) -> Result<ProxyProviderStats, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let days = days.unwrap_or(30);

    let mut stmt = db
        .prepare(
            "SELECT
                p.id, p.name, p.base_url,
                r.model_id,
                COUNT(*) as call_count,
                SUM(CASE WHEN r.status_code >= 200 AND r.status_code < 400 THEN 1 ELSE 0 END) as success_count,
                SUM(CASE WHEN r.status_code >= 400 OR r.status_code IS NULL THEN 1 ELSE 0 END) as failure_count,
                COALESCE(SUM(r.input_tokens), 0),
                COALESCE(SUM(r.output_tokens), 0),
                COALESCE(AVG(r.duration_ms), 0),
                COALESCE(SUM(r.tokens_saved), 0),
                SUM(CASE WHEN r.context_compressed = 1 THEN 1 ELSE 0 END)
             FROM request_logs r
             JOIN providers p ON r.provider_id = p.id
             WHERE r.timestamp >= datetime('now', '-' || ?1 || ' days')
             GROUP BY p.id, r.model_id
             ORDER BY p.name, call_count DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![days], |row| {
            Ok(ProviderModelStats {
                provider_id: row.get(0)?,
                provider_name: row.get(1)?,
                base_url: row.get(2)?,
                model_id: row.get(3)?,
                call_count: row.get(4)?,
                success_count: row.get(5)?,
                failure_count: row.get(6)?,
                total_input_tokens: row.get(7)?,
                total_output_tokens: row.get(8)?,
                avg_duration_ms: row.get(9)?,
                total_tokens_saved: row.get(10)?,
                compressed_count: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let providers: Vec<ProviderModelStats> = rows.filter_map(|r| r.ok()).collect();
    let total_calls = providers.iter().map(|p| p.call_count).sum();
    let total_failures = providers.iter().map(|p| p.failure_count).sum();
    let total_tokens_saved = providers.iter().map(|p| p.total_tokens_saved).sum();

    Ok(ProxyProviderStats {
        providers,
        total_calls,
        total_failures,
        total_tokens_saved,
    })
}

/// 重置指定供应商的调用计数（删除该供应商的 request_logs）
#[tauri::command]
pub fn reset_provider_counts(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
) -> Result<u64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let deleted = db
        .execute(
            "DELETE FROM request_logs WHERE provider_id = ?1",
            [provider_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(deleted as u64)
}

/// 从 URL 中提取域名（不依赖 url crate）
pub fn extract_host(url: &str) -> String {
    let s = url.trim();
    let after_scheme = if let Some(pos) = s.find("://") {
        &s[pos + 3..]
    } else {
        s
    };
    after_scheme.split('/').next().unwrap_or("")
        .split(':').next().unwrap_or("")
        .to_string()
}
