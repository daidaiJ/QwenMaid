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
}
