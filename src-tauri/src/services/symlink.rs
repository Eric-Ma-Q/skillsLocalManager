use crate::models::{AgentType, SkillInstallation};
use dunce;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::symlink as unix_symlink;

pub fn create_link(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    #[cfg(unix)]
    {
        unix_symlink(source, target).map_err(|e| e.to_string())?;
    }

    #[cfg(windows)]
    {
        junction::create(source, target).map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn remove_link(path: &Path) -> Result<(), String> {
    if path.symlink_metadata().is_err() {
        return Ok(());
    }

    #[cfg(windows)]
    {
        if junction::exists(path).unwrap_or(false) {
            junction::delete(path).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }

    if is_link(path) {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    } else if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())?;
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn is_link(path: &Path) -> bool {
    #[cfg(windows)]
    {
        if junction::exists(path).unwrap_or(false) {
            return true;
        }
    }

    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

pub fn resolve_link(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn find_installations(skill_id: &str, canonical_path: &Path) -> Vec<SkillInstallation> {
    let mut installations = Vec::new();
    let mut agents_with_direct_installation = std::collections::HashSet::new();

    // First pass: direct installations
    for agent_type in AgentType::all_cases() {
        if let Some(skills_dir) = agent_type.skills_dir() {
            let skill_path = skills_dir.join(skill_id);
            if skill_path.exists() {
                let is_sym = is_link(&skill_path);
                let resolved = if is_sym {
                    resolve_link(&skill_path)
                } else {
                    skill_path.clone()
                };

                if resolved == canonical_path {
                    installations.push(SkillInstallation {
                        agent_type,
                        path: skill_path.clone(),
                        is_symlink: is_sym,
                        is_inherited: false,
                        inherited_from: None,
                    });
                    agents_with_direct_installation.insert(agent_type);
                }
            }
        }
    }

    // Second pass: inherited installations
    for agent_type in AgentType::all_cases() {
        if agents_with_direct_installation.contains(&agent_type) {
            continue;
        }

        for (dir, source_agent) in agent_type.additional_readable_skills_directories() {
            let skill_path = dir.join(skill_id);
            if skill_path.exists() {
                let resolved = resolve_link(&skill_path);
                if resolved == canonical_path {
                    installations.push(SkillInstallation {
                        agent_type,
                        path: skill_path.clone(),
                        is_symlink: is_link(&skill_path),
                        is_inherited: true,
                        inherited_from: Some(source_agent),
                    });
                    break;
                }
            }
        }
    }

    installations
}
