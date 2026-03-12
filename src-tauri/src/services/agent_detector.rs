use crate::models::{Agent, AgentType, ReadableSkillsDirectory};
use std::fs;
use which::which;

/// Detects all supported agents on the system
pub fn detect_all() -> Vec<Agent> {
    let agent_types = [
        AgentType::ClaudeCode,
        AgentType::Codex,
        AgentType::GeminiCLI,
        AgentType::CopilotCLI,
        AgentType::OpenCode,
        AgentType::Antigravity,
        AgentType::Cursor,
        AgentType::Kiro,
        AgentType::CodeBuddy,
        AgentType::OpenClaw,
        AgentType::Trae,
    ];

    agent_types.iter().map(|&at| detect(at)).collect()
}

/// Detects a specific agent on the system
pub fn detect(agent_type: AgentType) -> Agent {
    // config_dir() and skills_dir() already return absolute Option<PathBuf>
    let config_dir_abs = agent_type.config_dir();
    let skills_dir_abs = agent_type.skills_dir();

    // Check if CLI command exists
    let is_installed = which(agent_type.detect_command()).is_ok();

    // Check if directories exist
    let config_directory_exists = config_dir_abs.as_ref().map(|d| d.exists()).unwrap_or(false);
    let skills_directory_exists = skills_dir_abs.as_ref().map(|d| d.exists()).unwrap_or(false);
    let readable_skills_directories = agent_type
        .additional_readable_skills_directories()
        .into_iter()
        .map(|(path, source_agent_type)| ReadableSkillsDirectory {
            exists: path.exists(),
            path,
            source_agent_type,
        })
        .collect();

    // Count skills (subdirectories containing SKILL.md)
    let mut skill_count = 0;
    if let Some(ref sd) = skills_dir_abs {
        if let Ok(entries) = fs::read_dir(sd) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("SKILL.md").exists() {
                    skill_count += 1;
                }
            }
        }
    }

    Agent {
        agent_type,
        is_installed,
        config_directory: config_dir_abs,
        skills_directory: skills_dir_abs,
        readable_skills_directories,
        config_directory_exists,
        skills_directory_exists,
        skill_count,
    }
}
