//! Skills management — install and execute skills from GitHub or local.
//!
//! Skills are stored in the AGN data directory under `skills/`.
//! Each skill is a directory with a `skill.json` manifest.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Skill manifest (skill.json in each skill directory).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub homepage: String,
    /// The command or script to execute when the skill is invoked.
    /// Can be a shell command, a script path, or a URL.
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub script: String,
    /// Whether this skill is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// Skills manager.
pub struct SkillsManager {
    skills_dir: PathBuf,
}

impl SkillsManager {
    pub fn new(data_dir: &Path) -> Self {
        let skills_dir = data_dir.join("skills");
        if !skills_dir.exists() {
            let _ = std::fs::create_dir_all(&skills_dir);
        }
        Self { skills_dir }
    }

    /// List all installed skills.
    pub fn list(&self) -> Vec<SkillManifest> {
        let mut skills = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let manifest_path = path.join("skill.json");
                    if manifest_path.exists() {
                        if let Ok(data) = std::fs::read_to_string(&manifest_path) {
                            if let Ok(manifest) = serde_json::from_str::<SkillManifest>(&data) {
                                skills.push(manifest);
                            }
                        }
                    }
                }
            }
        }
        skills
    }

    /// Get enabled skills for prompt injection.
    pub fn enabled_skills(&self) -> Vec<crate::prompt::SkillInfo> {
        self.list()
            .into_iter()
            .filter(|s| s.enabled)
            .map(|s| crate::prompt::SkillInfo {
                name: s.name,
                description: s.description,
            })
            .collect()
    }

    /// Install a skill from a GitHub URL.
    /// URL format: https://github.com/user/repo or https://github.com/user/repo/tree/branch/path
    pub async fn install_from_github(&self, url: &str) -> anyhow::Result<String> {
        info!("Installing skill from: {}", url);

        // Parse GitHub URL
        let (owner, repo, subpath, branch) = parse_github_url(url)?;

        // Download the repository as a tarball
        let tarball_url = match &branch {
            Some(b) => format!("https://github.com/{}/{}/archive/refs/heads/{}.tar.gz", owner, repo, b),
            None => format!("https://github.com/{}/{}/archive/refs/heads/main.tar.gz", owner, repo),
        };

        let resp = reqwest::get(&tarball_url).await?;
        if !resp.status().is_success() {
            // Try master branch
            let tarball_url = format!("https://github.com/{}/{}/archive/refs/heads/master.tar.gz", owner, repo);
            let resp = reqwest::get(&tarball_url).await?;
            if !resp.status().is_success() {
                anyhow::bail!("Failed to download repository");
            }
        }

        let bytes = resp.bytes().await?;
        let temp_dir = std::env::temp_dir().join(format!("agn-skill-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        // Extract tarball
        let tarball_path = temp_dir.join("repo.tar.gz");
        std::fs::write(&tarball_path, &bytes)?;

        let output = std::process::Command::new("tar")
            .arg("xzf")
            .arg(&tarball_path)
            .arg("-C")
            .arg(&temp_dir)
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to extract tarball");
        }

        // Find the extracted directory
        let extracted: PathBuf = std::fs::read_dir(&temp_dir)?
            .flatten()
            .find(|e| e.path().is_dir())
            .map(|e| e.path())
            .ok_or_else(|| anyhow::anyhow!("No directory found in tarball"))?;

        // Look for skill.json in the extracted dir (or subpath)
        let skill_source = match &subpath {
            Some(sp) => extracted.join(sp),
            None => extracted,
        };

        let manifest_path = skill_source.join("skill.json");
        if !manifest_path.exists() {
            anyhow::bail!("No skill.json found in repository");
        }

        let manifest_data = std::fs::read_to_string(&manifest_path)?;
        let manifest: SkillManifest = serde_json::from_str(&manifest_data)?;
        let skill_name = manifest.name.clone();

        // Copy to skills directory
        let dest = self.skills_dir.join(&skill_name);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
        copy_dir(&skill_source, &dest)?;

        info!("Skill '{}' installed to {}", skill_name, dest.display());
        Ok(skill_name)
    }

    /// Install a skill from a local directory.
    pub fn install_from_local(&self, path: &Path) -> anyhow::Result<String> {
        let manifest_path = path.join("skill.json");
        if !manifest_path.exists() {
            anyhow::bail!("No skill.json found in directory");
        }

        let manifest_data = std::fs::read_to_string(&manifest_path)?;
        let manifest: SkillManifest = serde_json::from_str(&manifest_data)?;
        let skill_name = manifest.name.clone();

        let dest = self.skills_dir.join(&skill_name);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
        copy_dir(path, &dest)?;

        Ok(skill_name)
    }

    /// Remove a skill.
    pub fn remove(&self, name: &str) -> anyhow::Result<()> {
        let dir = self.skills_dir.join(name);
        if !dir.exists() {
            anyhow::bail!("Skill not found: {}", name);
        }
        std::fs::remove_dir_all(&dir)?;
        Ok(())
    }

    /// Toggle a skill's enabled state.
    pub fn toggle(&self, name: &str, enabled: bool) -> anyhow::Result<()> {
        let dir = self.skills_dir.join(name);
        let manifest_path = dir.join("skill.json");
        if !manifest_path.exists() {
            anyhow::bail!("Skill not found: {}", name);
        }
        let data = std::fs::read_to_string(&manifest_path)?;
        let mut manifest: SkillManifest = serde_json::from_str(&data)?;
        manifest.enabled = enabled;
        let data = serde_json::to_string_pretty(&manifest)?;
        std::fs::write(&manifest_path, data)?;
        Ok(())
    }
}

/// Parse a GitHub URL into (owner, repo, subpath, branch).
fn parse_github_url(url: &str) -> anyhow::Result<(String, String, Option<String>, Option<String>)> {
    // https://github.com/user/repo
    // https://github.com/user/repo/tree/branch/sub/path
    let url = url.trim_end_matches('/');
    let url = url.strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .ok_or_else(|| anyhow::anyhow!("Not a GitHub URL"))?;

    let parts: Vec<&str> = url.split('/').collect();
    if parts.len() < 2 {
        anyhow::bail!("Invalid GitHub URL");
    }

    let owner = parts[0].to_string();
    let repo = parts[1].to_string();

    let (subpath, branch) = if parts.len() >= 4 && parts[2] == "tree" {
        let b = parts[3].to_string();
        let sp = if parts.len() > 4 {
            Some(parts[4..].join("/"))
        } else {
            None
        };
        (sp, Some(b))
    } else {
        (None, None)
    };

    Ok((owner, repo, subpath, branch))
}

/// Recursively copy a directory.
fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}