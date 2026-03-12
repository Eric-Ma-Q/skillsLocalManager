use crate::models::SkillMetadata;

/// Parses a SKILL.md file: splits YAML frontmatter (between `---` delimiters) from the markdown body.
/// Uses serde_yaml directly instead of gray_matter, which has API incompatibilities at v0.1.
pub fn parse(content: &str) -> Result<(SkillMetadata, String), String> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err("No YAML frontmatter found".to_string());
    }

    // Find the closing `---` delimiter (skip the opening one)
    let after_open = &trimmed[3..];
    let close_pos = after_open
        .find("\n---")
        .ok_or_else(|| "No closing --- for frontmatter".to_string())?;

    let yaml_str = &after_open[..close_pos];
    // Everything after the closing `---\n`
    let body_start = close_pos + 4; // "\n---".len()
    let body = after_open[body_start..]
        .trim_start_matches('\n')
        .to_string();

    let metadata: SkillMetadata = serde_yaml::from_str(yaml_str)
        .map_err(|e| format!("Failed to parse YAML metadata: {}", e))?;

    Ok((metadata, body))
}

pub fn serialize(metadata: &SkillMetadata, markdown_body: &str) -> Result<String, String> {
    let yaml = serde_yaml::to_string(metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
    Ok(format!("---\n{}---\n\n{}", yaml.trim(), markdown_body))
}
