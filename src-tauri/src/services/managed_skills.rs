use crate::models::{
    AgentType, CoverSkippedTarget, ManagedSkillActionResponse, ManagedSkillSource,
    ManagedSkillUpdateRequest, ManagedTargetAction, ManagedTargetResult,
    RegistrySkillInstallRequest, Skill, SkillMetadata, SkillOriginType, TargetMode,
};
use crate::services::{
    agent_detector, claude_bootstrap, clawhub, cover_history, md_parser, scanner, symlink,
    tree_hash,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

const MANAGED_METADATA_FILE: &str = ".skilldeck-source.json";
const REMOTE_SYNC_TTL_SECONDS: u64 = 15 * 60;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ManagedSkillMetadataFile {
    version: u32,
    provider: SkillOriginType,
    remote_slug: String,
    source_repo: Option<String>,
    source_ref: Option<String>,
    installed_version_label: Option<String>,
    remote_version_label: Option<String>,
    registry_url: Option<String>,
    last_synced_at: Option<u64>,
    local_commit_hash: Option<String>,
}

struct PreparedSourceUpdate {
    version_label: String,
    remote_version_label: Option<String>,
    remote_tree_hash: Option<String>,
    remote_commit_hash: Option<String>,
    local_commit_hash: Option<String>,
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn short_hash(value: &str) -> String {
    value.chars().take(8).collect()
}

fn metadata_path(root: &Path) -> PathBuf {
    root.join(MANAGED_METADATA_FILE)
}

fn current_version_label(skill: &Skill) -> String {
    skill
        .metadata
        .version
        .clone()
        .or_else(|| {
            skill
                .managed_source
                .as_ref()
                .and_then(|source| source.installed_version_label.clone())
        })
        .unwrap_or_else(|| short_hash(&skill.tree_hash))
}

fn modified_at_seconds(path: &Path) -> Option<u64> {
    path.metadata()
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

fn is_recent_sync(last_synced_at: Option<u64>) -> bool {
    match last_synced_at {
        Some(timestamp) => now_epoch_seconds().saturating_sub(timestamp) < REMOTE_SYNC_TTL_SECONDS,
        None => false,
    }
}

fn read_skill_metadata(path: &Path) -> Option<SkillMetadata> {
    let content = fs::read_to_string(path.join("SKILL.md")).ok()?;
    let (metadata, _) = md_parser::parse(&content).ok()?;
    Some(metadata)
}

fn read_managed_metadata(path: &Path) -> Option<ManagedSkillMetadataFile> {
    let file_path = metadata_path(path);
    let content = fs::read_to_string(file_path).ok()?;
    serde_json::from_str::<ManagedSkillMetadataFile>(&content).ok()
}

fn write_managed_metadata(path: &Path, metadata: &ManagedSkillMetadataFile) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| error.to_string())?;
    let file_path = metadata_path(path);
    let temp_path = file_path.with_extension("tmp");
    let payload = serde_json::to_string_pretty(metadata).map_err(|error| error.to_string())?;
    fs::write(&temp_path, payload).map_err(|error| error.to_string())?;
    fs::rename(&temp_path, &file_path).map_err(|error| error.to_string())
}

fn legacy_bootstrap_source(
    path: &Path,
    namespace: &str,
    slug: &str,
) -> Option<ManagedSkillMetadataFile> {
    if namespace != "agent:claude-code" {
        return None;
    }
    if !claude_bootstrap::is_bootstrap_slug(slug) {
        return None;
    }
    if path.parent() != AgentType::ClaudeCode.skills_dir().as_deref() {
        return None;
    }

    Some(ManagedSkillMetadataFile {
        version: 1,
        provider: SkillOriginType::ClaudeBootstrapManaged,
        remote_slug: slug.to_string(),
        source_repo: Some(claude_bootstrap::bootstrap_source_repo().to_string()),
        source_ref: Some(claude_bootstrap::bootstrap_source_ref().to_string()),
        installed_version_label: read_skill_metadata(path).and_then(|metadata| metadata.version),
        remote_version_label: None,
        registry_url: None,
        last_synced_at: None,
        local_commit_hash: None,
    })
}

fn metadata_to_source(metadata: &ManagedSkillMetadataFile) -> ManagedSkillSource {
    ManagedSkillSource {
        provider: metadata.provider,
        remote_slug: metadata.remote_slug.clone(),
        source_repo: metadata.source_repo.clone(),
        source_ref: metadata.source_ref.clone(),
        installed_version_label: metadata.installed_version_label.clone(),
        remote_version_label: metadata.remote_version_label.clone(),
        registry_url: metadata.registry_url.clone(),
        last_synced_at: metadata.last_synced_at,
    }
}

fn upsert_remote_sync_metadata(
    skill: &Skill,
    remote_version_label: Option<String>,
    local_commit_hash: Option<String>,
) {
    let mut metadata = read_managed_metadata(&skill.canonical_path)
        .or_else(|| legacy_bootstrap_source(&skill.canonical_path, &skill.namespace, &skill.slug));
    let Some(mut metadata) = metadata.take() else {
        return;
    };

    metadata.installed_version_label = Some(current_version_label(skill));
    metadata.remote_version_label = remote_version_label;
    metadata.last_synced_at = Some(now_epoch_seconds());
    metadata.local_commit_hash = local_commit_hash;
    let _ = write_managed_metadata(&skill.canonical_path, &metadata);
}

fn origin_label(origin_type: SkillOriginType) -> String {
    match origin_type {
        SkillOriginType::LocalManual => "Local".to_string(),
        SkillOriginType::ClawhubManaged => "ClawHub".to_string(),
        SkillOriginType::ClaudeBootstrapManaged => "Claude Bootstrap".to_string(),
    }
}

pub fn attach_origin_metadata(skills: &mut [Skill]) {
    for skill in skills.iter_mut() {
        let metadata = read_managed_metadata(&skill.canonical_path).or_else(|| {
            legacy_bootstrap_source(&skill.canonical_path, &skill.namespace, &skill.slug)
        });
        skill.modified_at = modified_at_seconds(&skill.canonical_path);
        skill.has_update = false;
        skill.remote_version_label = None;
        skill.remote_tree_hash = None;
        skill.remote_commit_hash = None;
        skill.local_commit_hash = None;

        if let Some(metadata) = metadata {
            skill.origin_type = metadata.provider;
            skill.origin_label = origin_label(metadata.provider);
            skill.origin_slug = Some(metadata.remote_slug.clone());
            skill.remote_version_label = metadata.remote_version_label.clone();
            skill.local_commit_hash = metadata.local_commit_hash.clone();
            skill.managed_source = Some(metadata_to_source(&metadata));
        } else {
            skill.origin_type = SkillOriginType::LocalManual;
            skill.origin_label = origin_label(SkillOriginType::LocalManual);
            skill.origin_slug = None;
            skill.managed_source = None;
        }
    }
}

fn update_skill_from_prepared(skill: &mut Skill, prepared: &PreparedSourceUpdate) {
    skill.remote_version_label = prepared.remote_version_label.clone();
    skill.remote_tree_hash = prepared.remote_tree_hash.clone();
    skill.remote_commit_hash = prepared.remote_commit_hash.clone();
    skill.local_commit_hash = prepared.local_commit_hash.clone();
    skill.has_update = match &prepared.remote_tree_hash {
        Some(remote_tree_hash) => remote_tree_hash != &skill.tree_hash,
        None => prepared.version_label != current_version_label(skill),
    };
    if let Some(managed_source) = skill.managed_source.as_mut() {
        managed_source.remote_version_label = prepared.remote_version_label.clone();
    }
}

pub async fn hydrate_remote_updates(skills: &mut [Skill]) {
    let clawhub_indices: Vec<usize> = skills
        .iter()
        .enumerate()
        .filter_map(|(index, skill)| {
            (skill.origin_type == SkillOriginType::ClawhubManaged).then_some(index)
        })
        .collect();
    if !clawhub_indices.is_empty() {
        let service = clawhub::ClawHubService::new();
        for index in clawhub_indices {
            if let Some(managed_source) = skills[index].managed_source.as_ref() {
                if is_recent_sync(managed_source.last_synced_at) {
                    let remote_version = managed_source.remote_version_label.clone();
                    skills[index].remote_version_label = remote_version.clone();
                    skills[index].has_update = remote_version
                        .as_ref()
                        .map(|remote| remote != &current_version_label(&skills[index]))
                        .unwrap_or(false);
                    continue;
                }
            }

            let slug = skills[index]
                .origin_slug
                .clone()
                .unwrap_or_else(|| skills[index].slug.clone());
            let detail = match service.fetch_skill_detail(&slug).await {
                Ok(detail) => detail,
                Err(_) => continue,
            };
            let remote_version = detail.latest_version.clone();
            skills[index].remote_version_label = remote_version.clone();
            skills[index].has_update = remote_version
                .as_ref()
                .map(|remote| remote != &current_version_label(&skills[index]))
                .unwrap_or(false);
            if let Some(managed_source) = skills[index].managed_source.as_mut() {
                managed_source.remote_version_label = remote_version;
                managed_source.last_synced_at = Some(now_epoch_seconds());
            }
            let local_commit_hash = skills[index].local_commit_hash.clone();
            upsert_remote_sync_metadata(
                &skills[index],
                skills[index].remote_version_label.clone(),
                local_commit_hash,
            );
        }
    }

    let bootstrap_indices: Vec<usize> = skills
        .iter()
        .enumerate()
        .filter_map(|(index, skill)| {
            (skill.origin_type == SkillOriginType::ClaudeBootstrapManaged).then_some(index)
        })
        .collect();
    if bootstrap_indices.is_empty() {
        return;
    }

    let repo_root =
        match crate::services::git::clone_repo(claude_bootstrap::bootstrap_repo_url(), true) {
            Ok(path) => path,
            Err(_) => return,
        };

    let remote_commit_hash = crate::services::git::get_commit_hash(&repo_root).ok();
    for index in bootstrap_indices {
        if let Some(managed_source) = skills[index].managed_source.as_ref() {
            if is_recent_sync(managed_source.last_synced_at) {
                skills[index].remote_version_label = managed_source.remote_version_label.clone();
                skills[index].has_update = skills[index]
                    .remote_version_label
                    .as_ref()
                    .map(|remote| remote != &current_version_label(&skills[index]))
                    .unwrap_or(false);
                continue;
            }
        }

        let slug = skills[index]
            .origin_slug
            .clone()
            .unwrap_or_else(|| skills[index].slug.clone());
        let remote_path = repo_root.join("skills").join(&slug);
        if !remote_path.join("SKILL.md").exists() {
            continue;
        }

        let remote_tree_hash = match tree_hash::compute_tree_hash(&remote_path) {
            Ok(hash) => hash,
            Err(_) => continue,
        };
        let remote_version_label = read_skill_metadata(&remote_path)
            .and_then(|metadata| metadata.version)
            .unwrap_or_else(|| short_hash(&remote_tree_hash));
        skills[index].remote_tree_hash = Some(remote_tree_hash.clone());
        skills[index].remote_commit_hash = remote_commit_hash.clone();
        skills[index].remote_version_label = Some(remote_version_label.clone());
        skills[index].has_update = remote_tree_hash != skills[index].tree_hash;
        if let Some(managed_source) = skills[index].managed_source.as_mut() {
            managed_source.remote_version_label = Some(remote_version_label);
            managed_source.last_synced_at = Some(now_epoch_seconds());
        }
        let local_commit_hash = skills[index].local_commit_hash.clone();
        upsert_remote_sync_metadata(
            &skills[index],
            skills[index].remote_version_label.clone(),
            local_commit_hash,
        );
    }

    let _ = fs::remove_dir_all(repo_root);
}

fn target_skills_path(agent_type: AgentType, slug: &str) -> Result<PathBuf, String> {
    let dir = agent_type
        .skills_dir()
        .ok_or_else(|| "Could not determine agent skills directory".to_string())?;
    Ok(dir.join(slug))
}

fn apply_target(
    source_skill: &Skill,
    target_agent_type: AgentType,
) -> Result<Option<ManagedTargetResult>, String> {
    let target_path = target_skills_path(target_agent_type, &source_skill.slug)?;
    if target_path == source_skill.canonical_path {
        return Ok(None);
    }

    if target_path.symlink_metadata().is_ok() {
        let resolved = if symlink::is_link(&target_path) {
            symlink::resolve_link(&target_path)
        } else {
            target_path.clone()
        };

        if resolved == source_skill.canonical_path {
            return Ok(None);
        }

        cover_history::record_uninstall_history(source_skill, target_agent_type)?;
        symlink::remove_link(&target_path)?;
        symlink::create_link(&source_skill.canonical_path, &target_path)?;
        return Ok(Some(ManagedTargetResult {
            target_agent_type,
            target_path: target_path.to_string_lossy().to_string(),
            action: ManagedTargetAction::Relinked,
        }));
    }

    symlink::create_link(&source_skill.canonical_path, &target_path)?;
    Ok(Some(ManagedTargetResult {
        target_agent_type,
        target_path: target_path.to_string_lossy().to_string(),
        action: ManagedTargetAction::Linked,
    }))
}

fn collect_targets(
    target_mode: TargetMode,
    target_agent_type: Option<AgentType>,
) -> Result<(Vec<AgentType>, Vec<CoverSkippedTarget>), String> {
    match target_mode {
        TargetMode::SingleAgent => {
            let agent_type = target_agent_type.ok_or_else(|| {
                "Target agent type is required for single-agent actions".to_string()
            })?;
            Ok((vec![agent_type], Vec::new()))
        }
        TargetMode::AllAvailable => {
            let mut targets = Vec::new();
            let mut skipped = Vec::new();
            let mut seen = HashSet::new();
            for agent in agent_detector::detect_all() {
                if !seen.insert(agent.agent_type) {
                    continue;
                }
                if agent.is_installed || agent.skills_directory_exists || agent.skill_count > 0 {
                    targets.push(agent.agent_type);
                } else {
                    skipped.push(CoverSkippedTarget {
                        target_agent_type: agent.agent_type,
                        reason: "Agent unavailable: missing CLI and skills directory".to_string(),
                    });
                }
            }
            Ok((targets, skipped))
        }
    }
}

fn apply_targets(
    source_skill: &Skill,
    target_mode: TargetMode,
    target_agent_type: Option<AgentType>,
) -> Result<(Vec<ManagedTargetResult>, Vec<CoverSkippedTarget>), String> {
    let (targets, mut skipped) = collect_targets(target_mode, target_agent_type)?;
    let mut results = Vec::new();
    for target in targets {
        match apply_target(source_skill, target) {
            Ok(Some(result)) => results.push(result),
            Ok(None) => skipped.push(CoverSkippedTarget {
                target_agent_type: target,
                reason: "Target already linked to managed source".to_string(),
            }),
            Err(error) => skipped.push(CoverSkippedTarget {
                target_agent_type: target,
                reason: format!("Failed: {}", error),
            }),
        }
    }
    Ok((results, skipped))
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn replace_directory(source: &Path, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("Invalid target path: {}", target.display()))?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let staging = parent.join(format!(".skilldeck-managed-stage-{}", uuid::Uuid::new_v4()));
    if staging.exists() {
        symlink::remove_link(&staging)?;
    }
    copy_dir_recursive(source, &staging)?;
    if target.symlink_metadata().is_ok() {
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

fn write_zip_to_directory(bytes: &[u8], directory: &Path) -> Result<(), String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| error.to_string())?;
        let Some(relative_path) = file.enclosed_name().map(|path| path.to_path_buf()) else {
            continue;
        };
        let target_path = directory.join(relative_path);
        if file.is_dir() {
            fs::create_dir_all(&target_path).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut output = fs::File::create(&target_path).map_err(|error| error.to_string())?;
        io::copy(&mut file, &mut output).map_err(|error| error.to_string())?;
    }

    if !directory.join("SKILL.md").exists() {
        return Err("Downloaded skill bundle is missing SKILL.md".to_string());
    }
    Ok(())
}

async fn prepare_clawhub_update(
    skill_dir: &Path,
    slug: &str,
    version_or_tag: Option<&str>,
) -> Result<PreparedSourceUpdate, String> {
    let service = clawhub::ClawHubService::new();
    let detail = service.fetch_skill_detail(slug).await?;
    let requested = version_or_tag
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let remote_version_label = requested
        .map(|value| value.to_string())
        .or(detail.latest_version.clone());
    let bytes = service.download_skill_zip(slug, requested).await?;
    let temp_dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    write_zip_to_directory(&bytes, temp_dir.path())?;
    let remote_tree_hash = tree_hash::compute_tree_hash(temp_dir.path())?;
    let remote_metadata = read_skill_metadata(temp_dir.path());
    let installed_version_label = remote_metadata
        .as_ref()
        .and_then(|metadata| metadata.version.clone())
        .or(remote_version_label.clone())
        .unwrap_or_else(|| short_hash(&remote_tree_hash));
    replace_directory(temp_dir.path(), skill_dir)?;
    let metadata = ManagedSkillMetadataFile {
        version: 1,
        provider: SkillOriginType::ClawhubManaged,
        remote_slug: slug.to_string(),
        source_repo: None,
        source_ref: None,
        installed_version_label: Some(installed_version_label.clone()),
        remote_version_label: remote_version_label.clone(),
        registry_url: Some(service.base_url().to_string()),
        last_synced_at: Some(now_epoch_seconds()),
        local_commit_hash: None,
    };
    write_managed_metadata(skill_dir, &metadata)?;
    Ok(PreparedSourceUpdate {
        version_label: installed_version_label,
        remote_version_label,
        remote_tree_hash: Some(remote_tree_hash),
        remote_commit_hash: None,
        local_commit_hash: None,
    })
}

fn prepare_bootstrap_update(skill_dir: &Path, slug: &str) -> Result<PreparedSourceUpdate, String> {
    let repo_root = crate::services::git::clone_repo(claude_bootstrap::bootstrap_repo_url(), true)
        .map_err(|error| format!("CLAUDE_BOOTSTRAP_CLONE_FAILED: {}", error))?;
    let remote_path = repo_root.join("skills").join(slug);
    if !remote_path.join("SKILL.md").exists() {
        let _ = fs::remove_dir_all(&repo_root);
        return Err(format!("CLAUDE_BOOTSTRAP_SOURCE_SKILL_MISSING: {}", slug));
    }

    let remote_tree_hash = tree_hash::compute_tree_hash(&remote_path)?;
    let remote_commit_hash = crate::services::git::get_commit_hash(&repo_root).ok();
    let remote_version_label = read_skill_metadata(&remote_path)
        .and_then(|metadata| metadata.version)
        .unwrap_or_else(|| short_hash(&remote_tree_hash));
    replace_directory(&remote_path, skill_dir)?;
    let metadata = ManagedSkillMetadataFile {
        version: 1,
        provider: SkillOriginType::ClaudeBootstrapManaged,
        remote_slug: slug.to_string(),
        source_repo: Some(claude_bootstrap::bootstrap_source_repo().to_string()),
        source_ref: Some(claude_bootstrap::bootstrap_source_ref().to_string()),
        installed_version_label: Some(remote_version_label.clone()),
        remote_version_label: Some(remote_version_label.clone()),
        registry_url: None,
        last_synced_at: Some(now_epoch_seconds()),
        local_commit_hash: remote_commit_hash.clone(),
    };
    write_managed_metadata(skill_dir, &metadata)?;
    let _ = fs::remove_dir_all(repo_root);
    Ok(PreparedSourceUpdate {
        version_label: remote_version_label.clone(),
        remote_version_label: Some(remote_version_label),
        remote_tree_hash: Some(remote_tree_hash),
        remote_commit_hash: remote_commit_hash.clone(),
        local_commit_hash: remote_commit_hash,
    })
}

fn shared_skill_path(slug: &str) -> Result<PathBuf, String> {
    let shared_root = AgentType::shared_skills_dir()
        .ok_or_else(|| "Could not determine shared skills directory".to_string())?;
    fs::create_dir_all(&shared_root).map_err(|error| error.to_string())?;
    Ok(shared_root.join(slug))
}

fn find_skill_by_uid(skills: &[Skill], uid: &str) -> Result<Skill, String> {
    skills
        .iter()
        .find(|skill| skill.uid == uid)
        .cloned()
        .ok_or_else(|| format!("Skill uid not found: {}", uid))
}

fn scan_skills_with_origin() -> Result<Vec<Skill>, String> {
    let mut skills = scanner::scan_all_v2()?;
    attach_origin_metadata(&mut skills);
    Ok(skills)
}

pub fn write_bootstrap_metadata(
    path: &Path,
    slug: &str,
    version_label: Option<String>,
) -> Result<(), String> {
    let metadata = ManagedSkillMetadataFile {
        version: 1,
        provider: SkillOriginType::ClaudeBootstrapManaged,
        remote_slug: slug.to_string(),
        source_repo: Some(claude_bootstrap::bootstrap_source_repo().to_string()),
        source_ref: Some(claude_bootstrap::bootstrap_source_ref().to_string()),
        installed_version_label: version_label.clone(),
        remote_version_label: version_label,
        registry_url: None,
        last_synced_at: Some(now_epoch_seconds()),
        local_commit_hash: None,
    };
    write_managed_metadata(path, &metadata)
}

pub async fn install_registry_skill(
    request: RegistrySkillInstallRequest,
) -> Result<ManagedSkillActionResponse, String> {
    let skill_path = shared_skill_path(&request.slug)?;
    if skill_path.exists() {
        let existing_metadata = read_managed_metadata(&skill_path);
        if existing_metadata
            .as_ref()
            .map(|metadata| metadata.provider != SkillOriginType::ClawhubManaged)
            .unwrap_or(true)
        {
            return Err(format!(
                "A local skill with slug '{}' already exists and is not managed by ClawHub",
                request.slug
            ));
        }
    }

    let requested_version = request.version_or_tag.as_deref();
    let prepared = prepare_clawhub_update(&skill_path, &request.slug, requested_version).await?;
    let skills = scan_skills_with_origin()?;
    let mut source_skill = find_skill_by_uid(&skills, &format!("shared:agents:{}", request.slug))?;
    update_skill_from_prepared(&mut source_skill, &prepared);
    let (results, skipped) = apply_targets(
        &source_skill,
        request.target_mode,
        request.target_agent_type,
    )?;
    Ok(ManagedSkillActionResponse {
        source_uid: source_skill.uid,
        source_slug: source_skill.slug,
        source_version_label: prepared.version_label,
        remote_version_label: prepared.remote_version_label,
        updated_source: true,
        already_latest: false,
        results,
        skipped,
    })
}

pub async fn update_managed_skill(
    request: ManagedSkillUpdateRequest,
) -> Result<ManagedSkillActionResponse, String> {
    let skills = scan_skills_with_origin()?;
    let source_skill = find_skill_by_uid(&skills, &request.source_uid)?;
    let managed_source = source_skill
        .managed_source
        .clone()
        .ok_or_else(|| "Local skills cannot be updated automatically".to_string())?;

    let prepared = match source_skill.origin_type {
        SkillOriginType::LocalManual => {
            return Err("Local skills cannot be updated automatically".to_string())
        }
        SkillOriginType::ClawhubManaged => {
            let service = clawhub::ClawHubService::new();
            let detail = service
                .fetch_skill_detail(&managed_source.remote_slug)
                .await?;
            let remote_version_label = detail.latest_version.clone();
            let current_label = current_version_label(&source_skill);
            if remote_version_label.as_deref() == Some(current_label.as_str()) {
                PreparedSourceUpdate {
                    version_label: current_label,
                    remote_version_label,
                    remote_tree_hash: None,
                    remote_commit_hash: None,
                    local_commit_hash: source_skill.local_commit_hash.clone(),
                }
            } else {
                prepare_clawhub_update(
                    &source_skill.canonical_path,
                    &managed_source.remote_slug,
                    None,
                )
                .await?
            }
        }
        SkillOriginType::ClaudeBootstrapManaged => {
            let repo_root =
                crate::services::git::clone_repo(claude_bootstrap::bootstrap_repo_url(), true)
                    .map_err(|error| format!("CLAUDE_BOOTSTRAP_CLONE_FAILED: {}", error))?;
            let remote_path = repo_root.join("skills").join(&managed_source.remote_slug);
            if !remote_path.join("SKILL.md").exists() {
                let _ = fs::remove_dir_all(&repo_root);
                return Err(format!(
                    "CLAUDE_BOOTSTRAP_SOURCE_SKILL_MISSING: {}",
                    managed_source.remote_slug
                ));
            }
            let remote_tree_hash = tree_hash::compute_tree_hash(&remote_path)?;
            let remote_commit_hash = crate::services::git::get_commit_hash(&repo_root).ok();
            let remote_version_label = read_skill_metadata(&remote_path)
                .and_then(|metadata| metadata.version)
                .unwrap_or_else(|| short_hash(&remote_tree_hash));
            let current_label = current_version_label(&source_skill);
            let prepared = if remote_tree_hash == source_skill.tree_hash {
                PreparedSourceUpdate {
                    version_label: current_label,
                    remote_version_label: Some(remote_version_label),
                    remote_tree_hash: Some(remote_tree_hash),
                    remote_commit_hash: remote_commit_hash.clone(),
                    local_commit_hash: source_skill.local_commit_hash.clone(),
                }
            } else {
                prepare_bootstrap_update(&source_skill.canonical_path, &managed_source.remote_slug)?
            };
            let _ = fs::remove_dir_all(&repo_root);
            prepared
        }
    };

    let rescanned_skills = scan_skills_with_origin()?;
    let mut rescanned_skill = find_skill_by_uid(&rescanned_skills, &request.source_uid)?;
    update_skill_from_prepared(&mut rescanned_skill, &prepared);
    let already_latest = prepared
        .remote_tree_hash
        .as_ref()
        .map(|hash| hash == &source_skill.tree_hash)
        .unwrap_or_else(|| prepared.version_label == current_version_label(&source_skill));
    let updated_source = !already_latest;
    let (results, skipped) = apply_targets(
        &rescanned_skill,
        request.target_mode,
        request.target_agent_type,
    )?;
    let source_version_label = current_version_label(&rescanned_skill);

    Ok(ManagedSkillActionResponse {
        source_uid: rescanned_skill.uid,
        source_slug: rescanned_skill.slug,
        source_version_label,
        remote_version_label: prepared.remote_version_label,
        updated_source,
        already_latest,
        results,
        skipped,
    })
}
