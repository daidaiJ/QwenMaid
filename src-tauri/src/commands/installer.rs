use serde::Serialize;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use tauri::{Emitter, Window};
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Clone, Serialize)]
struct InstallProgress {
    line: String,
    source: String, // "stdout" | "stderr"
}

/// 执行 npm install/update -g，逐行 emit 进度事件
async fn run_npm_lifecycle(
    action: &str,
    mirror: Option<String>,
    window: Window,
) -> Result<String, String> {
    let mut args: Vec<String> = vec![
        "/C".into(),
        "npm".into(),
        action.into(),
        "-g".into(),
        "@qwen-code/qwen-code@latest".into(),
    ];

    if let Some(reg) = mirror {
        args.push(format!("--registry={}", reg));
    }

    let mut child = if cfg!(target_os = "windows") {
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        cmd.spawn()
            .map_err(|e| format!("failed to spawn npm {}: {}", action, e))?
    } else {
        tokio::process::Command::new("npm")
            .args(&args[2..])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn npm {}: {}", action, e))?
    };

    // 并行读取 stdout 和 stderr
    if let Some(stdout) = child.stdout.take() {
        let window = window.clone();
        let mut reader = BufReader::new(stdout).lines();
        tokio::spawn(async move {
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = window.emit("install-progress", InstallProgress {
                    line: line.clone(),
                    source: "stdout".into(),
                });
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let window = window.clone();
        let mut reader = BufReader::new(stderr).lines();
        tokio::spawn(async move {
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = window.emit("install-progress", InstallProgress {
                    line: line.clone(),
                    source: "stderr".into(),
                });
            }
        });
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("npm {} process error: {}", action, e))?;

    if status.success() {
        Ok(format!("npm {} completed successfully", action))
    } else {
        Err(format!("npm {} failed with exit code: {:?}", action, status.code()))
    }
}

// ── 版本检测 ─────────────────────────────────────────────

/// 探测本地 Qwen Code 版本
#[tauri::command]
pub fn detect_qwen_version() -> Option<String> {
    detect_cli_version("qwen", &["--version"])
}

/// 探测 Node.js 版本
#[tauri::command]
pub fn detect_node_version() -> Option<ToolVersion> {
    detect_tool("node", &["--version"])
}

/// 探测 npm 版本
#[tauri::command]
pub fn detect_npm_version() -> Option<ToolVersion> {
    detect_tool("npm", &["--version"])
}

/// 从 GitHub API 获取最新稳定版本 tag
#[tauri::command]
pub async fn check_latest_qwen_version() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get("https://api.github.com/repos/QwenLM/qwen-code/releases")
        .header("User-Agent", "AgentBox/0.1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let releases: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;

    for release in &releases {
        if let Some(tag) = release.get("tag_name").and_then(|v| v.as_str()) {
            if is_stable_tag(tag) {
                return Ok(tag.to_string());
            }
        }
    }

    Err("no stable release found".to_string())
}

// ── 安装/更新操作 ────────────────────────────────────────

/// 安装 Qwen Code（npm install -g），流式输出进度
#[tauri::command]
pub async fn install_qwen_code(window: Window, mirror: Option<String>) -> Result<String, String> {
    run_npm_lifecycle("install", mirror, window).await
}

/// 更新 Qwen Code（npm install -g @latest），流式输出进度
#[tauri::command]
pub async fn update_qwen_code(window: Window, mirror: Option<String>) -> Result<String, String> {
    run_npm_lifecycle("install", mirror, window).await
}

/// 配置 npm 镜像源
#[tauri::command]
pub fn configure_npm_mirror(registry: String) -> Result<(), String> {
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "npm", "config", "set", "registry", &registry])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
    } else {
        std::process::Command::new("npm")
            .args(["config", "set", "registry", &registry])
            .output()
    };

    let output = output.map_err(|e| format!("failed to run npm config: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// 获取当前 npm 镜像源
#[tauri::command]
pub fn get_npm_mirror() -> Result<String, String> {
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "npm", "config", "get", "registry"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
    } else {
        std::process::Command::new("npm")
            .args(["config", "get", "registry"])
            .output()
    };

    let output = output.map_err(|e| format!("failed to run npm config: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

// ── 内部工具 ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ToolVersion {
    pub path: String,
    pub version: String,
}

fn detect_tool(cmd: &str, args: &[&str]) -> Option<ToolVersion> {
    let output = if cfg!(target_os = "windows") {
        let mut full_args = vec!["/C", cmd];
        full_args.extend_from_slice(args);
        std::process::Command::new("cmd")
            .args(&full_args)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
            .ok()?
    } else {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .ok()?
    };

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout.trim().to_string();
        if version.is_empty() {
            return None;
        }
        Some(ToolVersion {
            path: cmd.to_string(),
            version,
        })
    } else {
        None
    }
}

fn detect_cli_version(cmd: &str, args: &[&str]) -> Option<String> {
    detect_tool(cmd, args).map(|t| t.version)
}

/// 匹配 v数字.数字.数字 格式
fn is_stable_tag(tag: &str) -> bool {
    let rest = tag.strip_prefix('v').unwrap_or(tag);
    let parts: Vec<&str> = rest.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}
