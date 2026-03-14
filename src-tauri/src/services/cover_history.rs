use crate::models::{
    Agent, AgentType, CoverSkillResponse, CoverSkippedTarget, CoverTargetAction, CoverTargetResult,
    RollbackSkillCoverResponse, Skill, SkillCoverHistoryEntry,
};
use crate::services::{agent_detector, scanner, symlink, tree_hash};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const APP_DATA_DIR: &str = "skills-local-manager";
const COVER_INDEX_FILE_NAME: &str = "cover-history-index.json";
const COVER_BACKUP_DIR_NAME: &str = "cover-history-backups";
const MAX_HISTORY_PER_SKILL_TARGET: usize = 10;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CoverHistoryRecord {
    entry_id: String,
    operation_id: String,
    skill_slug: String,
    target_agent_type: AgentType,
    target_uid: String,
    target_path: String,
    source_uid: String,
    source_namespace: String,
    source_version_label: String,
    source_hash: String,
    previous_hash: Option<String>,
    backup_path: Option<String>,
    had_original: bool,
    applied_at: u64,
    rolled_back_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CoverHistoryIndex {
    version: u32,
    entries: Vec<CoverHistoryRecord>,
}

impl Default for CoverHistoryIndex {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

enum CoverAttemptOutcome {
    Applied(CoverTargetResult, CoverHistoryRecord),
    Skipped(String),
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn app_data_root() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .ok_or_else(|| "Could not determine app data directory".to_string())?;
    let root = base.join(APP_DATA_DIR);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

fn index_path() -> Result<PathBuf, String> {
    Ok(app_data_root()?.join(COVER_INDEX_FILE_NAME))
}

fn backups_root() -> Result<PathBuf, String> {
    let root = app_data_root()?.join(COVER_BACKUP_DIR_NAME);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

fn load_index() -> Result<CoverHistoryIndex, String> {
    let path = index_path()?;
    if !path.exists() {
        return Ok(CoverHistoryIndex::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str::<CoverHistoryIndex>(&content).map_err(|e| e.to_string())
}

fn save_index(index: &CoverHistoryIndex) -> Result<(), String> {
    let path = index_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| "Invalid cover history index path".to_string())?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp_path = path.with_extension("tmp");
    let payload = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
    fs::write(&tmp_path, payload).map_err(|e| e.to_string())?;
    fs::rename(&tmp_path, &path).map_err(|e| e.to_string())
}

fn source_version_label(skill: &Skill) -> String {
    skill
        .metadata
        .version
        .as_ref()
        .map(|x| x.trim())
        .filter(|x| !x.is_empty())
        .map(|x| x.to_string())
        .unwrap_or_else(|| skill.tree_hash.chars().take(8).collect())
}

fn source_agent_type_from_namespace(namespace: &str) -> Option<AgentType> {
    namespace
        .strip_prefix("agent:")
        .and_then(AgentType::from_id)
}

fn target_uid(agent_type: AgentType, slug: &str) -> String {
    format!("agent:{}:{}", agent_type.id(), slug)
}

fn is_agent_available(agent: &Agent) -> bool {
    agent.is_installed || agent.skills_directory_exists || agent.skill_count > 0
}

fn is_noise_file(name: &str) -> bool {
    name.eq_ignore_ascii_case(".ds_store")
        || name.eq_ignore_ascii_case("thumbs.db")
        || name.eq_ignore_ascii_case("desktop.ini")
}

fn is_noise_dir(name: &str) -> bool {
    name.eq_ignore_ascii_case(".git")
        || name.eq_ignore_ascii_case(".skilldeck-sync-snapshots")
        || name.eq_ignore_ascii_case(".skilldeck-cover-backups")
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Err(format!(
            "Source directory does not exist: {}",
            src.display()
        ));
    }
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    let entries = fs::read_dir(src).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let target_path = dst.join(&name);
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        if metadata.is_dir() {
            if is_noise_dir(&name) {
                continue;
            }
            copy_dir_recursive(&path, &target_path)?;
        } else if metadata.is_file() {
            if is_noise_file(&name) {
                continue;
            }
            fs::copy(&path, &target_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn replace_directory_atomic(source: &Path, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("Invalid target path: {}", target.display()))?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;

    let staging = parent.join(format!(".skilldeck-cover-staging-{}", Uuid::new_v4()));
    if staging.exists() {
        symlink::remove_link(&staging)?;
    }
    copy_dir_recursive(source, &staging)?;

    if target.exists() {
        symlink::remove_link(target)?;
    }

    match fs::rename(&staging, target) {
        Ok(_) => Ok(()),
        Err(_) => {
            copy_dir_recursive(&staging, target)?;
            symlink::remove_link(&staging)
        }
    }
}

fn resolve_existing_directory(path: &Path) -> PathBuf {
    if symlink::is_link(path) {
        symlink::resolve_link(path)
    } else {
        path.to_path_buf()
    }
}

fn compute_install_hash(path: &Path) -> Result<String, String> {
    tree_hash::compute_tree_hash(&resolve_existing_directory(path))
}

fn prune_history(index: &mut CoverHistoryIndex) -> Vec<PathBuf> {
    let mut grouped: HashMap<(String, AgentType), Vec<&CoverHistoryRecord>> = HashMap::new();
    for entry in &index.entries {
        grouped
            .entry((entry.skill_slug.clone(), entry.target_agent_type))
            .or_default()
            .push(entry);
    }

    let mut keep_ids: HashSet<String> = HashSet::new();
    for entries in grouped.values_mut() {
        entries.sort_by(|a, b| b.applied_at.cmp(&a.applied_at));
        for entry in entries.iter().take(MAX_HISTORY_PER_SKILL_TARGET) {
            keep_ids.insert(entry.entry_id.clone());
        }
    }

    let mut removed_backup_paths = Vec::new();
    index.entries.retain(|entry| {
        let keep = keep_ids.contains(&entry.entry_id);
        if !keep {
            if let Some(backup) = &entry.backup_path {
                removed_backup_paths.push(PathBuf::from(backup));
            }
        }
        keep
    });

    removed_backup_paths
}

fn apply_cover_to_target(
    source_skill: &Skill,
    target_agent_type: AgentType,
    target_path: &Path,
    target_uid_value: String,
    operation_id: &str,
    backup_root: &Path,
    applied_at: u64,
) -> Result<CoverAttemptOutcome, String> {
    let had_original = target_path.exists();
    let previous_hash = if had_original {
        Some(compute_install_hash(target_path)?)
    } else {
        None
    };

    if previous_hash.as_deref() == Some(source_skill.tree_hash.as_str()) {
        return Ok(CoverAttemptOutcome::Skipped(
            "Target already on source version".to_string(),
        ));
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let entry_id = Uuid::new_v4().to_string();
    let backup_path = if had_original {
        let path = backup_root.join(operation_id).join(&entry_id);
        let source_existing = resolve_existing_directory(target_path);
        copy_dir_recursive(&source_existing, &path)?;
        Some(path)
    } else {
        None
    };

    replace_directory_atomic(&source_skill.canonical_path, target_path)?;
    let new_hash = tree_hash::compute_tree_hash(target_path)?;
    let source_version = source_version_label(source_skill);

    let result = CoverTargetResult {
        target_agent_type,
        target_uid: target_uid_value.clone(),
        target_path: target_path.to_string_lossy().to_string(),
        action: if had_original {
            CoverTargetAction::Updated
        } else {
            CoverTargetAction::Installed
        },
        previous_hash: previous_hash.clone(),
        new_hash: new_hash.clone(),
        history_entry_id: entry_id.clone(),
    };

    let record = CoverHistoryRecord {
        entry_id,
        operation_id: operation_id.to_string(),
        skill_slug: source_skill.slug.clone(),
        target_agent_type,
        target_uid: target_uid_value,
        target_path: target_path.to_string_lossy().to_string(),
        source_uid: source_skill.uid.clone(),
        source_namespace: source_skill.namespace.clone(),
        source_version_label: source_version,
        source_hash: source_skill.tree_hash.clone(),
        previous_hash,
        backup_path: backup_path.map(|p| p.to_string_lossy().to_string()),
        had_original,
        applied_at,
        rolled_back_at: None,
    };

    Ok(CoverAttemptOutcome::Applied(result, record))
}

fn record_uninstall_history_for_target_path(
    skill: &Skill,
    target_agent_type: AgentType,
    target_path: &Path,
    index: &mut CoverHistoryIndex,
    backup_root: &Path,
) -> Result<Option<String>, String> {
    if !target_path.exists() {
        return Ok(None);
    }

    let operation_id = Uuid::new_v4().to_string();
    let applied_at = now_epoch_seconds();
    let entry_id = Uuid::new_v4().to_string();
    let current_hash = compute_install_hash(target_path)?;
    let backup_path = backup_root.join(&operation_id).join(&entry_id);
    let source_existing = resolve_existing_directory(target_path);
    copy_dir_recursive(&source_existing, &backup_path)?;

    let label_base = source_version_label(skill);
    let source_version = format!("removed {}", label_base);
    let target_uid_value = target_uid(target_agent_type, &skill.slug);

    index.entries.push(CoverHistoryRecord {
        entry_id: entry_id.clone(),
        operation_id,
        skill_slug: skill.slug.clone(),
        target_agent_type,
        target_uid: target_uid_value,
        target_path: target_path.to_string_lossy().to_string(),
        source_uid: skill.uid.clone(),
        source_namespace: skill.namespace.clone(),
        source_version_label: source_version,
        source_hash: current_hash.clone(),
        previous_hash: Some(current_hash),
        backup_path: Some(backup_path.to_string_lossy().to_string()),
        had_original: true,
        applied_at,
        rolled_back_at: None,
    });

    let removed_backups = prune_history(index);
    for path in removed_backups {
        if path.exists() {
            let _ = symlink::remove_link(&path);
        }
    }
    Ok(Some(entry_id))
}

fn cover_to_targets(
    source_uid: String,
    target_agents: Vec<AgentType>,
    skip_same_namespace: bool,
    continue_on_error: bool,
) -> Result<CoverSkillResponse, String> {
    let skills = scanner::scan_all_v2()?;
    let source_skill = skills
        .iter()
        .find(|s| s.uid == source_uid)
        .ok_or_else(|| format!("Skill uid not found: {}", source_uid))?;

    let source_hash = source_skill.tree_hash.clone();
    let source_version = source_version_label(source_skill);
    let source_agent_type = source_agent_type_from_namespace(&source_skill.namespace);
    let operation_id = Uuid::new_v4().to_string();
    let applied_at = now_epoch_seconds();

    let mut index = load_index()?;
    let backup_root = backups_root()?;
    let mut results = Vec::new();
    let mut skipped = Vec::new();
    let mut touched_index = false;

    let mut seen = HashSet::new();
    for target_agent_type in target_agents {
        if !seen.insert(target_agent_type) {
            continue;
        }

        if skip_same_namespace && source_agent_type == Some(target_agent_type) {
            skipped.push(CoverSkippedTarget {
                target_agent_type,
                reason: "Skipped source namespace target".to_string(),
            });
            continue;
        }

        let target_skills_dir = match target_agent_type.skills_dir() {
            Some(path) => path,
            None => {
                if continue_on_error {
                    skipped.push(CoverSkippedTarget {
                        target_agent_type,
                        reason: "Could not determine target skills directory".to_string(),
                    });
                    continue;
                }
                return Err("Could not determine target skills directory".to_string());
            }
        };
        let target_path = target_skills_dir.join(&source_skill.slug);
        let target_uid_value = target_uid(target_agent_type, &source_skill.slug);

        match apply_cover_to_target(
            source_skill,
            target_agent_type,
            &target_path,
            target_uid_value,
            &operation_id,
            &backup_root,
            applied_at,
        ) {
            Ok(CoverAttemptOutcome::Applied(result, record)) => {
                results.push(result);
                index.entries.push(record);
                touched_index = true;
            }
            Ok(CoverAttemptOutcome::Skipped(reason)) => {
                skipped.push(CoverSkippedTarget {
                    target_agent_type,
                    reason,
                });
            }
            Err(err) => {
                if continue_on_error {
                    skipped.push(CoverSkippedTarget {
                        target_agent_type,
                        reason: format!("Failed: {}", err),
                    });
                } else {
                    return Err(err);
                }
            }
        }
    }

    if touched_index {
        let removed_backups = prune_history(&mut index);
        for backup_path in removed_backups {
            if backup_path.exists() {
                let _ = symlink::remove_link(&backup_path);
            }
        }
        save_index(&index)?;
    }

    Ok(CoverSkillResponse {
        operation_id,
        source_uid,
        source_hash,
        source_version_label: source_version,
        results,
        skipped,
    })
}

pub fn cover_skill_to_agent(
    source_uid: String,
    target_agent_type: AgentType,
) -> Result<CoverSkillResponse, String> {
    cover_to_targets(source_uid, vec![target_agent_type], true, false)
}

pub fn cover_skill_to_all_available_agents(
    source_uid: String,
) -> Result<CoverSkillResponse, String> {
    let agents = agent_detector::detect_all();
    let mut targets = Vec::new();
    let mut unavailable_skipped = Vec::new();

    for agent in agents {
        if is_agent_available(&agent) {
            targets.push(agent.agent_type);
        } else {
            unavailable_skipped.push(CoverSkippedTarget {
                target_agent_type: agent.agent_type,
                reason: "Agent unavailable: missing CLI and skills directory".to_string(),
            });
        }
    }

    let mut response = cover_to_targets(source_uid, targets, true, true)?;
    response.skipped.extend(unavailable_skipped);
    Ok(response)
}

pub fn list_skill_cover_history(
    skill_slug: String,
    target_agent_type: AgentType,
) -> Result<Vec<SkillCoverHistoryEntry>, String> {
    let index = load_index()?;
    let mut items: Vec<SkillCoverHistoryEntry> = index
        .entries
        .iter()
        .filter(|entry| {
            entry.skill_slug == skill_slug && entry.target_agent_type == target_agent_type
        })
        .map(|entry| SkillCoverHistoryEntry {
            entry_id: entry.entry_id.clone(),
            skill_slug: entry.skill_slug.clone(),
            target_agent_type: entry.target_agent_type,
            target_uid: entry.target_uid.clone(),
            source_uid: entry.source_uid.clone(),
            source_namespace: entry.source_namespace.clone(),
            source_version_label: entry.source_version_label.clone(),
            source_hash: entry.source_hash.clone(),
            previous_hash: entry.previous_hash.clone(),
            applied_at: entry.applied_at,
            rolled_back_at: entry.rolled_back_at,
        })
        .collect();
    items.sort_by(|a, b| b.applied_at.cmp(&a.applied_at));
    Ok(items)
}

pub fn record_uninstall_history(
    skill: &Skill,
    target_agent_type: AgentType,
) -> Result<Option<String>, String> {
    let target_skills_dir = target_agent_type
        .skills_dir()
        .ok_or_else(|| "Could not determine target skills directory".to_string())?;
    let target_path = target_skills_dir.join(&skill.slug);
    let mut index = load_index()?;
    let backup_root = backups_root()?;
    let entry_id = record_uninstall_history_for_target_path(
        skill,
        target_agent_type,
        &target_path,
        &mut index,
        &backup_root,
    )?;
    if entry_id.is_some() {
        save_index(&index)?;
    }
    Ok(entry_id)
}

pub fn rollback_skill_cover_entry(entry_id: String) -> Result<RollbackSkillCoverResponse, String> {
    let mut index = load_index()?;
    let pos = index
        .entries
        .iter()
        .position(|entry| entry.entry_id == entry_id)
        .ok_or_else(|| format!("Cover history entry not found: {}", entry_id))?;

    if index.entries[pos].rolled_back_at.is_some() {
        let entry = &index.entries[pos];
        return Ok(RollbackSkillCoverResponse {
            entry_id: entry.entry_id.clone(),
            target_agent_type: entry.target_agent_type,
            target_uid: entry.target_uid.clone(),
            restored_hash: None,
            restored: false,
        });
    }

    let target_path = PathBuf::from(&index.entries[pos].target_path);
    if target_path.exists() {
        symlink::remove_link(&target_path)?;
    }

    let restored_hash = if index.entries[pos].had_original {
        let backup_path = index.entries[pos]
            .backup_path
            .as_ref()
            .ok_or_else(|| "Missing backup path for rollback".to_string())?;
        let backup = PathBuf::from(backup_path);
        if !backup.exists() {
            return Err(format!("Rollback backup missing: {}", backup.display()));
        }
        copy_dir_recursive(&backup, &target_path)?;
        Some(tree_hash::compute_tree_hash(&target_path)?)
    } else {
        None
    };

    index.entries[pos].rolled_back_at = Some(now_epoch_seconds());
    let response = RollbackSkillCoverResponse {
        entry_id: index.entries[pos].entry_id.clone(),
        target_agent_type: index.entries[pos].target_agent_type,
        target_uid: index.entries[pos].target_uid.clone(),
        restored_hash,
        restored: true,
    };
    save_index(&index)?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{SkillConflictState, SkillMetadata, SkillOriginType, SkillScope};
    use tempfile::tempdir;

    fn write_skill(path: &Path, marker: &str) -> Result<(), String> {
        fs::create_dir_all(path).map_err(|e| e.to_string())?;
        fs::write(path.join("SKILL.md"), format!("# {}\n", marker)).map_err(|e| e.to_string())?;
        fs::write(path.join("payload.txt"), marker.as_bytes()).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn build_skill(uid: &str, slug: &str, namespace: &str, path: &Path, version: &str) -> Skill {
        Skill {
            id: uid.to_string(),
            uid: uid.to_string(),
            slug: slug.to_string(),
            namespace: namespace.to_string(),
            canonical_path: path.to_path_buf(),
            metadata: SkillMetadata {
                name: slug.to_string(),
                description: String::new(),
                version: Some(version.to_string()),
                author: None,
                homepage: None,
                repository: None,
            },
            markdown_body: String::new(),
            scope: SkillScope::AgentLocal,
            tree_hash: tree_hash::compute_tree_hash(path).expect("source hash"),
            conflict_state: SkillConflictState::None,
            sync_group_id: None,
            installations: Vec::new(),
            lock_entry: None,
            origin_type: SkillOriginType::LocalManual,
            origin_label: "Local".to_string(),
            origin_slug: None,
            managed_source: None,
            modified_at: None,
            has_update: false,
            remote_version_label: None,
            remote_tree_hash: None,
            remote_commit_hash: None,
            local_commit_hash: None,
        }
    }

    #[test]
    fn cover_existing_target_updates_and_records_history() {
        let temp = tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let backups = temp.path().join("backups");
        write_skill(&source, "source-v1").expect("source");
        write_skill(&target, "target-v0").expect("target");

        let source_skill = build_skill("agent:codex:demo", "demo", "agent:codex", &source, "1.0.0");
        let previous_hash = tree_hash::compute_tree_hash(&target).expect("previous hash");

        let outcome = apply_cover_to_target(
            &source_skill,
            AgentType::ClaudeCode,
            &target,
            "agent:claude-code:demo".to_string(),
            "op-1",
            &backups,
            100,
        )
        .expect("cover");

        match outcome {
            CoverAttemptOutcome::Applied(result, record) => {
                assert_eq!(result.action, CoverTargetAction::Updated);
                assert_eq!(result.previous_hash, Some(previous_hash));
                assert_eq!(result.new_hash, source_skill.tree_hash);
                assert!(record.backup_path.is_some());
                assert!(PathBuf::from(record.backup_path.expect("backup path")).exists());
            }
            CoverAttemptOutcome::Skipped(reason) => panic!("unexpected skip: {}", reason),
        }
    }

    #[test]
    fn cover_missing_target_installs_with_no_previous_hash() {
        let temp = tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let target = temp.path().join("missing-target");
        let backups = temp.path().join("backups");
        write_skill(&source, "source-v1").expect("source");
        let source_skill = build_skill("agent:codex:demo", "demo", "agent:codex", &source, "1.0.0");

        let outcome = apply_cover_to_target(
            &source_skill,
            AgentType::ClaudeCode,
            &target,
            "agent:claude-code:demo".to_string(),
            "op-2",
            &backups,
            101,
        )
        .expect("cover");

        match outcome {
            CoverAttemptOutcome::Applied(result, record) => {
                assert_eq!(result.action, CoverTargetAction::Installed);
                assert_eq!(result.previous_hash, None);
                assert_eq!(result.new_hash, source_skill.tree_hash);
                assert!(!record.had_original);
                assert!(record.backup_path.is_none());
                assert!(target.exists());
            }
            CoverAttemptOutcome::Skipped(reason) => panic!("unexpected skip: {}", reason),
        }
    }

    #[test]
    fn batch_target_selection_includes_cli_or_skills_agents() {
        let agents = vec![
            Agent {
                agent_type: AgentType::Codex,
                is_installed: true,
                config_directory: None,
                skills_directory: None,
                readable_skills_directories: Vec::new(),
                config_directory_exists: false,
                skills_directory_exists: false,
                skill_count: 0,
            },
            Agent {
                agent_type: AgentType::ClaudeCode,
                is_installed: false,
                config_directory: None,
                skills_directory: None,
                readable_skills_directories: Vec::new(),
                config_directory_exists: false,
                skills_directory_exists: true,
                skill_count: 0,
            },
            Agent {
                agent_type: AgentType::Trae,
                is_installed: false,
                config_directory: None,
                skills_directory: None,
                readable_skills_directories: Vec::new(),
                config_directory_exists: true,
                skills_directory_exists: false,
                skill_count: 0,
            },
        ];

        let selected: Vec<AgentType> = agents
            .iter()
            .filter(|agent| is_agent_available(agent))
            .map(|agent| agent.agent_type)
            .collect();
        assert_eq!(selected, vec![AgentType::Codex, AgentType::ClaudeCode]);
    }

    #[test]
    fn rollback_restores_original_content() {
        let temp = tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let backups = temp.path().join("backups");
        write_skill(&source, "source-v1").expect("source");
        write_skill(&target, "target-v0").expect("target");
        let source_skill = build_skill("agent:codex:demo", "demo", "agent:codex", &source, "1.0.0");
        let original_hash = tree_hash::compute_tree_hash(&target).expect("original");

        let outcome = apply_cover_to_target(
            &source_skill,
            AgentType::ClaudeCode,
            &target,
            "agent:claude-code:demo".to_string(),
            "op-3",
            &backups,
            102,
        )
        .expect("cover");
        let record = match outcome {
            CoverAttemptOutcome::Applied(_, record) => record,
            CoverAttemptOutcome::Skipped(reason) => panic!("unexpected skip: {}", reason),
        };
        symlink::remove_link(&target).expect("clean target");
        let backup_path = PathBuf::from(record.backup_path.expect("backup"));
        copy_dir_recursive(&backup_path, &target).expect("restore");
        let restored_hash = tree_hash::compute_tree_hash(&target).expect("restored");
        assert_eq!(restored_hash, original_hash);
    }

    #[test]
    fn rollback_can_target_non_latest_history_entry() {
        let temp = tempdir().expect("tempdir");
        let source_v1 = temp.path().join("source-v1");
        let source_v2 = temp.path().join("source-v2");
        let target = temp.path().join("target");
        let backups = temp.path().join("backups");
        write_skill(&source_v1, "source-v1").expect("v1");
        write_skill(&source_v2, "source-v2").expect("v2");
        write_skill(&target, "target-v0").expect("target");
        let original_hash = tree_hash::compute_tree_hash(&target).expect("orig");

        let source_skill_v1 = build_skill(
            "agent:codex:demo",
            "demo",
            "agent:codex",
            &source_v1,
            "1.0.0",
        );
        let source_skill_v2 = build_skill(
            "agent:codex:demo",
            "demo",
            "agent:codex",
            &source_v2,
            "2.0.0",
        );

        let first = apply_cover_to_target(
            &source_skill_v1,
            AgentType::ClaudeCode,
            &target,
            "agent:claude-code:demo".to_string(),
            "op-4",
            &backups,
            103,
        )
        .expect("first");
        let first_record = match first {
            CoverAttemptOutcome::Applied(_, record) => record,
            CoverAttemptOutcome::Skipped(reason) => panic!("unexpected skip: {}", reason),
        };

        let second = apply_cover_to_target(
            &source_skill_v2,
            AgentType::ClaudeCode,
            &target,
            "agent:claude-code:demo".to_string(),
            "op-5",
            &backups,
            104,
        )
        .expect("second");
        match second {
            CoverAttemptOutcome::Applied(_, _) => {}
            CoverAttemptOutcome::Skipped(reason) => panic!("unexpected skip: {}", reason),
        }

        symlink::remove_link(&target).expect("remove target");
        let first_backup = PathBuf::from(first_record.backup_path.expect("first backup"));
        copy_dir_recursive(&first_backup, &target).expect("restore from first backup");
        let restored_hash = tree_hash::compute_tree_hash(&target).expect("hash");
        assert_eq!(restored_hash, original_hash);
    }

    #[test]
    fn uninstall_history_records_deleted_version() {
        let temp = tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let backups = temp.path().join("backups");
        write_skill(&source, "source-v1").expect("source");
        write_skill(&target, "target-v0").expect("target");
        let skill = build_skill("agent:codex:demo", "demo", "agent:codex", &source, "1.0.0");

        let mut index = CoverHistoryIndex::default();
        let entry_id = record_uninstall_history_for_target_path(
            &skill,
            AgentType::Codex,
            &target,
            &mut index,
            &backups,
        )
        .expect("record uninstall")
        .expect("entry id");

        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].entry_id, entry_id);
        assert_eq!(index.entries[0].target_agent_type, AgentType::Codex);
        assert!(index.entries[0]
            .source_version_label
            .starts_with("removed "));
        let backup_path = PathBuf::from(index.entries[0].backup_path.clone().expect("backup path"));
        assert!(backup_path.exists());
    }

    #[test]
    fn history_prunes_to_max_per_skill_target() {
        let temp = tempdir().expect("tempdir");
        let mut index = CoverHistoryIndex::default();
        let mut oldest_backup: Option<PathBuf> = None;

        for i in 0..11 {
            let backup = temp.path().join(format!("backup-{}", i));
            fs::create_dir_all(&backup).expect("backup dir");
            if i == 0 {
                oldest_backup = Some(backup.clone());
            }
            index.entries.push(CoverHistoryRecord {
                entry_id: format!("entry-{}", i),
                operation_id: format!("op-{}", i),
                skill_slug: "demo".to_string(),
                target_agent_type: AgentType::Codex,
                target_uid: "agent:codex:demo".to_string(),
                target_path: "x".to_string(),
                source_uid: "agent:claude-code:demo".to_string(),
                source_namespace: "agent:claude-code".to_string(),
                source_version_label: i.to_string(),
                source_hash: i.to_string(),
                previous_hash: None,
                backup_path: Some(backup.to_string_lossy().to_string()),
                had_original: true,
                applied_at: i as u64,
                rolled_back_at: None,
            });
        }

        let removed = prune_history(&mut index);
        for path in removed {
            if path.exists() {
                symlink::remove_link(&path).expect("remove pruned backup");
            }
        }

        assert_eq!(index.entries.len(), 10);
        assert!(oldest_backup.expect("oldest").exists() == false);
    }
}
