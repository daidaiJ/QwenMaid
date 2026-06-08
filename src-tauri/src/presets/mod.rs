use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 内嵌的默认预设（编译时打包进二进制）
const DEFAULT_PRESETS: &str = include_str!("../../resources/provider-presets.json");

/// 预设供应商模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPreset {
    pub name: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(rename = "envPrefix")]
    pub env_prefix: String,
    #[serde(rename = "proxyMode")]
    pub proxy_mode: Option<String>,
    #[serde(rename = "billingType")]
    pub billing_type: Option<String>,
    #[serde(rename = "authHeader")]
    pub auth_header: Option<String>,
    pub models: Vec<ModelPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreset {
    pub id: String,
    pub name: String,
    #[serde(rename = "authType")]
    pub auth_type: Vec<String>,
    #[serde(rename = "contextWindowSize")]
    pub context_window_size: Option<u64>,
    #[serde(rename = "maxOutputTokens")]
    pub max_output_tokens: Option<u64>,
    #[serde(rename = "inputModalities")]
    pub input_modalities: Option<Vec<String>>,
}

/// 预设版本元数据（用于更新检查）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetsMeta {
    pub version: u32,
    pub updated_at: String,
    pub source: String,
}

/// 加载预设列表
///
/// 优先级：本地覆盖文件 > 内嵌默认
pub fn load_presets(app_data_dir: &PathBuf) -> Vec<ProviderPreset> {
    let local_path = app_data_dir.join("provider-presets.json");

    let json_str = if local_path.exists() {
        fs::read_to_string(&local_path).unwrap_or_else(|_| DEFAULT_PRESETS.to_string())
    } else {
        DEFAULT_PRESETS.to_string()
    };

    serde_json::from_str(&json_str).unwrap_or_default()
}

/// 获取内嵌预设版本（简单 hash 用于比对）
pub fn embedded_presets_hash() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    DEFAULT_PRESETS.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// 检查远程是否有新版本预设
///
/// 从 GitHub raw 文件获取最新预设，与本地比对
pub async fn check_presets_update() -> Result<Option<String>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    // 从 GitHub raw 获取最新预设文件
    let url = "https://raw.githubusercontent.com/daidaiJ/QwenMaid/main/src-tauri/resources/provider-presets.json";

    let resp = client
        .get(url)
        .header("User-Agent", "AgentBox/0.1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let remote_body = resp.text().await.map_err(|e| e.to_string())?;

    // 简单比对：内容不同则返回新内容
    if remote_body.trim() != DEFAULT_PRESETS.trim() {
        Ok(Some(remote_body))
    } else {
        Ok(None)
    }
}

/// 保存远程更新到本地
pub fn save_presets_update(app_data_dir: &PathBuf, content: &str) -> Result<(), String> {
    // 先验证是合法 JSON
    let _: Vec<ProviderPreset> = serde_json::from_str(content).map_err(|e| e.to_string())?;

    let path = app_data_dir.join("provider-presets.json");
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_embedded_presets() {
        let presets: Vec<ProviderPreset> = serde_json::from_str(DEFAULT_PRESETS).unwrap();
        assert!(!presets.is_empty());
        assert!(presets.iter().any(|p| p.name == "OpenAI"));
        assert!(presets.iter().any(|p| p.name == "Xiaomi MiMo (Anthropic)"));
        assert!(presets.iter().any(|p| p.name == "OpenCode Go (OpenAI)"));
    }

    #[test]
    fn test_preset_has_models() {
        let presets: Vec<ProviderPreset> = serde_json::from_str(DEFAULT_PRESETS).unwrap();
        for p in &presets {
            assert!(!p.models.is_empty(), "{} has no models", p.name);
        }
    }

    #[test]
    fn test_embedded_hash_stable() {
        let h1 = embedded_presets_hash();
        let h2 = embedded_presets_hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_mimo_anthropic_has_auth_header() {
        let presets: Vec<ProviderPreset> = serde_json::from_str(DEFAULT_PRESETS).unwrap();
        let mimo_anth = presets.iter().find(|p| p.name == "Xiaomi MiMo (Anthropic)").unwrap();
        assert_eq!(mimo_anth.auth_header.as_deref(), Some("api-key"));
    }

    #[test]
    fn test_opencode_go_presets_valid() {
        let presets: Vec<ProviderPreset> = serde_json::from_str(DEFAULT_PRESETS).unwrap();

        // ── OpenCode Go (OpenAI) ──
        let oc_openai = presets.iter().find(|p| p.name == "OpenCode Go (OpenAI)").unwrap();
        assert_eq!(oc_openai.env_prefix, "OPENCODE_API_KEY");
        let oc_openai_ids: Vec<&str> = oc_openai.models.iter().map(|m| m.id.as_str()).collect();
        for expected in &["glm-5.1", "kimi-k2.6", "kimi-k2.5", "deepseek-v4-pro", "deepseek-v4-flash", "mimo-v2.5", "mimo-v2.5-pro"] {
            assert!(oc_openai_ids.contains(expected), "OpenCode Go (OpenAI) 缺少模型: {}", expected);
        }
        // model ID 唯一性
        let mut sorted = oc_openai_ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), oc_openai_ids.len(), "OpenCode Go (OpenAI) 存在重复 model ID");
        // gap 文档要求的显式配置
        let mimo25 = oc_openai.models.iter().find(|m| m.id == "mimo-v2.5").unwrap();
        assert_eq!(mimo25.context_window_size, Some(1000000));
        assert_eq!(mimo25.max_output_tokens, Some(128000));
        let mimo25pro = oc_openai.models.iter().find(|m| m.id == "mimo-v2.5-pro").unwrap();
        assert_eq!(mimo25pro.context_window_size, Some(1000000));
        assert_eq!(mimo25pro.max_output_tokens, Some(128000));
        let glm51 = oc_openai.models.iter().find(|m| m.id == "glm-5.1").unwrap();
        assert_eq!(glm51.max_output_tokens, Some(131072));
        let dspro = oc_openai.models.iter().find(|m| m.id == "deepseek-v4-pro").unwrap();
        assert_eq!(dspro.max_output_tokens, Some(384000));
        let kk26 = oc_openai.models.iter().find(|m| m.id == "kimi-k2.6").unwrap();
        assert_eq!(kk26.max_output_tokens, Some(262144));

        // ── OpenCode Go (Anthropic) ──
        let oc_anth = presets.iter().find(|p| p.name == "OpenCode Go (Anthropic)").unwrap();
        assert_eq!(oc_anth.env_prefix, "OPENCODE_API_KEY");
        assert_eq!(oc_anth.auth_header.as_deref(), Some("x-api-key"));
        let oc_anth_ids: Vec<&str> = oc_anth.models.iter().map(|m| m.id.as_str()).collect();
        for expected in &["minimax-m3", "minimax-m2.7", "qwen3.7-max", "qwen3.6-plus"] {
            assert!(oc_anth_ids.contains(expected), "OpenCode Go (Anthropic) 缺少模型: {}", expected);
        }
        let mut sorted = oc_anth_ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), oc_anth_ids.len(), "OpenCode Go (Anthropic) 存在重复 model ID");
        // gap 文档要求的显式配置
        let mm3 = oc_anth.models.iter().find(|m| m.id == "minimax-m3").unwrap();
        assert_eq!(mm3.max_output_tokens, Some(512000));
        let mm27 = oc_anth.models.iter().find(|m| m.id == "minimax-m2.7").unwrap();
        assert_eq!(mm27.max_output_tokens, Some(196608));
    }

    #[test]
    fn test_preset_model_config_json_from_discovery() {
        // 模拟发现流程：从预设模型构建 config_json
        let presets: Vec<ProviderPreset> = serde_json::from_str(DEFAULT_PRESETS).unwrap();
        let oc_openai = presets.iter().find(|p| p.name == "OpenCode Go (OpenAI)").unwrap();
        let mimo_model = oc_openai.models.iter().find(|m| m.id == "mimo-v2.5").unwrap();

        // 构建 config_json（与 commands/mod.rs 中的逻辑一致）
        let mut gen = serde_json::Map::new();
        if let Some(cws) = mimo_model.context_window_size {
            gen.insert("contextWindowSize".to_string(), serde_json::json!(cws));
        }
        if let Some(mot) = mimo_model.max_output_tokens {
            gen.insert("samplingParams".to_string(), serde_json::json!({ "max_tokens": mot }));
        }
        if let Some(ref modalities) = mimo_model.input_modalities {
            let mods: serde_json::Map<String, serde_json::Value> = modalities.iter()
                .map(|m| (m.clone(), serde_json::Value::Bool(true)))
                .collect();
            gen.insert("modalities".to_string(), serde_json::Value::Object(mods));
        }
        let config_json = serde_json::to_string(&gen).unwrap();

        // 验证 config_json 可被正确解析回 generationConfig
        let parsed: serde_json::Value = serde_json::from_str(&config_json).unwrap();
        assert_eq!(parsed["contextWindowSize"], serde_json::json!(1000000));
        assert_eq!(parsed["samplingParams"]["max_tokens"], serde_json::json!(128000));
        assert_eq!(parsed["modalities"]["text"], serde_json::json!(true));
        assert_eq!(parsed["modalities"]["image"], serde_json::json!(true));
        assert_eq!(parsed["modalities"]["audio"], serde_json::json!(true));
        assert_eq!(parsed["modalities"]["video"], serde_json::json!(true));
    }

    #[test]
    fn test_dump_presets_for_review() {
        // 将预设转换为 Qwen Code settings.json 的 modelProviders 格式输出
        let presets: Vec<ProviderPreset> = serde_json::from_str(DEFAULT_PRESETS).unwrap();

        let mut model_providers: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

        for preset in &presets {
            for model in &preset.models {
                for auth_type in &model.auth_type {
                    let mut entry = serde_json::json!({
                        "id": model.id,
                        "name": model.name,
                        "envKey": preset.env_prefix,
                        "baseUrl": preset.base_url,
                    });

                    // 只有预设显式设置了字段才构建 generationConfig
                    let mut gen = serde_json::Map::new();
                    if let Some(cws) = model.context_window_size {
                        gen.insert("contextWindowSize".to_string(), serde_json::json!(cws));
                    }
                    if let Some(mot) = model.max_output_tokens {
                        gen.insert("samplingParams".to_string(), serde_json::json!({ "max_tokens": mot }));
                    }
                    if let Some(ref modalities) = model.input_modalities {
                        let mods: serde_json::Map<String, serde_json::Value> = modalities.iter()
                            .map(|m| (m.clone(), serde_json::Value::Bool(true)))
                            .collect();
                        gen.insert("modalities".to_string(), serde_json::Value::Object(mods));
                    }
                    // Anthropic 协议 + 非标准 authHeader → 注入 customHeaders
                    if auth_type == "anthropic" {
                        if let Some(ref header) = preset.auth_header {
                            if header.to_lowercase() != "authorization" {
                                let mut headers = serde_json::Map::new();
                                headers.insert(header.clone(), serde_json::json!("sk-your-api-key-here"));
                                gen.insert("customHeaders".to_string(), serde_json::Value::Object(headers));
                            }
                        }
                    }
                    if !gen.is_empty() {
                        entry.as_object_mut().unwrap().insert(
                            "generationConfig".to_string(),
                            serde_json::Value::Object(gen),
                        );
                    }

                    let list = model_providers
                        .entry(auth_type.clone())
                        .or_insert_with(|| serde_json::json!([]));
                    list.as_array_mut().unwrap().push(entry);
                }
            }
        }

        let output = serde_json::json!({ "modelProviders": model_providers });
        let pretty = serde_json::to_string_pretty(&output).unwrap();
        let out_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("provider-presets-review.json");
        std::fs::write(&out_path, &pretty).unwrap();

        let content = std::fs::read_to_string(&out_path).unwrap();
        assert!(!content.is_empty());
        assert!(content.contains("customHeaders"));
    }
}
