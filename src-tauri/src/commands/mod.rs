pub mod analytics;
pub mod filesystem;
pub mod installer;
pub mod metrics;
pub mod skill_marketplace;

use rusqlite::Connection;
use serde_json::{json, Map, Value};
use std::sync::{Arc, Mutex};

use crate::config;
use crate::db::providers::{self, CreateModel, CreateProvider, Model, Provider, UpdateModel, UpdateProvider};
use crate::presets;

/// Tauri 命令共享状态
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
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

/// 获取全局分析汇总（从 SQLite 读取，不触发解析）
#[tauri::command]
pub fn get_analytics_summary(
    state: tauri::State<'_, AppState>,
) -> Result<analytics::AnalyticsSummary, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    analytics::get_analytics_summary(&db)
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
