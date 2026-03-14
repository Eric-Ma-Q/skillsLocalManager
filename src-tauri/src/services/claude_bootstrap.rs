use crate::models::{
    AgentType, ClaudeBootstrapCatalog, ClaudeBootstrapRequest, ClaudeBootstrapResult,
    ClaudeBootstrapSkill, ClaudeBootstrapSkippedSkill,
};
use crate::services::{git, managed_skills};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use which::which;

pub const ANTHROPIC_SKILLS_REPO_URL: &str = "https://github.com/anthropics/skills.git";
pub const ANTHROPIC_SKILLS_REPO_REF: &str = "main";

fn bootstrap_skills() -> Vec<ClaudeBootstrapSkill> {
    vec![
        ClaudeBootstrapSkill {
            slug: "mcp-builder".to_string(),
            name: "MCP Builder".to_string(),
            description: "Build high-quality MCP servers and tool integrations.".to_string(),
            recommended: true,
        },
        ClaudeBootstrapSkill {
            slug: "skill-creator".to_string(),
            name: "Skill Creator".to_string(),
            description: "Create and refine Claude skills with an evaluation loop.".to_string(),
            recommended: true,
        },
        ClaudeBootstrapSkill {
            slug: "webapp-testing".to_string(),
            name: "Webapp Testing".to_string(),
            description: "Test local web apps with Playwright workflows and debugging support."
                .to_string(),
            recommended: true,
        },
        ClaudeBootstrapSkill {
            slug: "frontend-design".to_string(),
            name: "Frontend Design".to_string(),
            description: "Design distinctive production-grade frontend interfaces.".to_string(),
            recommended: true,
        },
        ClaudeBootstrapSkill {
            slug: "claude-api".to_string(),
            name: "Claude API".to_string(),
            description: "Build applications with the Claude API and Anthropic SDKs.".to_string(),
            recommended: true,
        },
        ClaudeBootstrapSkill {
            slug: "pdf".to_string(),
            name: "PDF".to_string(),
            description: "Read, create, split, merge, and process PDF files.".to_string(),
            recommended: false,
        },
        ClaudeBootstrapSkill {
            slug: "docx".to_string(),
            name: "DOCX".to_string(),
            description: "Create, edit, and analyze Word documents.".to_string(),
            recommended: false,
        },
        ClaudeBootstrapSkill {
            slug: "pptx".to_string(),
            name: "PPTX".to_string(),
            description: "Build and manipulate PowerPoint presentations.".to_string(),
            recommended: false,
        },
        ClaudeBootstrapSkill {
            slug: "xlsx".to_string(),
            name: "XLSX".to_string(),
            description: "Work with spreadsheet files and spreadsheet automation.".to_string(),
            recommended: false,
        },
    ]
}

pub fn bootstrap_source_repo() -> &'static str {
    "anthropics/skills"
}

pub fn bootstrap_source_ref() -> &'static str {
    ANTHROPIC_SKILLS_REPO_REF
}

pub fn bootstrap_repo_url() -> &'static str {
    ANTHROPIC_SKILLS_REPO_URL
}

pub fn is_bootstrap_slug(slug: &str) -> bool {
    bootstrap_skills()
        .into_iter()
        .any(|skill| skill.slug == slug)
}

fn target_dir() -> Result<PathBuf, String> {
    AgentType::ClaudeCode
        .skills_dir()
        .ok_or_else(|| "Could not determine Claude skills directory".to_string())
}

fn path_is_creatable(path: &Path) -> bool {
    if path.exists() {
        return path.is_dir();
    }

    let mut current = path.parent();
    while let Some(candidate) = current {
        if candidate.exists() {
            return candidate.is_dir();
        }
        current = candidate.parent();
    }

    false
}

fn existing_skill_slugs_in_dir(path: &Path) -> Result<Vec<String>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut slugs = Vec::new();
    let entries = fs::read_dir(path).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let child = entry.path();
        if child.exists() {
            if let Some(name) = child.file_name().and_then(|value| value.to_str()) {
                slugs.push(name.to_string());
            }
        }
    }
    slugs.sort();
    Ok(slugs)
}

pub fn get_catalog() -> Result<ClaudeBootstrapCatalog, String> {
    let target_dir = target_dir()?;
    let skills = bootstrap_skills();
    let existing_skill_slugs = existing_skill_slugs_in_dir(&target_dir)?;

    let (recommended_skills, optional_skills): (Vec<_>, Vec<_>) =
        skills.into_iter().partition(|skill| skill.recommended);

    Ok(ClaudeBootstrapCatalog {
        target_dir: target_dir.to_string_lossy().to_string(),
        target_dir_exists: target_dir.exists(),
        can_create_target_dir: path_is_creatable(&target_dir),
        claude_cli_installed: which(AgentType::ClaudeCode.detect_command()).is_ok(),
        recommended_skills,
        optional_skills,
        existing_skill_slugs,
    })
}

fn validate_request_skills(skill_slugs: &[String]) -> Result<Vec<String>, String> {
    if skill_slugs.is_empty() {
        return Err("CLAUDE_BOOTSTRAP_NO_SKILLS_SELECTED".to_string());
    }

    let allowed: HashSet<String> = bootstrap_skills()
        .into_iter()
        .map(|skill| skill.slug)
        .collect();
    let mut deduped = Vec::new();
    let mut seen = HashSet::new();

    for slug in skill_slugs {
        let normalized = slug.trim().to_string();
        if normalized.is_empty() {
            continue;
        }
        if !allowed.contains(&normalized) {
            return Err(format!(
                "CLAUDE_BOOTSTRAP_INVALID_SKILL_SLUG: {}",
                normalized
            ));
        }
        if seen.insert(normalized.clone()) {
            deduped.push(normalized);
        }
    }

    if deduped.is_empty() {
        return Err("CLAUDE_BOOTSTRAP_NO_SKILLS_SELECTED".to_string());
    }

    Ok(deduped)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;

    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn install_skill_to_target(
    repo_root: &Path,
    target_dir: &Path,
    slug: &str,
) -> Result<Option<ClaudeBootstrapSkippedSkill>, String> {
    let source_dir = repo_root.join("skills").join(slug);
    if !source_dir.exists() || !source_dir.join("SKILL.md").exists() {
        return Err(format!("CLAUDE_BOOTSTRAP_SOURCE_SKILL_MISSING: {}", slug));
    }

    let final_target = target_dir.join(slug);
    if final_target.exists() {
        return Ok(Some(ClaudeBootstrapSkippedSkill {
            slug: slug.to_string(),
            reason: "already_exists".to_string(),
        }));
    }

    let staging = target_dir.join(format!(".skilldeck-bootstrap-{}-{}", slug, Uuid::new_v4()));
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }

    copy_dir_recursive(&source_dir, &staging)?;
    fs::rename(&staging, &final_target).map_err(|e| {
        let _ = fs::remove_dir_all(&staging);
        e.to_string()
    })?;
    let version_label = git::scan_skills_in_repo(repo_root)
        .into_iter()
        .find(|skill| skill.id == slug)
        .and_then(|skill| skill.metadata.version);
    managed_skills::write_bootstrap_metadata(&final_target, slug, version_label)?;

    Ok(None)
}

fn install_bootstrap_skills_impl(
    repo_root: &Path,
    target_dir: &Path,
    request: &ClaudeBootstrapRequest,
) -> Result<ClaudeBootstrapResult, String> {
    let skill_slugs = validate_request_skills(&request.skill_slugs)?;
    let mut created_target_dir = false;

    if !target_dir.exists() {
        if !request.create_target_dir_if_missing {
            return Err("CLAUDE_BOOTSTRAP_TARGET_DIR_MISSING".to_string());
        }
        if !path_is_creatable(target_dir) {
            return Err("CLAUDE_BOOTSTRAP_TARGET_DIR_NOT_CREATABLE".to_string());
        }
        fs::create_dir_all(target_dir)
            .map_err(|e| format!("CLAUDE_BOOTSTRAP_TARGET_DIR_CREATE_FAILED: {}", e))?;
        created_target_dir = true;
    }

    let mut installed = Vec::new();
    let mut skipped = Vec::new();

    for slug in skill_slugs {
        match install_skill_to_target(repo_root, target_dir, &slug)? {
            Some(skip) => skipped.push(skip),
            None => installed.push(slug),
        }
    }

    Ok(ClaudeBootstrapResult {
        target_dir: target_dir.to_string_lossy().to_string(),
        created_target_dir,
        installed,
        skipped,
        source_repo: "anthropics/skills".to_string(),
        source_ref: ANTHROPIC_SKILLS_REPO_REF.to_string(),
    })
}

pub fn install_skills(request: ClaudeBootstrapRequest) -> Result<ClaudeBootstrapResult, String> {
    let target_dir = target_dir()?;
    let repo_root = git::clone_repo(ANTHROPIC_SKILLS_REPO_URL, true)
        .map_err(|err| format!("CLAUDE_BOOTSTRAP_CLONE_FAILED: {}", err))?;
    let result = install_bootstrap_skills_impl(&repo_root, &target_dir, &request);
    let _ = fs::remove_dir_all(&repo_root);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_repo(root: &Path, slugs: &[&str]) {
        let skills_root = root.join("skills");
        fs::create_dir_all(&skills_root).expect("create skills root");

        for slug in slugs {
            let dir = skills_root.join(slug);
            fs::create_dir_all(&dir).expect("create skill dir");
            fs::write(
                dir.join("SKILL.md"),
                format!("---\nname: {}\ndescription: demo\n---\n", slug),
            )
            .expect("write skill");
        }
    }

    #[test]
    fn creates_target_directory_when_missing() {
        let repo = tempdir().expect("repo tempdir");
        let target_parent = tempdir().expect("target tempdir");
        let target_dir = target_parent.path().join(".claude").join("skills");
        make_repo(repo.path(), &["mcp-builder"]);

        let result = install_bootstrap_skills_impl(
            repo.path(),
            &target_dir,
            &ClaudeBootstrapRequest {
                skill_slugs: vec!["mcp-builder".to_string()],
                create_target_dir_if_missing: true,
            },
        )
        .expect("install result");

        assert!(target_dir.exists());
        assert!(target_dir.join("mcp-builder").join("SKILL.md").exists());
        assert!(result.created_target_dir);
        assert_eq!(result.installed, vec!["mcp-builder".to_string()]);
    }

    #[test]
    fn skips_existing_skill_without_overwriting() {
        let repo = tempdir().expect("repo tempdir");
        let target = tempdir().expect("target tempdir");
        let existing = target.path().join("mcp-builder");
        make_repo(repo.path(), &["mcp-builder"]);
        fs::create_dir_all(&existing).expect("existing dir");
        fs::write(existing.join("SKILL.md"), "original").expect("write original");

        let result = install_bootstrap_skills_impl(
            repo.path(),
            target.path(),
            &ClaudeBootstrapRequest {
                skill_slugs: vec!["mcp-builder".to_string()],
                create_target_dir_if_missing: false,
            },
        )
        .expect("install result");

        assert!(result.installed.is_empty());
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].reason, "already_exists");
        assert_eq!(
            fs::read_to_string(existing.join("SKILL.md")).expect("read skill"),
            "original"
        );
    }

    #[test]
    fn installs_selected_skill_from_repo_root() {
        let repo = tempdir().expect("repo tempdir");
        let target = tempdir().expect("target tempdir");
        make_repo(repo.path(), &["skill-creator"]);

        let result = install_bootstrap_skills_impl(
            repo.path(),
            target.path(),
            &ClaudeBootstrapRequest {
                skill_slugs: vec!["skill-creator".to_string()],
                create_target_dir_if_missing: false,
            },
        )
        .expect("install result");

        assert_eq!(result.installed, vec!["skill-creator".to_string()]);
        assert!(target
            .path()
            .join("skill-creator")
            .join("SKILL.md")
            .exists());
    }

    #[test]
    fn rejects_empty_skill_selection() {
        let repo = tempdir().expect("repo tempdir");
        let target = tempdir().expect("target tempdir");

        let err = install_bootstrap_skills_impl(
            repo.path(),
            target.path(),
            &ClaudeBootstrapRequest {
                skill_slugs: Vec::new(),
                create_target_dir_if_missing: false,
            },
        )
        .expect_err("should reject");

        assert_eq!(err, "CLAUDE_BOOTSTRAP_NO_SKILLS_SELECTED");
    }
}
