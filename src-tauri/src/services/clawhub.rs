use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Raw API response structs matching the actual ClawHub API format
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct RawClawHubSkill {
    slug: String,
    display_name: String,
    summary: String,
    #[serde(default)]
    tags: Option<HashMap<String, String>>,
    #[serde(default)]
    stats: Option<RawStats>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct RawStats {
    #[serde(default)]
    downloads: i32,
    #[serde(default)]
    stars: i32,
}

/// Flattened struct sent to the frontend
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClawHubSkill {
    pub slug: String,
    pub display_name: String,
    pub summary: String,
    pub latest_version: Option<String>,
    pub downloads: i32,
    pub stars: i32,
}

impl From<RawClawHubSkill> for ClawHubSkill {
    fn from(raw: RawClawHubSkill) -> Self {
        let latest_version = raw.tags.as_ref().and_then(|t| t.get("latest").cloned());
        let (downloads, stars) = match raw.stats {
            Some(s) => (s.downloads, s.stars),
            None => (0, 0),
        };
        ClawHubSkill {
            slug: raw.slug,
            display_name: raw.display_name,
            summary: raw.summary,
            latest_version,
            downloads,
            stars,
        }
    }
}

pub struct ClawHubService {
    client: Client,
    base_url: String,
}

impl ClawHubService {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("skillsLocalManager-Multiplatform")
                .build()
                .unwrap(),
            base_url: "https://clawhub.ai".to_string(),
        }
    }

    pub async fn fetch_skills(&self) -> Result<Vec<ClawHubSkill>, String> {
        let url = format!("{}/api/v1/skills?limit=50", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        #[derive(Deserialize)]
        struct Resp {
            items: Vec<RawClawHubSkill>,
        }

        let data: Resp = resp.json().await.map_err(|e| e.to_string())?;
        Ok(data.items.into_iter().map(ClawHubSkill::from).collect())
    }

    pub async fn search_skills(&self, query: &str) -> Result<Vec<ClawHubSkill>, String> {
        let url = format!("{}/api/v1/search?q={}&limit=50", self.base_url, query);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        #[derive(Deserialize)]
        struct Resp {
            results: Vec<RawClawHubSkill>,
        }

        let data: Resp = resp.json().await.map_err(|e| e.to_string())?;
        Ok(data.results.into_iter().map(ClawHubSkill::from).collect())
    }

    pub async fn fetch_skill_content(&self, slug: &str) -> Result<String, String> {
        let url = format!(
            "{}/api/v1/skills/{}/file?path=SKILL.md",
            self.base_url, slug
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let content = resp.text().await.map_err(|e| e.to_string())?;
        Ok(content)
    }
}
