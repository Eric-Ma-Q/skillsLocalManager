#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use skills_local_manager_lib::models::{
    Agent, AgentType, ApplySyncResponse, ClaudeBootstrapCatalog, ClaudeBootstrapRequest,
    ClaudeBootstrapResult, CoverSkillResponse, ManagedSkillActionResponse,
    ManagedSkillUpdateRequest, RegistrySkillDetail, RegistrySkillsRequest, RegistrySkillsResponse,
    RollbackSkillCoverResponse, RollbackSyncResponse, Skill, SkillCoverHistoryEntry,
    SyncMappingUpsertResponse, SyncPreviewResponse,
};
use skills_local_manager_lib::services::clawhub::ClawHubSkill;
use skills_local_manager_lib::services::translator::TranslatorConfig;
use skills_local_manager_lib::services::{
    agent_detector, claude_bootstrap, clawhub, cover_history, managed_skills, scanner, symlink,
    sync, translator,
};
use std::path::PathBuf;
use std::process::Command;
use tauri::Emitter;
use tauri::Manager;

#[tauri::command]
async fn get_agents() -> Result<Vec<Agent>, String> {
    Ok(agent_detector::detect_all())
}

#[tauri::command]
async fn get_skills() -> Result<Vec<Skill>, String> {
    get_skills_v2().await
}

#[tauri::command]
async fn get_skills_v2() -> Result<Vec<Skill>, String> {
    let mut skills = scanner::scan_all_v2()?;
    managed_skills::attach_origin_metadata(&mut skills);
    managed_skills::hydrate_remote_updates(&mut skills).await;
    sync::attach_sync_group_ids(&mut skills)?;
    Ok(skills)
}

#[tauri::command]
async fn toggle_skill(
    skill_id: String,
    agent_type: AgentType,
    install: bool,
) -> Result<(), String> {
    let skills = scanner::scan_all_v2()?;
    let mut skill = skills
        .iter()
        .find(|s| s.uid == skill_id || s.id == skill_id);

    if skill.is_none() {
        let matches: Vec<&Skill> = skills.iter().filter(|s| s.slug == skill_id).collect();
        if matches.len() == 1 {
            skill = matches.first().copied();
        } else if matches.len() > 1 {
            return Err(format!(
                "Skill slug '{}' is ambiguous. Please use skill uid instead.",
                skill_id
            ));
        }
    }

    let skill = skill.ok_or_else(|| "Skill not found".to_string())?;

    let agent_skills_dir = agent_type
        .skills_dir()
        .ok_or_else(|| "Could not determine agent skills directory".to_string())?;

    let target_path = agent_skills_dir.join(&skill.slug);

    if install {
        symlink::create_link(&skill.canonical_path, &target_path)
    } else {
        cover_history::record_uninstall_history(skill, agent_type)?;
        symlink::remove_link(&target_path)
    }
}

#[tauri::command]
async fn get_registry_skills(
    request: RegistrySkillsRequest,
) -> Result<RegistrySkillsResponse<ClawHubSkill>, String> {
    let service = clawhub::ClawHubService::new();
    service.fetch_skills(request).await
}

#[tauri::command]
async fn get_registry_skill_detail(slug: String) -> Result<RegistrySkillDetail, String> {
    let service = clawhub::ClawHubService::new();
    service.fetch_skill_detail(&slug).await
}

#[tauri::command]
async fn get_claude_bootstrap_catalog() -> Result<ClaudeBootstrapCatalog, String> {
    claude_bootstrap::get_catalog()
}

#[tauri::command]
async fn install_claude_bootstrap_skills(
    request: ClaudeBootstrapRequest,
) -> Result<ClaudeBootstrapResult, String> {
    claude_bootstrap::install_skills(request)
}

#[tauri::command]
async fn install_registry_skill(
    request: skills_local_manager_lib::models::RegistrySkillInstallRequest,
) -> Result<ManagedSkillActionResponse, String> {
    managed_skills::install_registry_skill(request).await
}

#[tauri::command]
async fn update_managed_skill(
    request: ManagedSkillUpdateRequest,
) -> Result<ManagedSkillActionResponse, String> {
    managed_skills::update_managed_skill(request).await
}

#[tauri::command]
async fn browse_registry() -> Result<Vec<ClawHubSkill>, String> {
    let service = clawhub::ClawHubService::new();
    Ok(service
        .fetch_skills(RegistrySkillsRequest {
            query: None,
            sort: None,
            cursor: None,
            limit: Some(50),
        })
        .await?
        .items)
}

#[tauri::command]
async fn search_registry(query: String) -> Result<Vec<ClawHubSkill>, String> {
    let service = clawhub::ClawHubService::new();
    Ok(service
        .fetch_skills(RegistrySkillsRequest {
            query: Some(query),
            sort: None,
            cursor: None,
            limit: Some(50),
        })
        .await?
        .items)
}

#[tauri::command]
async fn upsert_sync_mapping(
    source_uid: String,
    target_uids: Vec<String>,
    group_name: Option<String>,
) -> Result<SyncMappingUpsertResponse, String> {
    sync::upsert_sync_mapping(source_uid, target_uids, group_name)
}

#[tauri::command]
async fn preview_sync(
    source_uid: String,
    target_uids: Vec<String>,
) -> Result<SyncPreviewResponse, String> {
    sync::preview_sync(source_uid, target_uids)
}

#[tauri::command]
async fn apply_sync(
    source_uid: String,
    target_uids: Vec<String>,
    expected_source_hash: String,
) -> Result<ApplySyncResponse, String> {
    sync::apply_sync(source_uid, target_uids, expected_source_hash)
}

#[tauri::command]
async fn rollback_sync(snapshot_id: String) -> Result<RollbackSyncResponse, String> {
    sync::rollback_sync(snapshot_id)
}

#[tauri::command]
async fn cover_skill_to_agent(
    source_uid: String,
    target_agent_type: AgentType,
) -> Result<CoverSkillResponse, String> {
    cover_history::cover_skill_to_agent(source_uid, target_agent_type)
}

#[tauri::command]
async fn cover_skill_to_all_available_agents(
    source_uid: String,
) -> Result<CoverSkillResponse, String> {
    cover_history::cover_skill_to_all_available_agents(source_uid)
}

#[tauri::command]
async fn list_skill_cover_history(
    skill_slug: String,
    target_agent_type: AgentType,
) -> Result<Vec<SkillCoverHistoryEntry>, String> {
    cover_history::list_skill_cover_history(skill_slug, target_agent_type)
}

#[tauri::command]
async fn rollback_skill_cover_entry(
    entry_id: String,
) -> Result<RollbackSkillCoverResponse, String> {
    cover_history::rollback_skill_cover_entry(entry_id)
}

#[tauri::command]
async fn translate_text_to_zh(text: String) -> Result<String, String> {
    translator::translate_text_to_zh(&text).await
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TranslatorStreamEvent {
    session_id: String,
    stage: String,
    chunk: Option<String>,
    result: Option<String>,
    error: Option<String>,
}

fn emit_translator_event(
    window: &tauri::Window,
    session_id: &str,
    stage: &str,
    chunk: Option<String>,
    result: Option<String>,
    error: Option<String>,
) -> Result<(), String> {
    translator::add_translator_log(
        "DEBUG",
        &format!(
            "emit_translator_event: session={}, stage={}, chunk_len={}, result_len={}, has_error={}",
            session_id,
            stage,
            chunk.as_ref().map(|s| s.len()).unwrap_or(0),
            result.as_ref().map(|s| s.len()).unwrap_or(0),
            error.is_some()
        ),
    );
    window
        .emit(
            "translator-stream",
            TranslatorStreamEvent {
                session_id: session_id.to_string(),
                stage: stage.to_string(),
                chunk,
                result,
                error,
            },
        )
        .map_err(|e| format!("TRANSLATOR_EVENT_EMIT_FAILED: {}", e))
}

#[tauri::command]
async fn translate_text_to_zh_stream(
    window: tauri::Window,
    session_id: String,
    text: String,
) -> Result<(), String> {
    emit_translator_event(&window, &session_id, "start", None, None, None)?;

    let window_for_stream = window.clone();
    let session_for_stream = session_id.clone();
    match translator::translate_text_to_zh_stream(&text, move |piece| {
        emit_translator_event(
            &window_for_stream,
            &session_for_stream,
            "chunk",
            Some(piece.to_string()),
            None,
            None,
        )
    })
    .await
    {
        Ok(full_text) => {
            emit_translator_event(&window, &session_id, "done", None, Some(full_text), None)?;
            Ok(())
        }
        Err(err) => {
            let _ =
                emit_translator_event(&window, &session_id, "error", None, None, Some(err.clone()));
            Err(err)
        }
    }
}

#[tauri::command]
async fn test_translator_connection() -> Result<String, String> {
    translator::test_connection().await
}

#[tauri::command]
async fn get_translator_config() -> Result<TranslatorConfig, String> {
    translator::get_translator_config()
}

#[tauri::command]
async fn save_translator_config(config: TranslatorConfig) -> Result<(), String> {
    translator::set_translator_config(config)
}

#[tauri::command]
async fn get_translator_log_path() -> Result<String, String> {
    translator::get_translator_log_path()
}

#[tauri::command]
async fn get_translator_log_tail(max_lines: usize) -> Result<String, String> {
    translator::get_translator_log_tail(max_lines)
}

#[tauri::command]
async fn open_local_path(path: String) -> Result<(), String> {
    let mut target = PathBuf::from(&path);
    while !target.exists() {
        target = target
            .parent()
            .map(|parent| parent.to_path_buf())
            .ok_or_else(|| format!("Path does not exist: {}", path))?;
    }

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg("start").arg("").arg(&target);
        cmd
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = Command::new("open");
        cmd.arg(&target);
        cmd
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(&target);
        cmd
    };

    command
        .spawn()
        .map_err(|e| format!("Failed to open path: {}", e))?;

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            // Set macOS vibrancy
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                apply_vibrancy(
                    &window,
                    NSVisualEffectMaterial::UnderWindowBackground,
                    None,
                    None,
                )
                .expect("Unsupported platform! 'apply_vibrancy' is only supported on macOS");
            }

            // Set Windows Mica effect
            #[cfg(target_os = "windows")]
            {
                use window_vibrancy::apply_mica;
                apply_mica(&window, None)
                    .expect("Unsupported platform! 'apply_mica' is only supported on Windows");
            }

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_agents,
            get_skills,
            get_skills_v2,
            toggle_skill,
            get_registry_skills,
            get_registry_skill_detail,
            browse_registry,
            get_claude_bootstrap_catalog,
            install_claude_bootstrap_skills,
            install_registry_skill,
            update_managed_skill,
            search_registry,
            upsert_sync_mapping,
            preview_sync,
            apply_sync,
            rollback_sync,
            cover_skill_to_agent,
            cover_skill_to_all_available_agents,
            list_skill_cover_history,
            rollback_skill_cover_entry,
            translate_text_to_zh,
            translate_text_to_zh_stream,
            test_translator_connection,
            get_translator_config,
            save_translator_config,
            get_translator_log_path,
            get_translator_log_tail,
            open_local_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

