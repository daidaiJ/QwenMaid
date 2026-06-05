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

    // 启动新服务器
    let (tx, rx) = tokio::sync::watch::channel(false);
    {
        let mut handle = state.mcp_shutdown.lock().map_err(|e| e.to_string())?;
        *handle = Some(tx);
    }

    let db = state.db.clone();
    tokio::spawn(async move {
        if let Err(e) = mcp::start_mcp_server(db, rx).await {
            log::error!("MCP server failed: {}", e);
        }
    });

    Ok(())
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
                "type": "http",
                "url": format!("http://localhost:{}/mcp", port)
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
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e: tauri::Error| e.to_string())?;
    let cmd_path = resource_dir.join("cli").join("statusline.cmd");

    if !cmd_path.exists() {
        return Err(format!("statusline.cmd not found at {:?}", cmd_path));
    }

    let settings_path = config::user_settings_path();
    let mut settings = if settings_path.exists() {
        config::read_settings(&settings_path)?
    } else {
        json!({})
    };

    let command_str = format!("cmd /c \"{}\"", cmd_path.to_string_lossy().replace('/', "\\"));
    config::set_by_path(
        &mut settings,
        "statusLine",
        json!({
            "type": "command",
            "command": command_str
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

    config::set_by_path(&mut settings, "statusLine", Value::Null);
    config::write_settings(&settings_path, &settings)
}

// ── Provider Discovery ───────────────────────────────────

#[derive(serde::Serialize)]
pub struct DiscoveredModel {
    pub id: String,
    pub name: String,
    pub auth_type: Vec<String>,
    pub valid: bool,
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

    // 构建 API Key 查找表（优先级：.env > settings.json env > 系统环境变量）
    let mut key_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // 1. 系统环境变量（最低优先级）
    for (k, v) in std::env::vars() {
        key_map.insert(k, v);
    }

    // 2. settings.json 的 env 对象
    if let Some(env_obj) = settings.get("env").and_then(|v| v.as_object()) {
        for (k, v) in env_obj {
            if let Some(s) = v.as_str() {
                key_map.insert(k.clone(), s.to_string());
            }
        }
    }

    // 3. .env 文件（最高优先级）
    let env_path = config::env_file_path();
    if env_path.exists() {
        if let Ok(content) = fs::read_to_string(&env_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some((k, v)) = line.split_once('=') {
                    key_map.insert(k.trim().to_string(), v.trim().to_string());
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

            // 解析 models
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
                models.push(DiscoveredModel {
                    id: id.to_string(),
                    name: name.to_string(),
                    auth_type: auth_types,
                    valid: true,
                });
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

/// 从 URL 中提取域名（不依赖 url crate）
fn extract_host(url: &str) -> String {
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
