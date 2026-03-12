use async_openai::{config::OpenAIConfig, Client};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::lookup_host;

const SILICONFLOW_BASE_URL: &str = "https://api.siliconflow.cn/v1";
const DEFAULT_MODEL: &str = "Qwen/Qwen3.5-4B";
const TRANSLATOR_SYSTEM_PROMPT: &str = "You are a professional bilingual translator for Chinese and English.\nTask:\n1) Detect whether the source text is primarily Chinese or English.\n2) If source is primarily English, translate to Simplified Chinese.\n3) If source is primarily Chinese, translate to English.\n4) Preserve meaning, tone, and all markdown structure (headings, lists, links, tables, and code blocks).\n5) Keep technical terms, file paths, URLs, commands, and code identifiers accurate.\n6) Perform an internal self-check for completeness and fidelity before finalizing.\nOutput rules:\n- Output only the final translated text.\n- Do not output analysis, notes, explanations, or labels like \"Translation\" or \"翻译内容\".\n- Do not wrap the whole answer in code fences unless the source itself is entirely a code block.\n- Keep markdown spacing compact: use at most one blank line between paragraphs or sections.";

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranslatorConfig {
    pub api_key: String,
    pub model: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct AppConfigFile {
    #[serde(default)]
    translator: TranslatorConfig,
}

fn config_file_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
    Ok(home.join(".skills-local-manager").join("config.json"))
}

fn logs_dir_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
    Ok(home.join(".skills-local-manager").join("logs"))
}

fn log_file_path() -> Result<PathBuf, String> {
    Ok(logs_dir_path()?.join("translator.log"))
}

fn now_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn excerpt(text: &str, max_len: usize) -> String {
    let compact = text.replace('\n', "\\n").replace('\r', "\\r");
    if compact.len() <= max_len {
        compact
    } else {
        format!("{}...(truncated)", &compact[..max_len])
    }
}

fn unwrap_outer_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") || !trimmed.ends_with("```") {
        return trimmed.to_string();
    }

    let mut lines = trimmed.lines();
    let Some(first_line) = lines.next() else {
        return trimmed.to_string();
    };
    if !first_line.trim_start().starts_with("```") {
        return trimmed.to_string();
    }

    let mut collected: Vec<&str> = lines.collect();
    if collected.is_empty() {
        return String::new();
    }
    if collected.last().map(|line| line.trim()) != Some("```") {
        return trimmed.to_string();
    }
    collected.pop();
    collected.join("\n")
}

fn dedupe_consecutive_blocks(lines: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut current_block: Vec<String> = Vec::new();
    let mut previous_block_key: Option<String> = None;

    let flush_block = |result: &mut Vec<String>,
                       current_block: &mut Vec<String>,
                       previous_block_key: &mut Option<String>| {
        if current_block.is_empty() {
            return;
        }

        let block_key = current_block.join("\n");
        if previous_block_key.as_deref() != Some(block_key.as_str()) {
            if !result.is_empty() {
                result.push(String::new());
            }
            result.extend(current_block.iter().cloned());
            *previous_block_key = Some(block_key);
        }

        current_block.clear();
    };

    for line in lines {
        if line.trim().is_empty() {
            flush_block(&mut result, &mut current_block, &mut previous_block_key);
        } else {
            current_block.push(line.clone());
        }
    }

    flush_block(&mut result, &mut current_block, &mut previous_block_key);
    result
}

fn normalize_translation_output(text: &str) -> String {
    let unwrapped = unwrap_outer_code_fence(text);
    let mut compact_lines: Vec<String> = Vec::new();
    let mut blank_run = 0usize;

    for raw_line in unwrapped.lines() {
        let line = raw_line.trim_end().to_string();
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                compact_lines.push(String::new());
            }
            continue;
        }

        if matches!(
            line.trim(),
            "翻译内容" | "Translation" | "Translated Content"
        ) {
            continue;
        }

        blank_run = 0;
        if compact_lines.last().map(|prev| prev.trim()) == Some(line.trim()) {
            continue;
        }
        compact_lines.push(line);
    }

    dedupe_consecutive_blocks(&compact_lines)
        .join("\n")
        .trim()
        .to_string()
}

fn append_log(level: &str, message: &str) {
    let path = match log_file_path() {
        Ok(p) => p,
        Err(_) => return,
    };
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }

    let mut file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let line = format!("[{}][{}] {}\n", now_timestamp_ms(), level, message);
    let _ = file.write_all(line.as_bytes());
}

pub fn add_translator_log(level: &str, message: &str) {
    append_log(level, message);
}

fn env_hint(name: &str) -> String {
    match env::var(name) {
        Ok(v) if !v.trim().is_empty() => "set".to_string(),
        _ => "unset".to_string(),
    }
}

pub fn get_translator_log_path() -> Result<String, String> {
    let path = log_file_path()?;
    Ok(path.to_string_lossy().to_string())
}

pub fn get_translator_log_tail(max_lines: usize) -> Result<String, String> {
    let path = log_file_path()?;
    if !path.exists() {
        return Ok(String::new());
    }
    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read translator log: {}", e))?;
    let lines: Vec<&str> = content.lines().collect();
    let keep = max_lines.max(1);
    let start = lines.len().saturating_sub(keep);
    Ok(lines[start..].join("\n"))
}

fn load_app_config() -> Result<AppConfigFile, String> {
    let path = config_file_path()?;
    if !path.exists() {
        return Ok(AppConfigFile::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read translator config: {}", e))?;
    serde_json::from_str::<AppConfigFile>(&content)
        .map_err(|e| format!("Failed to parse translator config: {}", e))
}

fn save_app_config(config: &AppConfigFile) -> Result<(), String> {
    let path = config_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    let serialized = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize translator config: {}", e))?;
    fs::write(&path, serialized).map_err(|e| format!("Failed to write translator config: {}", e))
}

fn normalize_model(model: &str) -> String {
    if model.trim().is_empty() {
        DEFAULT_MODEL.to_string()
    } else {
        model.trim().to_string()
    }
}

fn resolve_config() -> Result<TranslatorConfig, String> {
    let config = get_translator_config().unwrap_or_default();
    if config.api_key.trim().is_empty() {
        append_log(
            "ERROR",
            "resolve_config failed: missing SiliconFlow API key",
        );
        return Err("TRANSLATOR_CONFIG_MISSING: SILICONFLOW_API_KEY".to_string());
    }
    let resolved = TranslatorConfig {
        api_key: config.api_key.trim().to_string(),
        model: normalize_model(&config.model),
    };
    append_log(
        "INFO",
        &format!(
            "resolve_config ok: model={}, api_key_len={}",
            resolved.model,
            resolved.api_key.len()
        ),
    );
    Ok(resolved)
}

fn describe_openai_error<E: std::fmt::Debug + std::fmt::Display>(err: &E) -> String {
    format!("err={}, debug={:?}", err, err)
}

fn extract_message_content(value: &Value) -> Option<String> {
    value
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .map(|s| s.to_string())
}

fn extract_delta_content(value: &Value) -> Option<String> {
    value
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("content"))
        .and_then(|content| content.as_str())
        .map(|s| s.to_string())
}

fn build_request_payload(text: &str, model: &str, stream: bool) -> Value {
    json!({
        "model": model,
        "stream": stream,
        "enable_thinking": false,
        "temperature": 0.2,
        "messages": [
            {
                "role": "system",
                "content": TRANSLATOR_SYSTEM_PROMPT
            },
            {
                "role": "user",
                "content": text
            }
        ]
    })
}

fn build_client(config: &TranslatorConfig) -> Result<Client<OpenAIConfig>, String> {
    let http_client = reqwest::Client::builder()
        .user_agent("skillsLocalManager-Multiplatform")
        .http1_only()
        .connect_timeout(Duration::from_secs(12))
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| {
            append_log("ERROR", &format!("http client build failed: {}", e));
            format!("TRANSLATOR_CLIENT_ERROR: {}", e)
        })?;

    let openai_config = OpenAIConfig::new()
        .with_api_key(config.api_key.clone())
        .with_api_base(SILICONFLOW_BASE_URL);

    Ok(Client::with_config(openai_config).with_http_client(http_client))
}

async fn log_request_context(stream: bool, model: &str, text_len: usize) {
    append_log(
        "INFO",
        &format!(
            "request start: stream={}, model={}, text_len={}",
            stream, model, text_len
        ),
    );
    append_log(
        "INFO",
        &format!(
            "proxy env: HTTPS_PROXY={}, HTTP_PROXY={}, ALL_PROXY={}, NO_PROXY={}",
            env_hint("HTTPS_PROXY"),
            env_hint("HTTP_PROXY"),
            env_hint("ALL_PROXY"),
            env_hint("NO_PROXY")
        ),
    );
    match lookup_host(("api.siliconflow.cn", 443)).await {
        Ok(addrs) => {
            let mut ips: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();
            ips.sort();
            ips.dedup();
            append_log(
                "INFO",
                &format!("dns resolved api.siliconflow.cn => {:?}", ips),
            );
        }
        Err(e) => append_log("WARN", &format!("dns lookup failed: {}", e)),
    }
}

pub fn get_translator_config() -> Result<TranslatorConfig, String> {
    let config = load_app_config()?;
    Ok(TranslatorConfig {
        api_key: config.translator.api_key,
        model: normalize_model(&config.translator.model),
    })
}

pub fn set_translator_config(config: TranslatorConfig) -> Result<(), String> {
    let sanitized = TranslatorConfig {
        api_key: config.api_key.trim().to_string(),
        model: normalize_model(&config.model),
    };

    let mut app_config = load_app_config().unwrap_or_default();
    app_config.translator = sanitized;
    save_app_config(&app_config)
}

pub async fn test_connection() -> Result<String, String> {
    append_log("INFO", "test_connection start");
    let config = resolve_config()?;
    log_request_context(false, &config.model, 22).await;
    let client = build_client(&config)?;
    let payload = build_request_payload("Reply with exactly: OK", &config.model, false);

    let response: Value = client.chat().create_byot(payload).await.map_err(|e| {
        let details = describe_openai_error(&e);
        append_log(
            "ERROR",
            &format!("test_connection request failed: {}", details),
        );
        format!("TRANSLATOR_REQUEST_FAILED: {}", details)
    })?;

    append_log("INFO", "request accepted: async-openai create_byot ok");
    let raw = response.to_string();
    append_log("INFO", &format!("test_connection body len={}", raw.len()));

    let value: Value = response;
    let text = extract_message_content(&value).unwrap_or_default();
    if text.trim().is_empty() {
        append_log("ERROR", "test_connection invalid response: empty text");
        return Err("TRANSLATOR_INVALID_RESPONSE".to_string());
    }

    append_log(
        "INFO",
        &format!("test_connection success: model={}", config.model),
    );
    Ok(format!("Connected ({})", config.model))
}

pub async fn translate_text_to_zh(text: &str) -> Result<String, String> {
    append_log(
        "INFO",
        &format!("translate_text_to_zh start: len={}", text.len()),
    );
    if text.trim().is_empty() {
        append_log("WARN", "translate_text_to_zh skipped: empty text");
        return Err("TRANSLATOR_EMPTY_TEXT".to_string());
    }

    let config = resolve_config()?;
    log_request_context(false, &config.model, text.len()).await;
    let client = build_client(&config)?;
    let payload = build_request_payload(text, &config.model, false);

    let response: Value = client.chat().create_byot(payload).await.map_err(|e| {
        let details = describe_openai_error(&e);
        append_log(
            "ERROR",
            &format!("translate_text_to_zh request failed: {}", details),
        );
        format!("TRANSLATOR_REQUEST_FAILED: {}", details)
    })?;

    append_log("INFO", "request accepted: async-openai create_byot ok");
    let raw = response.to_string();
    append_log(
        "INFO",
        &format!("translate_text_to_zh body len={}", raw.len()),
    );

    let value: Value = response;
    let translated = extract_message_content(&value).ok_or_else(|| {
        append_log(
            "ERROR",
            "translate_text_to_zh invalid response: no message content",
        );
        "TRANSLATOR_INVALID_RESPONSE".to_string()
    })?;

    Ok(normalize_translation_output(&translated))
}

pub async fn translate_text_to_zh_stream<F>(text: &str, mut on_chunk: F) -> Result<String, String>
where
    F: FnMut(&str) -> Result<(), String>,
{
    append_log(
        "INFO",
        &format!("translate_text_to_zh_stream start: len={}", text.len()),
    );
    if text.trim().is_empty() {
        append_log("WARN", "translate_text_to_zh_stream skipped: empty text");
        return Err("TRANSLATOR_EMPTY_TEXT".to_string());
    }

    let config = resolve_config()?;
    log_request_context(true, &config.model, text.len()).await;
    let client = build_client(&config)?;
    let payload = build_request_payload(text, &config.model, true);

    let mut stream = client
        .chat()
        .create_stream_byot::<Value, Value>(payload)
        .await
        .map_err(|e| {
            let details = describe_openai_error(&e);
            append_log(
                "ERROR",
                &format!("translate_text_to_zh_stream open failed: {}", details),
            );
            format!("TRANSLATOR_REQUEST_FAILED: {}", details)
        })?;

    append_log(
        "INFO",
        "request accepted: async-openai create_stream_byot ok",
    );

    let mut merged = String::new();
    let mut chunk_count: usize = 0;

    while let Some(item) = stream.next().await {
        let chunk: Value = item.map_err(|e| {
            let details = describe_openai_error(&e);
            append_log("ERROR", &format!("stream read failed: {}", details));
            format!("TRANSLATOR_STREAM_FAILED: {}", details)
        })?;

        if let Some(text_part) = extract_delta_content(&chunk) {
            if text_part.is_empty() {
                append_log("DEBUG", "stream chunk contained empty content");
                continue;
            }
            merged.push_str(&text_part);
            chunk_count += 1;
            append_log(
                "DEBUG",
                &format!(
                    "stream chunk {} accepted: part_len={}, merged_len={}",
                    chunk_count,
                    text_part.len(),
                    merged.len()
                ),
            );
            on_chunk(&text_part)?;
        } else {
            append_log(
                "DEBUG",
                &format!(
                    "stream non-text chunk: {}",
                    excerpt(&chunk.to_string(), 240)
                ),
            );
        }
    }

    if merged.trim().is_empty() {
        append_log(
            "ERROR",
            &format!(
                "stream finished but merged is empty; chunks={}",
                chunk_count
            ),
        );
        return Err("TRANSLATOR_INVALID_RESPONSE".to_string());
    }

    let normalized = normalize_translation_output(&merged);

    append_log(
        "INFO",
        &format!(
            "stream success: chunks={}, merged_len={}, normalized_len={}",
            chunk_count,
            merged.len(),
            normalized.len()
        ),
    );
    Ok(normalized)
}
