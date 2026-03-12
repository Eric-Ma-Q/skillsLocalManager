use crate::models::{AgentType, Skill, SkillConflictState, SkillMetadata, SkillScope};
use crate::services::{md_parser, symlink, tree_hash};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub fn scan_all() -> Result<Vec<Skill>, String> {
    scan_all_v2()
}

pub fn scan_all_v2() -> Result<Vec<Skill>, String> {
    let mut result: Vec<Skill> = Vec::new();

    if let Some(shared_dir) = AgentType::shared_skills_dir() {
        result.extend(scan_directory(
            &shared_dir,
            SkillScope::GlobalShared,
            "shared:agents",
        ));
    }

    for agent_type in AgentType::all_cases() {
        if let Some(agent_skills_dir) = agent_type.skills_dir() {
            let namespace = format!("agent:{}", agent_type.id());
            result.extend(scan_directory(
                &agent_skills_dir,
                SkillScope::AgentLocal,
                &namespace,
            ));
        }
    }

    let mut slug_to_hashes: HashMap<String, HashSet<String>> = HashMap::new();
    for skill in &result {
        slug_to_hashes
            .entry(skill.slug.clone())
            .or_default()
            .insert(skill.tree_hash.clone());
    }

    for skill in &mut result {
        let is_diverged = slug_to_hashes
            .get(&skill.slug)
            .map(|hashes| hashes.len() > 1)
            .unwrap_or(false);
        skill.conflict_state = if is_diverged {
            SkillConflictState::Diverged
        } else {
            SkillConflictState::None
        };
    }

    result.sort_by(|a, b| {
        a.metadata
            .name
            .to_lowercase()
            .cmp(&b.metadata.name.to_lowercase())
            .then_with(|| a.namespace.cmp(&b.namespace))
            .then_with(|| a.slug.cmp(&b.slug))
    });
    Ok(result)
}

fn scan_directory(directory: &Path, scope: SkillScope, namespace: &str) -> Vec<Skill> {
    if !directory.exists() || !directory.is_dir() {
        return Vec::new();
    }

    let mut skills = Vec::new();
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() || symlink::is_link(&path) {
                if let Some(skill) = parse_skill_directory(&path, scope, namespace) {
                    skills.push(skill);
                }
            }
        }
    }
    skills
}

fn parse_skill_directory(path: &Path, scope: SkillScope, namespace: &str) -> Option<Skill> {
    let slug = path.file_name()?.to_str()?.to_string();
    let uid = format!("{}:{}", namespace, slug);

    let canonical_path = if symlink::is_link(path) {
        symlink::resolve_link(path)
    } else {
        path.to_path_buf()
    };

    let skill_md_path = canonical_path.join("SKILL.md");
    if !skill_md_path.exists() {
        return None;
    }

    let (metadata, markdown_body) = match fs::read_to_string(&skill_md_path) {
        Ok(content) => md_parser::parse(&content).unwrap_or_else(|_| {
            (
                SkillMetadata {
                    name: slug.clone(),
                    description: String::new(),
                    version: None,
                    author: None,
                    homepage: None,
                    repository: None,
                },
                String::new(),
            )
        }),
        Err(_) => (
            SkillMetadata {
                name: slug.clone(),
                description: String::new(),
                version: None,
                author: None,
                homepage: None,
                repository: None,
            },
            String::new(),
        ),
    };

    let tree_hash = tree_hash::compute_tree_hash(&canonical_path).ok()?;
    let installations = symlink::find_installations(&slug, &canonical_path);

    Some(Skill {
        id: uid.clone(),
        uid,
        slug,
        namespace: namespace.to_string(),
        canonical_path,
        metadata,
        markdown_body,
        scope,
        tree_hash,
        conflict_state: SkillConflictState::None,
        sync_group_id: None,
        installations,
        lock_entry: None,
        has_update: false,
        remote_tree_hash: None,
        remote_commit_hash: None,
        local_commit_hash: None,
    })
}
