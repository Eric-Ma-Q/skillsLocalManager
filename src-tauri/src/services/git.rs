use git2::{build::RepoBuilder, Repository};
use std::path::{Path, PathBuf};

use crate::models::SkillMetadata;
use crate::services::md_parser;
use std::fs;

pub struct DiscoveredSkill {
    pub id: String,
    pub folder_path: String,
    pub skill_md_path: String,
    pub metadata: SkillMetadata,
    pub markdown_body: String,
}

pub fn clone_repo(url: &str, shallow: bool) -> Result<PathBuf, String> {
    let temp_dir = tempfile::Builder::new()
        .prefix("skillsLocalManager-")
        .tempdir()
        .map_err(|e| e.to_string())?;

    let path = temp_dir.keep();

    let mut fo = git2::FetchOptions::new();
    if shallow {
        fo.depth(1);
    }

    let mut builder = RepoBuilder::new();
    builder.fetch_options(fo);

    builder.clone(url, &path).map_err(|e| e.to_string())?;

    Ok(path)
}

pub fn get_tree_hash(repo_path: &Path, rel_path: &str) -> Result<String, String> {
    let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;
    let obj = repo
        .revparse_single(&format!("HEAD:{}", rel_path))
        .map_err(|e| e.to_string())?;
    Ok(obj.id().to_string())
}

pub fn get_commit_hash(repo_path: &Path) -> Result<String, String> {
    let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;
    let head = repo.head().map_err(|e| e.to_string())?;
    let commit = head.peel_to_commit().map_err(|e| e.to_string())?;
    Ok(commit.id().to_string())
}

pub fn scan_skills_in_repo(repo_path: &Path) -> Vec<DiscoveredSkill> {
    let mut discovered = Vec::new();

    for entry in walkdir::WalkDir::new(repo_path)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .flatten()
    {
        if entry.file_name() == "SKILL.md" {
            let skill_md_path = entry.path();
            let skill_dir = skill_md_path.parent().unwrap();
            let skill_id = skill_dir.file_name().unwrap().to_str().unwrap().to_string();

            let rel_folder_path = skill_dir
                .strip_prefix(repo_path)
                .unwrap()
                .to_str()
                .unwrap_or("")
                .to_string();

            let rel_skill_md_path = skill_md_path
                .strip_prefix(repo_path)
                .unwrap()
                .to_str()
                .unwrap_or("")
                .to_string();

            if let Ok(content) = fs::read_to_string(skill_md_path) {
                if let Ok((metadata, body)) = md_parser::parse(&content) {
                    discovered.push(DiscoveredSkill {
                        id: skill_id,
                        folder_path: rel_folder_path,
                        skill_md_path: rel_skill_md_path,
                        metadata,
                        markdown_body: body,
                    });
                }
            }
        }
    }

    discovered
}

pub fn normalize_repo_url(input: &str) -> Result<(String, String), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Empty repository URL".to_string());
    }

    if trimmed.to_lowercase().starts_with("https://") {
        let mut source = trimmed.to_string();
        if let Some(pos) = source.find("github.com/") {
            source = source[pos + 11..].to_string();
        }
        if source.ends_with(".git") {
            source = source[..source.len() - 4].to_string();
        }

        let mut repo_url = trimmed.to_string();
        if !repo_url.ends_with(".git") {
            repo_url.push_str(".git");
        }
        return Ok((repo_url, source));
    }

    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        let mut repo_name = parts[1].to_string();
        if repo_name.ends_with(".git") {
            repo_name = repo_name[..repo_name.len() - 4].to_string();
        }
        let source = format!("{}/{}", parts[0], repo_name);
        let repo_url = format!("https://github.com/{}.git", source);
        return Ok((repo_url, source));
    }

    Err(format!("Invalid repository format: {}", input))
}
