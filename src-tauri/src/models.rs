use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum AgentType {
    ClaudeCode,
    Codex,
    GeminiCLI,
    CopilotCLI,
    OpenCode,
    Antigravity,
    Cursor,
    Kiro,
    CodeBuddy,
    OpenClaw,
    Trae,
}

impl AgentType {
    pub fn id(&self) -> String {
        match self {
            AgentType::ClaudeCode => "claude-code".to_string(),
            AgentType::Codex => "codex".to_string(),
            AgentType::GeminiCLI => "gemini-cli".to_string(),
            AgentType::CopilotCLI => "copilot-cli".to_string(),
            AgentType::OpenCode => "opencode".to_string(),
            AgentType::Antigravity => "antigravity".to_string(),
            AgentType::Cursor => "cursor".to_string(),
            AgentType::Kiro => "kiro".to_string(),
            AgentType::CodeBuddy => "codebuddy".to_string(),
            AgentType::OpenClaw => "openclaw".to_string(),
            AgentType::Trae => "trae".to_string(),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            AgentType::ClaudeCode => "Claude Code".to_string(),
            AgentType::Codex => "Codex".to_string(),
            AgentType::GeminiCLI => "Gemini CLI".to_string(),
            AgentType::CopilotCLI => "Copilot CLI".to_string(),
            AgentType::OpenCode => "OpenCode".to_string(),
            AgentType::Antigravity => "Antigravity".to_string(),
            AgentType::Cursor => "Cursor".to_string(),
            AgentType::Kiro => "Kiro".to_string(),
            AgentType::CodeBuddy => "CodeBuddy".to_string(),
            AgentType::OpenClaw => "OpenClaw".to_string(),
            AgentType::Trae => "Trae".to_string(),
        }
    }

    pub fn all_cases() -> Vec<AgentType> {
        vec![
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
        ]
    }

    pub fn config_dir(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        Some(match self {
            AgentType::ClaudeCode => home.join(".claude"),
            AgentType::Codex => home.join(".codex"),
            AgentType::GeminiCLI => home.join(".gemini"),
            AgentType::CopilotCLI => home.join(".copilot"),
            AgentType::OpenCode => home.join(".config").join("opencode"),
            AgentType::Antigravity => home.join(".gemini").join("antigravity"),
            AgentType::Cursor => home.join(".cursor"),
            AgentType::Kiro => home.join(".kiro"),
            AgentType::CodeBuddy => home.join(".codebuddy"),
            AgentType::OpenClaw => home.join(".openclaw"),
            AgentType::Trae => home.join(".trae"),
        })
    }

    pub fn skills_dir(&self) -> Option<PathBuf> {
        self.config_dir().map(|d| d.join("skills"))
    }

    pub fn shared_skills_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".agents").join("skills"))
    }

    pub fn additional_readable_skills_directories(&self) -> Vec<(PathBuf, AgentType)> {
        let mut extras = Vec::new();
        match self {
            AgentType::Codex => {
                if let Some(shared) = Self::shared_skills_dir() {
                    extras.push((shared, AgentType::Codex));
                }
            }
            AgentType::CopilotCLI => {
                if let Some(claude) = AgentType::ClaudeCode.skills_dir() {
                    extras.push((claude, AgentType::ClaudeCode));
                }
            }
            AgentType::OpenCode => {
                if let Some(claude) = AgentType::ClaudeCode.skills_dir() {
                    extras.push((claude, AgentType::ClaudeCode));
                }
                if let Some(shared) = Self::shared_skills_dir() {
                    extras.push((shared, AgentType::Codex));
                }
            }
            AgentType::Cursor => {
                if let Some(claude) = AgentType::ClaudeCode.skills_dir() {
                    extras.push((claude, AgentType::ClaudeCode));
                }
                if let Some(codex) = AgentType::Codex.skills_dir() {
                    extras.push((codex, AgentType::Codex));
                }
            }
            _ => {}
        }
        extras
    }

    pub fn detect_command(&self) -> &str {
        match self {
            AgentType::ClaudeCode => "claude",
            AgentType::Codex => "codex",
            AgentType::GeminiCLI => "gemini",
            AgentType::CopilotCLI => "gh",
            AgentType::OpenCode => "opencode",
            AgentType::Antigravity => "antigravity",
            AgentType::Cursor => "cursor",
            AgentType::Kiro => "kiro",
            AgentType::CodeBuddy => "codebuddy",
            AgentType::OpenClaw => "openclaw",
            AgentType::Trae => "trae",
        }
    }

    pub fn from_id(id: &str) -> Option<AgentType> {
        match id {
            "claude-code" => Some(AgentType::ClaudeCode),
            "codex" => Some(AgentType::Codex),
            "gemini-cli" => Some(AgentType::GeminiCLI),
            "copilot-cli" => Some(AgentType::CopilotCLI),
            "opencode" => Some(AgentType::OpenCode),
            "antigravity" => Some(AgentType::Antigravity),
            "cursor" => Some(AgentType::Cursor),
            "kiro" => Some(AgentType::Kiro),
            "codebuddy" => Some(AgentType::CodeBuddy),
            "openclaw" => Some(AgentType::OpenClaw),
            "trae" => Some(AgentType::Trae),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillScope {
    GlobalShared,
    AgentLocal,
    ProjectLevel,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SkillConflictState {
    None,
    Diverged,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LockEntry {
    pub name: String,
    pub version: String,
    pub resolved: String,
    pub integrity: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstallation {
    pub agent_type: AgentType,
    pub path: PathBuf,
    pub is_symlink: bool,
    pub is_inherited: bool,
    pub inherited_from: Option<AgentType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReadableSkillsDirectory {
    pub path: PathBuf,
    pub source_agent_type: AgentType,
    pub exists: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub uid: String,
    pub slug: String,
    pub namespace: String,
    pub canonical_path: PathBuf,
    pub metadata: SkillMetadata,
    pub markdown_body: String,
    pub scope: SkillScope,
    pub tree_hash: String,
    pub conflict_state: SkillConflictState,
    pub sync_group_id: Option<String>,
    pub installations: Vec<SkillInstallation>,
    pub lock_entry: Option<LockEntry>,
    pub has_update: bool,
    pub remote_tree_hash: Option<String>,
    pub remote_commit_hash: Option<String>,
    pub local_commit_hash: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub agent_type: AgentType,
    pub is_installed: bool,
    pub config_directory: Option<PathBuf>,
    pub skills_directory: Option<PathBuf>,
    pub readable_skills_directories: Vec<ReadableSkillsDirectory>,
    pub config_directory_exists: bool,
    pub skills_directory_exists: bool,
    pub skill_count: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeBootstrapSkill {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub recommended: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeBootstrapCatalog {
    pub target_dir: String,
    pub target_dir_exists: bool,
    pub can_create_target_dir: bool,
    pub claude_cli_installed: bool,
    pub recommended_skills: Vec<ClaudeBootstrapSkill>,
    pub optional_skills: Vec<ClaudeBootstrapSkill>,
    pub existing_skill_slugs: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeBootstrapRequest {
    pub skill_slugs: Vec<String>,
    pub create_target_dir_if_missing: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeBootstrapSkippedSkill {
    pub slug: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeBootstrapResult {
    pub target_dir: String,
    pub created_target_dir: bool,
    pub installed: Vec<String>,
    pub skipped: Vec<ClaudeBootstrapSkippedSkill>,
    pub source_repo: String,
    pub source_ref: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncMappingUpsertResponse {
    pub group_id: String,
    pub source_uid: String,
    pub target_uids: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SyncFileChangeType {
    Added,
    Modified,
    Deleted,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncFileChange {
    pub path: String,
    pub change_type: SyncFileChangeType,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SyncDecisionType {
    NoChange,
    SourceChanged,
    TargetChanged,
    Diverged,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncTargetPreview {
    pub target_uid: String,
    pub target_hash: String,
    pub baseline_hash: Option<String>,
    pub decision: SyncDecisionType,
    pub changes: Vec<SyncFileChange>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreviewResponse {
    pub source_uid: String,
    pub source_hash: String,
    pub targets: Vec<SyncTargetPreview>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppliedTarget {
    pub target_uid: String,
    pub previous_hash: Option<String>,
    pub new_hash: String,
    pub changed_files: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApplySyncResponse {
    pub snapshot_id: String,
    pub source_uid: String,
    pub source_hash: String,
    pub updated_targets: Vec<AppliedTarget>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RollbackSyncResponse {
    pub snapshot_id: String,
    pub restored_targets: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CoverTargetAction {
    Updated,
    Installed,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoverTargetResult {
    pub target_agent_type: AgentType,
    pub target_uid: String,
    pub target_path: String,
    pub action: CoverTargetAction,
    pub previous_hash: Option<String>,
    pub new_hash: String,
    pub history_entry_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoverSkippedTarget {
    pub target_agent_type: AgentType,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoverSkillResponse {
    pub operation_id: String,
    pub source_uid: String,
    pub source_hash: String,
    pub source_version_label: String,
    pub results: Vec<CoverTargetResult>,
    pub skipped: Vec<CoverSkippedTarget>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillCoverHistoryEntry {
    pub entry_id: String,
    pub skill_slug: String,
    pub target_agent_type: AgentType,
    pub target_uid: String,
    pub source_uid: String,
    pub source_namespace: String,
    pub source_version_label: String,
    pub source_hash: String,
    pub previous_hash: Option<String>,
    pub applied_at: u64,
    pub rolled_back_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RollbackSkillCoverResponse {
    pub entry_id: String,
    pub target_agent_type: AgentType,
    pub target_uid: String,
    pub restored_hash: Option<String>,
    pub restored: bool,
}
