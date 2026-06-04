use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

fn qwen_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".qwen")
}

// ════════════════════════════════════════════════════════════
// skills.sh 搜索
// ════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSearchResult {
    pub key: String,
    pub name: String,
    pub description: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub repo_branch: String,
    pub directory: String,
    pub installs: u64,
    pub readme_url: Option<String>,
    pub installed: bool,
}

/// 搜索 skills.sh 公共目录
pub async fn search_skills_sh(query: &str, limit: usize, offset: usize) -> Result<Vec<SkillSearchResult>, String> {
    let url = format!(
        "https://skills.sh/api/search?q={}&limit={}&offset={}",
        urlencoding::encode(query), limit, offset
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;

    let skills = body.get("skills").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let installed = load_installed_names();

    let mut results = Vec::new();
    for skill in skills {
        let source = skill.get("source").and_then(|v| v.as_str()).unwrap_or("");
        // 过滤非 GitHub 来源
        if source.contains('.') { continue; }

        let parts: Vec<&str> = source.split('/').collect();
        if parts.len() < 2 { continue; }
        let owner = parts[0].to_string();
        let repo = parts[1].to_string();

        let name = skill.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let directory = name.clone();

        results.push(SkillSearchResult {
            key: format!("{}/{}:{}", owner, repo, directory),
            name: name.clone(),
            description: skill.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            repo_owner: owner,
            repo_name: repo,
            repo_branch: "main".into(),
            directory,
            installs: skill.get("installs").and_then(|v| v.as_u64()).unwrap_or(0),
            readme_url: Some(format!("https://github.com/{}/{}", source.split('/').nth(0).unwrap_or(""), source.split('/').nth(1).unwrap_or(""))),
            installed: installed.contains(&name),
        });
    }

    Ok(results)
}

fn load_installed_names() -> std::collections::HashSet<String> {
    let skills_dir = qwen_home().join("skills");
    let mut set = std::collections::HashSet::new();
    if let Ok(entries) = fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                set.insert(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    set
}

// ════════════════════════════════════════════════════════════
// 从 GitHub 安装技能
// ════════════════════════════════════════════════════════════

/// 从 GitHub 仓库下载并安装技能到 ~/.qwen/skills/<name>/
pub async fn install_skill_from_repo(
    owner: &str, repo: &str, branch: &str, directory: &str,
) -> Result<String, String> {
    let skills_dir = qwen_home().join("skills");
    fs::create_dir_all(&skills_dir).map_err(|e| e.to_string())?;

    // 下载 ZIP
    let url = format!(
        "https://github.com/{}/{}/archive/refs/heads/{}.zip",
        owner, repo, branch
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;

    // 解压到临时目录
    let tmp_dir = std::env::temp_dir().join(format!("agentbox-skill-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

    let zip_path = tmp_dir.join("repo.zip");
    fs::write(&zip_path, &bytes).map_err(|e| e.to_string())?;

    // 解压
    extract_zip(&zip_path, &tmp_dir).map_err(|e| e.to_string())?;

    // ZIP 解压后目录名为 {repo}-{branch}
    let extracted_dir_name = format!("{}-{}", repo, branch);
    let extracted_dir = tmp_dir.join(&extracted_dir_name);

    // 找到技能目录
    let skill_src = if directory.is_empty() || directory == repo {
        extracted_dir.clone()
    } else {
        extracted_dir.join(directory)
    };

    if !skill_src.join("SKILL.md").exists() {
        // 尝试在根目录找
        if !extracted_dir.join("SKILL.md").exists() {
            let _ = fs::remove_dir_all(&tmp_dir);
            return Err("未找到 SKILL.md 文件".into());
        }
    }

    // 复制到目标目录
    let target_name = if directory.is_empty() { repo } else { directory };
    let target_dir = skills_dir.join(target_name);

    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).map_err(|e| e.to_string())?;
    }

    copy_dir_recursive(&skill_src, &target_dir).map_err(|e| e.to_string())?;

    // 清理临时目录
    let _ = fs::remove_dir_all(&tmp_dir);

    Ok(target_dir.to_string_lossy().to_string())
}

/// 卸载技能（删除目录）
pub fn uninstall_skill(name: &str) -> Result<(), String> {
    let target = qwen_home().join("skills").join(name);
    if !target.exists() {
        return Err("技能不存在".into());
    }
    fs::remove_dir_all(&target).map_err(|e| e.to_string())
}

// ════════════════════════════════════════════════════════════
// 工具函数
// ════════════════════════════════════════════════════════════

fn extract_zip(zip_path: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let outpath = dest.join(file.mangled_name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut outfile = fs::File::create(&outpath).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
