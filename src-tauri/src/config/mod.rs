use rusqlite::Connection;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::PathBuf;

use crate::db::providers;

/// 用户级 settings.json 路径
pub fn user_settings_path() -> PathBuf {
    dirs_or_default().join(".qwen").join("settings.json")
}

/// .env 文件路径
pub fn env_file_path() -> PathBuf {
    dirs_or_default().join(".qwen").join(".env")
}

fn dirs_or_default() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// 读取 settings.json（支持 JSONC → 先剥离注释再解析）
pub fn read_settings(path: &PathBuf) -> Result<Value, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let stripped = strip_jsonc_comments(&content);
    serde_json::from_str(&stripped).map_err(|e| e.to_string())
}

/// 按点号路径设置值（如 "model.name"、"ui.statusLine.type"）
/// value 为 Null 时删除该字段
pub fn set_by_path(root: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    set_by_path_inner(root, &parts, value);
}

fn set_by_path_inner(node: &mut Value, parts: &[&str], value: Value) {
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        let key = parts[0];
        if let Value::Null = value {
            if let Some(obj) = node.as_object_mut() {
                obj.remove(key);
            }
        } else if let Some(obj) = node.as_object_mut() {
            obj.insert(key.to_string(), value);
        }
        return;
    }
    let key = parts[0];
    let rest = &parts[1..];
    if node.get(key).is_none() || !node.get(key).unwrap().is_object() {
        if let Some(obj) = node.as_object_mut() {
            obj.insert(key.to_string(), Value::Object(Map::new()));
        }
    }
    if let Some(child) = node.get_mut(key) {
        set_by_path_inner(child, rest, value);
    }
}

/// 写入 settings.json（写入前自动备份）
pub fn write_settings(path: &PathBuf, value: &Value) -> Result<(), String> {
    // 备份
    if path.exists() {
        let ts = chrono::Local::now().format("%Y%m%dT%H%M%S");
        let backup_dir = path
            .parent()
            .unwrap_or(path)
            .join("backup");
        fs::create_dir_all(&backup_dir).ok();
        let backup_path = backup_dir.join(format!("settings.json.{}", ts));
        fs::copy(path, &backup_path).ok();
    }

    let content = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

// ── 同步校验 ─────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncValidation {
    pub valid: bool,
    pub errors: Vec<SyncError>,
    pub warnings: Vec<SyncWarning>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncError {
    pub field: String,
    pub provider_id: Option<i64>,
    pub model_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncWarning {
    pub field: String,
    pub env_key: String,
    pub message: String,
}

/// 校验 providers/models 数据完整性
fn validate_providers(
    db: &Connection,
    settings: &Value,
) -> SyncValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let all_providers = match providers::list_providers(db) {
        Ok(p) => p,
        Err(e) => {
            errors.push(SyncError {
                field: "database".into(),
                provider_id: None,
                model_id: None,
                message: e,
            });
            return SyncValidation { valid: false, errors, warnings };
        }
    };

    let all_models = match providers::list_models(db, None) {
        Ok(m) => m,
        Err(e) => {
            errors.push(SyncError {
                field: "database".into(),
                provider_id: None,
                model_id: None,
                message: e,
            });
            return SyncValidation { valid: false, errors, warnings };
        }
    };

    let env = settings.get("env").and_then(|v| v.as_object());

    for provider in &all_providers {
        if !provider.is_active {
            continue;
        }
        if provider.api_key_env.is_empty() {
            errors.push(SyncError {
                field: "api_key_env".into(),
                provider_id: Some(provider.id),
                model_id: None,
                message: format!("供应商 '{}' 的环境变量名为空", provider.name),
            });
        }
        if provider.base_url.is_empty() {
            errors.push(SyncError {
                field: "base_url".into(),
                provider_id: Some(provider.id),
                model_id: None,
                message: format!("供应商 '{}' 的 baseUrl 为空", provider.name),
            });
        }
        // 检查 env 中是否有对应的 key
        if !provider.api_key_env.is_empty() {
            let has_env = env
                .and_then(|e| e.get(&provider.api_key_env))
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            let has_db_key = provider.api_key_value.is_some();
            if !has_env && !has_db_key {
                warnings.push(SyncWarning {
                    field: "env".into(),
                    env_key: provider.api_key_env.clone(),
                    message: format!(
                        "供应商 '{}' 的 API Key ({}) 在 settings.json 和数据库中均未配置",
                        provider.name, provider.api_key_env
                    ),
                });
            }
        }
    }

    for model in &all_models {
        if model.model_id.is_empty() {
            let provider_name = all_providers
                .iter()
                .find(|p| p.id == model.provider_id)
                .map(|p| p.name.as_str())
                .unwrap_or("unknown");
            errors.push(SyncError {
                field: "model_id".into(),
                provider_id: Some(model.provider_id),
                model_id: Some(model.model_id.clone()),
                message: format!("供应商 '{}' 的模型 ID 为空", provider_name),
            });
        }
    }

    let valid = errors.is_empty();
    SyncValidation { valid, errors, warnings }
}

/// 写入 .env 文件（备份旧文件）
pub fn write_env_file(keys: &Map<String, Value>) -> Result<(), String> {
    let path = env_file_path();
    if path.exists() {
        let ts = chrono::Local::now().format("%Y%m%dT%H%M%S");
        let backup = path.with_extension(format!("env.{}", ts));
        fs::copy(&path, &backup).ok();
    }

    let mut content = String::new();
    for (k, v) in keys {
        if let Some(val) = v.as_str() {
            content.push_str(&format!("{}={}\n", k, val));
        }
    }
    fs::write(&path, content).map_err(|e| e.to_string())
}

/// 从 DB 读取 api_key_value（明文）生成 env 键值对
fn build_env_from_db(db: &Connection) -> Map<String, Value> {
    let mut env = Map::new();
    let all_providers = match providers::list_providers(db) {
        Ok(p) => p,
        Err(_) => return env,
    };

    for provider in &all_providers {
        if !provider.is_active || provider.api_key_env.is_empty() {
            continue;
        }
        if let Some(ref key) = provider.api_key_value {
            if !key.is_empty() {
                env.insert(provider.api_key_env.clone(), Value::String(key.clone()));
            }
        }
    }

    env
}

// ── 同步主逻辑 ───────────────────────────────────────────

/// 从 DB 同步 providers/models 到 settings.json 的 modelProviders 段
///
/// 合并规则：
/// 1. 校验数据完整性
/// 2. 从 DB 生成新的 modelProviders 段
/// 3. 根据 envStorageMode 决定 env 写入位置
/// 4. 写回
/// 同步结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncResult {
    pub validation: SyncValidation,
    pub settings: Option<Value>,
}

pub fn sync_providers_to_settings(
    settings_path: &PathBuf,
    db: &Connection,
) -> Result<SyncResult, String> {
    let mut settings = if settings_path.exists() {
        read_settings(settings_path)?
    } else {
        json!({})
    };

    // 校验
    let validation = validate_providers(db, &settings);
    if !validation.valid {
        return Ok(SyncResult {
            validation,
            settings: None,
        });
    }

    // 加载预设（用于 direct 模式下查找原始 baseUrl）
    let data_dir = crate::db::db_path(&std::path::PathBuf::from("."))
        .parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let presets = crate::presets::load_presets(&data_dir);

    // 从 DB 读取所有 active providers 和 models
    let all_providers = providers::list_providers(db)?;
    let all_models = providers::list_models(db, None)?;

    // 按 authType 分组生成 modelProviders
    let mut new_providers: Map<String, Value> = Map::new();

    for model in &all_models {
        let provider = all_providers.iter().find(|p| p.id == model.provider_id);
        let provider = match provider {
            Some(p) if p.is_active => p,
            _ => continue,
        };

        // auth_type 是 JSON 数组，如 ["openai","anthropic"]
        let auth_types: Vec<String> =
            serde_json::from_str(&model.auth_type).unwrap_or_else(|_| {
                vec![model.auth_type.clone()]
            });

        for auth_type in &auth_types {
            let effective_url = get_effective_base_url(provider, auth_type, &presets);
            let mut entry = json!({
                "id": model.model_id,
                "name": model.display_name.as_deref().unwrap_or(&model.model_id),
                "envKey": provider.api_key_env,
                "baseUrl": effective_url,
            });

            // 从 config_json 合并 generationConfig（含 contextWindowSize、extra_body 等）
            if let Some(ref cj) = model.config_json {
                if let Ok(gen) = serde_json::from_str::<Value>(cj) {
                    if gen.is_object() && !gen.as_object().unwrap().is_empty() {
                        entry.as_object_mut().unwrap().insert(
                            "generationConfig".to_string(),
                            gen,
                        );
                    }
                }
            }

            // 如果预设有非标准 authHeader，注入 customHeaders（用实际 key 值）
            let du = provider.base_url.trim_end_matches('/');
            let provider_host = crate::commands::extract_host(&provider.base_url);
            let matched_preset = presets.iter().find(|p| {
                p.base_url.trim_end_matches('/') == du
                    && p.models.iter().any(|m| m.auth_type.iter().any(|at| at == auth_type.as_str()))
            }).or_else(|| presets.iter().find(|p| {
                crate::commands::extract_host(&p.base_url) == provider_host
                    && p.models.iter().any(|m| m.auth_type.iter().any(|at| at == auth_type.as_str()))
            }));
            if let Some(preset) = matched_preset {
                if let Some(ref auth_header) = preset.auth_header {
                    if auth_header.to_lowercase() != "authorization" {
                        if let Some(ref api_key) = provider.api_key_value {
                            if !api_key.is_empty() {
                                let gen = entry
                                    .as_object_mut().unwrap()
                                    .entry("generationConfig".to_string())
                                    .or_insert_with(|| json!({}));
                                if let Some(gen_obj) = gen.as_object_mut() {
                                    let headers = gen_obj
                                        .entry("customHeaders".to_string())
                                        .or_insert_with(|| json!({}));
                                    if let Some(h) = headers.as_object_mut() {
                                        h.insert(auth_header.clone(), json!(api_key));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let list = new_providers
                .entry(auth_type.clone())
                .or_insert_with(|| json!([]));

            if let Some(arr) = list.as_array_mut() {
                // 用 effective_url 做 dedup，避免 baseUrl 不一致导致重复
                let existing_idx = arr.iter().position(|e| {
                    e.get("id").and_then(|v| v.as_str()) == Some(&model.model_id)
                        && e.get("baseUrl").and_then(|v| v.as_str()) == Some(effective_url.as_str())
                });

                if let Some(idx) = existing_idx {
                    arr[idx] = entry;
                } else {
                    arr.push(entry);
                }
            }
        }
    }

    // 合并到 settings（只替换 modelProviders 段）
    settings["modelProviders"] = Value::Object(new_providers);

    // 从 DB 构建 env（解密 api_key_value）
    let db_env = build_env_from_db(db);
    if !db_env.is_empty() {
        // 合并到 settings 的 env 字段
        let env = settings
            .as_object_mut()
            .unwrap()
            .entry("env")
            .or_insert_with(|| json!({}));
        if let Some(env_obj) = env.as_object_mut() {
            for (k, v) in &db_env {
                env_obj.insert(k.clone(), v.clone());
            }
        }
    }

    Ok(SyncResult {
        validation,
        settings: Some(settings),
    })
}

/// 写入 settings.json 的 baseUrl：始终使用预设原始地址
/// 匹配逻辑：先精确匹配 baseUrl + authType，再降级到域名匹配
fn get_effective_base_url(
    provider: &providers::Provider,
    auth_type: &str,
    presets: &[crate::presets::ProviderPreset],
) -> String {
    let du = provider.base_url.trim_end_matches('/');

    // 1. 精确匹配 baseUrl + 该协议下有模型
    if let Some(p) = presets.iter().find(|p| {
        p.base_url.trim_end_matches('/') == du
            && p.models.iter().any(|m| m.auth_type.iter().any(|at| at == auth_type))
    }) {
        return p.base_url.clone();
    }

    // 2. 域名匹配 + 该协议下有模型
    let provider_host = crate::commands::extract_host(&provider.base_url);
    if let Some(p) = presets.iter().find(|p| {
        crate::commands::extract_host(&p.base_url) == provider_host
            && p.models.iter().any(|m| m.auth_type.iter().any(|at| at == auth_type))
    }) {
        return p.base_url.clone();
    }

    provider.base_url.clone()
}

/// 剥离 JSONC 注释（// 和 /* */）
fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        if in_string {
            result.push(chars[i]);
            if chars[i] == '\\' && i + 1 < len {
                i += 1;
                result.push(chars[i]);
            } else if chars[i] == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if chars[i] == '"' {
            in_string = true;
            result.push(chars[i]);
            i += 1;
            continue;
        }

        if chars[i] == '/' && i + 1 < len {
            if chars[i + 1] == '/' {
                // 行注释
                i += 2;
                while i < len && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if chars[i + 1] == '*' {
                // 块注释
                i += 2;
                while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_line_comment() {
        let input = r#"{"key": "value" // comment
}"#;
        let stripped = strip_jsonc_comments(input);
        let v: Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(v["key"], "value");
    }

    #[test]
    fn test_strip_block_comment() {
        let input = r#"/* block */ {"key": "value"}"#;
        let stripped = strip_jsonc_comments(input);
        let v: Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(v["key"], "value");
    }

    #[test]
    fn test_strip_comment_in_string() {
        let input = r#"{"url": "http://example.com // not a comment"}"#;
        let stripped = strip_jsonc_comments(input);
        let v: Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(v["url"], "http://example.com // not a comment");
    }

    #[test]
    fn test_sync_providers_empty_db() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_db_with_conn(&conn);
        let result = sync_providers_to_settings(&PathBuf::from("/nonexistent"), &conn);
        assert!(result.is_ok());
        let sync_result = result.unwrap();
        assert!(sync_result.validation.valid);
        let settings = sync_result.settings.unwrap();
        assert_eq!(settings["modelProviders"], json!({}));
    }

    #[test]
    fn test_sync_providers_with_data() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_db_with_conn(&conn);

        let p = providers::create_provider(
            &conn,
            &providers::CreateProvider {
                name: "openai".into(),
                base_url: "https://api.openai.com".into(),
                api_key_env: "OPENAI_API_KEY".into(),
                proxy_mode: Some("direct".into()),
                proxy_url: None,
                auth_header: None,
                api_key_value: None,
                billing_type: Some("pay_per_use".into()),
                compress_enabled: None,
            },
        )
        .unwrap();

        providers::create_model(
            &conn,
            &providers::CreateModel {
                provider_id: p.id,
                model_id: "gpt-4o".into(),
                display_name: Some("GPT-4o".into()),
                auth_type: r#"["openai"]"#.into(),
                is_default: Some(true),
                config_json: None,
            },
        )
        .unwrap();

        let sync_result = sync_providers_to_settings(&PathBuf::from("/nonexistent"), &conn).unwrap();
        let settings = sync_result.settings.unwrap();
        let openai = &settings["modelProviders"]["openai"];
        assert!(openai.is_array());
        assert_eq!(openai[0]["id"], "gpt-4o");
        assert_eq!(openai[0]["baseUrl"], "https://api.openai.com");
        assert_eq!(openai[0]["envKey"], "OPENAI_API_KEY");
    }

    #[test]
    fn test_sync_always_uses_preset_base_url() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_db_with_conn(&conn);

        let p = providers::create_provider(
            &conn,
            &providers::CreateProvider {
                name: "openai-proxy".into(),
                base_url: "https://api.openai.com".into(),
                api_key_env: "OPENAI_API_KEY".into(),
                proxy_mode: Some("system".into()),
                proxy_url: None,
                auth_header: None,
                api_key_value: None,
                billing_type: Some("pay_per_use".into()),
                compress_enabled: None,
            },
        )
        .unwrap();

        providers::create_model(
            &conn,
            &providers::CreateModel {
                provider_id: p.id,
                model_id: "gpt-4o".into(),
                display_name: None,
                auth_type: r#"["openai"]"#.into(),
                is_default: Some(false),
                config_json: None,
            },
        )
        .unwrap();

        let sync_result = sync_providers_to_settings(&PathBuf::from("/nonexistent"), &conn).unwrap();
        let settings = sync_result.settings.unwrap();
        // baseUrl 始终使用预设原始地址，不因代理模式而改变
        assert_eq!(
            settings["modelProviders"]["openai"][0]["baseUrl"],
            "https://api.openai.com"
        );
    }
}
