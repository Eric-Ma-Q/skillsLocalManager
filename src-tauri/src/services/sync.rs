use crate::models::{
    AppliedTarget, ApplySyncResponse, RollbackSyncResponse, Skill, SyncDecisionType,
    SyncFileChange, SyncFileChangeType, SyncMappingUpsertResponse, SyncPreviewResponse,
    SyncTargetPreview,
};
use crate::services::{scanner, symlink, tree_hash};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const APP_DATA_DIR: &str = "skills-local-manager";
const INDEX_FILE_NAME: &str = "sync-index.json";
const SNAPSHOT_DIR_NAME: &str = "sync-snapshots";
const MAX_SNAPSHOTS: usize = 30;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncGroupRecord {
    id: String,
    name: Option<String>,
    member_uids: Vec<String>,
    created_at: u64,
    updated_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncMappingRecord {
    source_uid: String,
    target_uid: String,
    last_synced_hash: Option<String>,
    updated_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SnapshotEntryRecord {
    target_uid: String,
    target_path: String,
    backup_path: String,
    had_original: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncSnapshotRecord {
    id: String,
    source_uid: String,
    source_hash: String,
    entries: Vec<SnapshotEntryRecord>,
    created_at: u64,
    rolled_back: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncIndex {
    version: u32,
    sync_groups: Vec<SyncGroupRecord>,
    mappings: Vec<SyncMappingRecord>,
    snapshots: Vec<SyncSnapshotRecord>,
}

impl Default for SyncIndex {
    fn default() -> Self {
        Self {
            version: 1,
            sync_groups: Vec::new(),
            mappings: Vec::new(),
            snapshots: Vec::new(),
        }
    }
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
    Ok(app_data_root()?.join(INDEX_FILE_NAME))
}

fn snapshots_root() -> Result<PathBuf, String> {
    let root = app_data_root()?.join(SNAPSHOT_DIR_NAME);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

fn load_index() -> Result<SyncIndex, String> {
    let path = index_path()?;
    if !path.exists() {
        return Ok(SyncIndex::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str::<SyncIndex>(&content).map_err(|e| e.to_string())
}

fn save_index(index: &SyncIndex) -> Result<(), String> {
    let path = index_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| "Invalid sync index path".to_string())?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp_path = path.with_extension("tmp");
    let payload = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
    fs::write(&tmp_path, payload).map_err(|e| e.to_string())?;
    fs::rename(&tmp_path, &path).map_err(|e| e.to_string())
}

fn sanitize_uid(uid: &str) -> String {
    uid.replace(':', "_")
        .replace('/', "_")
        .replace('\\', "_")
        .replace(' ', "_")
}

fn dedupe_targets(source_uid: &str, target_uids: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    target_uids
        .iter()
        .filter(|u| !u.trim().is_empty())
        .filter(|u| u.as_str() != source_uid)
        .filter(|u| seen.insert((*u).clone()))
        .cloned()
        .collect()
}

fn find_skill_by_uid<'a>(skills: &'a [Skill], uid: &str) -> Result<&'a Skill, String> {
    skills
        .iter()
        .find(|s| s.uid == uid)
        .ok_or_else(|| format!("Skill uid not found: {}", uid))
}

fn mapping_baseline(index: &SyncIndex, source_uid: &str, target_uid: &str) -> Option<String> {
    index
        .mappings
        .iter()
        .find(|m| m.source_uid == source_uid && m.target_uid == target_uid)
        .and_then(|m| m.last_synced_hash.clone())
}

fn upsert_mapping_baseline(index: &mut SyncIndex, source_uid: &str, target_uid: &str, hash: &str) {
    let ts = now_epoch_seconds();
    if let Some(mapping) = index
        .mappings
        .iter_mut()
        .find(|m| m.source_uid == source_uid && m.target_uid == target_uid)
    {
        mapping.last_synced_hash = Some(hash.to_string());
        mapping.updated_at = ts;
    } else {
        index.mappings.push(SyncMappingRecord {
            source_uid: source_uid.to_string(),
            target_uid: target_uid.to_string(),
            last_synced_hash: Some(hash.to_string()),
            updated_at: ts,
        });
    }
}

fn compute_decision(
    source_hash: &str,
    target_hash: &str,
    baseline_hash: Option<&str>,
) -> SyncDecisionType {
    if source_hash == target_hash {
        return SyncDecisionType::NoChange;
    }

    let Some(base) = baseline_hash else {
        return SyncDecisionType::Diverged;
    };

    let source_changed = source_hash != base;
    let target_changed = target_hash != base;

    match (source_changed, target_changed) {
        (true, false) => SyncDecisionType::SourceChanged,
        (false, true) => SyncDecisionType::TargetChanged,
        (false, false) => SyncDecisionType::Diverged,
        (true, true) => SyncDecisionType::Diverged,
    }
}

fn diff_file_maps(
    source_files: &BTreeMap<String, String>,
    target_files: &BTreeMap<String, String>,
) -> Vec<SyncFileChange> {
    let mut changes = Vec::new();

    for (path, src_hash) in source_files {
        match target_files.get(path) {
            None => changes.push(SyncFileChange {
                path: path.clone(),
                change_type: SyncFileChangeType::Added,
            }),
            Some(tgt_hash) if tgt_hash != src_hash => changes.push(SyncFileChange {
                path: path.clone(),
                change_type: SyncFileChangeType::Modified,
            }),
            _ => {}
        }
    }

    for path in target_files.keys() {
        if !source_files.contains_key(path) {
            changes.push(SyncFileChange {
                path: path.clone(),
                change_type: SyncFileChangeType::Deleted,
            });
        }
    }

    changes.sort_by(|a, b| a.path.cmp(&b.path));
    changes
}

fn is_noise_file(name: &str) -> bool {
    name.eq_ignore_ascii_case(".ds_store")
        || name.eq_ignore_ascii_case("thumbs.db")
        || name.eq_ignore_ascii_case("desktop.ini")
}

fn is_noise_dir(name: &str) -> bool {
    name.eq_ignore_ascii_case(".git") || name.eq_ignore_ascii_case(".skilldeck-sync-snapshots")
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

    let staging = parent.join(format!(".skilldeck-sync-staging-{}", Uuid::new_v4()));
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

fn prune_old_snapshots(index: &mut SyncIndex) {
    if index.snapshots.len() <= MAX_SNAPSHOTS {
        return;
    }
    index
        .snapshots
        .sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let remove = index.snapshots.split_off(MAX_SNAPSHOTS);
    for snapshot in remove {
        let _ = snapshots_root().and_then(|root| {
            let dir = root.join(&snapshot.id);
            if dir.exists() {
                symlink::remove_link(&dir)?;
            }
            Ok(())
        });
    }
}

pub fn attach_sync_group_ids(skills: &mut [Skill]) -> Result<(), String> {
    let index = load_index()?;
    let mut membership: HashMap<String, String> = HashMap::new();
    for group in index.sync_groups {
        for uid in group.member_uids {
            membership.entry(uid).or_insert_with(|| group.id.clone());
        }
    }
    for skill in skills {
        skill.sync_group_id = membership.get(&skill.uid).cloned();
    }
    Ok(())
}

pub fn upsert_sync_mapping(
    source_uid: String,
    target_uids: Vec<String>,
    group_name: Option<String>,
) -> Result<SyncMappingUpsertResponse, String> {
    let targets = dedupe_targets(&source_uid, &target_uids);
    if targets.is_empty() {
        return Err(
            "target_uids must include at least one uid different from source_uid".to_string(),
        );
    }

    let mut index = load_index()?;
    let now = now_epoch_seconds();
    let mut members: Vec<String> = Vec::with_capacity(targets.len() + 1);
    members.push(source_uid.clone());
    members.extend(targets.clone());
    members.sort();
    members.dedup();

    let requested_group_name = group_name
        .as_ref()
        .map(|n| n.trim())
        .filter(|n| !n.is_empty())
        .map(|n| n.to_string());

    let group_id = if let Some(name) = requested_group_name.clone() {
        if let Some(group) = index
            .sync_groups
            .iter_mut()
            .find(|g| g.name.as_ref().map(|x| x == &name).unwrap_or(false))
        {
            let mut set: HashSet<String> = group.member_uids.iter().cloned().collect();
            for uid in &members {
                set.insert(uid.clone());
            }
            group.member_uids = set.into_iter().collect();
            group.member_uids.sort();
            group.updated_at = now;
            group.id.clone()
        } else {
            let id = Uuid::new_v4().to_string();
            index.sync_groups.push(SyncGroupRecord {
                id: id.clone(),
                name: Some(name),
                member_uids: members.clone(),
                created_at: now,
                updated_at: now,
            });
            id
        }
    } else {
        let id = Uuid::new_v4().to_string();
        index.sync_groups.push(SyncGroupRecord {
            id: id.clone(),
            name: None,
            member_uids: members.clone(),
            created_at: now,
            updated_at: now,
        });
        id
    };

    for target_uid in &targets {
        if let Some(record) = index
            .mappings
            .iter_mut()
            .find(|m| m.source_uid == source_uid && m.target_uid == *target_uid)
        {
            record.updated_at = now;
        } else {
            index.mappings.push(SyncMappingRecord {
                source_uid: source_uid.clone(),
                target_uid: target_uid.clone(),
                last_synced_hash: None,
                updated_at: now,
            });
        }
    }

    save_index(&index)?;

    Ok(SyncMappingUpsertResponse {
        group_id,
        source_uid,
        target_uids: targets,
    })
}

pub fn preview_sync(
    source_uid: String,
    target_uids: Vec<String>,
) -> Result<SyncPreviewResponse, String> {
    let targets = dedupe_targets(&source_uid, &target_uids);
    if targets.is_empty() {
        return Err(
            "target_uids must include at least one uid different from source_uid".to_string(),
        );
    }

    let index = load_index()?;
    let skills = scanner::scan_all_v2()?;
    let source_skill = find_skill_by_uid(&skills, &source_uid)?;
    let source_hash = source_skill.tree_hash.clone();
    let source_files = tree_hash::collect_file_hashes(&source_skill.canonical_path)?;

    let mut previews = Vec::new();
    for target_uid in targets {
        let target_skill = find_skill_by_uid(&skills, &target_uid)?;
        let target_files = tree_hash::collect_file_hashes(&target_skill.canonical_path)?;
        let baseline = mapping_baseline(&index, &source_uid, &target_uid);
        let decision = compute_decision(&source_hash, &target_skill.tree_hash, baseline.as_deref());
        let changes = diff_file_maps(&source_files, &target_files);
        previews.push(SyncTargetPreview {
            target_uid,
            target_hash: target_skill.tree_hash.clone(),
            baseline_hash: baseline,
            decision,
            changes,
        });
    }

    Ok(SyncPreviewResponse {
        source_uid,
        source_hash,
        targets: previews,
    })
}

pub fn apply_sync(
    source_uid: String,
    target_uids: Vec<String>,
    expected_source_hash: String,
) -> Result<ApplySyncResponse, String> {
    let targets = dedupe_targets(&source_uid, &target_uids);
    if targets.is_empty() {
        return Err(
            "target_uids must include at least one uid different from source_uid".to_string(),
        );
    }

    let mut index = load_index()?;
    let skills = scanner::scan_all_v2()?;
    let source_skill = find_skill_by_uid(&skills, &source_uid)?;

    if source_skill.tree_hash != expected_source_hash {
        return Err(format!(
            "Source hash mismatch. expected={}, actual={}",
            expected_source_hash, source_skill.tree_hash
        ));
    }

    let source_hash = source_skill.tree_hash.clone();
    let source_files = tree_hash::collect_file_hashes(&source_skill.canonical_path)?;
    let snapshot_id = Uuid::new_v4().to_string();
    let snapshot_dir = snapshots_root()?.join(&snapshot_id);
    fs::create_dir_all(&snapshot_dir).map_err(|e| e.to_string())?;

    let mut updated_targets = Vec::new();
    let mut snapshot_entries = Vec::new();

    for target_uid in targets {
        let target_skill = find_skill_by_uid(&skills, &target_uid)?;
        let target_path = target_skill.canonical_path.clone();
        let previous_hash = Some(target_skill.tree_hash.clone());
        let previous_files = tree_hash::collect_file_hashes(&target_path)?;
        let changed_files = diff_file_maps(&source_files, &previous_files).len();

        let backup_path = snapshot_dir.join(sanitize_uid(&target_uid));
        let had_original = target_path.exists();
        if had_original {
            copy_dir_recursive(&target_path, &backup_path)?;
        }

        replace_directory_atomic(&source_skill.canonical_path, &target_path)?;
        let new_hash = tree_hash::compute_tree_hash(&target_path)?;

        upsert_mapping_baseline(&mut index, &source_uid, &target_uid, &source_hash);

        snapshot_entries.push(SnapshotEntryRecord {
            target_uid: target_uid.clone(),
            target_path: target_path.to_string_lossy().to_string(),
            backup_path: backup_path.to_string_lossy().to_string(),
            had_original,
        });

        updated_targets.push(AppliedTarget {
            target_uid,
            previous_hash,
            new_hash,
            changed_files,
        });
    }

    index.snapshots.push(SyncSnapshotRecord {
        id: snapshot_id.clone(),
        source_uid: source_uid.clone(),
        source_hash: source_hash.clone(),
        entries: snapshot_entries,
        created_at: now_epoch_seconds(),
        rolled_back: false,
    });
    prune_old_snapshots(&mut index);
    save_index(&index)?;

    Ok(ApplySyncResponse {
        snapshot_id,
        source_uid,
        source_hash,
        updated_targets,
    })
}

pub fn rollback_sync(snapshot_id: String) -> Result<RollbackSyncResponse, String> {
    let mut index = load_index()?;
    let snapshot = index
        .snapshots
        .iter_mut()
        .find(|s| s.id == snapshot_id)
        .ok_or_else(|| format!("Snapshot not found: {}", snapshot_id))?;

    if snapshot.rolled_back {
        return Ok(RollbackSyncResponse {
            snapshot_id,
            restored_targets: Vec::new(),
        });
    }

    let mut restored_targets = Vec::new();
    for entry in &snapshot.entries {
        let target_path = PathBuf::from(&entry.target_path);
        if target_path.exists() {
            symlink::remove_link(&target_path)?;
        }

        if entry.had_original {
            let backup_path = PathBuf::from(&entry.backup_path);
            if backup_path.exists() {
                copy_dir_recursive(&backup_path, &target_path)?;
            }
        }
        restored_targets.push(entry.target_uid.clone());
    }
    snapshot.rolled_back = true;
    save_index(&index)?;

    Ok(RollbackSyncResponse {
        snapshot_id,
        restored_targets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_uses_baseline() {
        assert_eq!(
            compute_decision("a", "a", Some("z")),
            SyncDecisionType::NoChange
        );
        assert_eq!(
            compute_decision("a", "b", Some("b")),
            SyncDecisionType::SourceChanged
        );
        assert_eq!(
            compute_decision("a", "b", Some("a")),
            SyncDecisionType::TargetChanged
        );
        assert_eq!(
            compute_decision("a", "b", Some("c")),
            SyncDecisionType::Diverged
        );
        assert_eq!(compute_decision("a", "b", None), SyncDecisionType::Diverged);
    }
}
