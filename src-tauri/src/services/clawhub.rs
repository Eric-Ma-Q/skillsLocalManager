use crate::models::{
    RegistryOwner, RegistrySkillDetail, RegistrySkillsRequest, RegistrySkillsResponse,
    RegistrySortMode,
};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

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
    #[serde(default)]
    created_at: Option<u64>,
    #[serde(default)]
    updated_at: Option<u64>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct RawStats {
    #[serde(default)]
    downloads: i32,
    #[serde(default)]
    stars: i32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct RawOwner {
    #[serde(default)]
    handle: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct RawDetailResponse {
    skill: RawClawHubSkill,
    #[serde(default)]
    latest_version: Option<RawSkillVersion>,
    #[serde(default)]
    owner: Option<RawOwner>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct RawSkillVersion {
    version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClawHubSkill {
    pub slug: String,
    pub display_name: String,
    pub summary: String,
    pub latest_version: Option<String>,
    pub downloads: i32,
    pub stars: i32,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

impl From<RawClawHubSkill> for ClawHubSkill {
    fn from(raw: RawClawHubSkill) -> Self {
        let latest_version = raw
            .tags
            .as_ref()
            .and_then(|tags| tags.get("latest").cloned());
        let (downloads, stars) = match raw.stats {
            Some(stats) => (stats.downloads, stats.stars),
            None => (0, 0),
        };
        ClawHubSkill {
            slug: raw.slug,
            display_name: raw.display_name,
            summary: raw.summary,
            latest_version,
            downloads,
            stars,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        }
    }
}

pub struct ClawHubService {
    client: Client,
    site_url: String,
    api_base_url: String,
}

impl ClawHubService {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("SkillLocalManager-Multiplatform")
                .build()
                .unwrap(),
            site_url: "https://clawhub.ai".to_string(),
            api_base_url: "https://wry-manatee-359.convex.site".to_string(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.site_url
    }

    fn api_base_url(&self) -> &str {
        &self.api_base_url
    }

    async fn send_with_retry<F>(&self, build: F) -> Result<Response, String>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        const MAX_ATTEMPTS: usize = 3;
        const DEFAULT_RETRY_SECONDS: u64 = 2;

        for attempt in 0..MAX_ATTEMPTS {
            let response = build().send().await.map_err(|error| error.to_string())?;
            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                return Ok(response);
            }

            let retry_after_seconds = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(DEFAULT_RETRY_SECONDS);

            if attempt + 1 >= MAX_ATTEMPTS {
                return Err(format!(
                    "Registry rate limited by ClawHub. Please retry in {} seconds.",
                    retry_after_seconds
                ));
            }

            sleep(Duration::from_secs(retry_after_seconds)).await;
        }

        Err("Registry rate limited by ClawHub".to_string())
    }

    pub async fn fetch_skills(
        &self,
        request: RegistrySkillsRequest,
    ) -> Result<RegistrySkillsResponse<ClawHubSkill>, String> {
        let limit = request.limit.unwrap_or(50).clamp(1, 200);
        let query = request.query.unwrap_or_default();
        if query.trim().is_empty() {
            self.fetch_browse_skills(
                request.sort.unwrap_or(RegistrySortMode::Updated),
                request.cursor,
                limit,
            )
            .await
        } else {
            self.search_skills(
                &query,
                request.sort.unwrap_or(RegistrySortMode::Updated),
                limit,
            )
            .await
        }
    }

    async fn fetch_browse_skills(
        &self,
        sort: RegistrySortMode,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<RegistrySkillsResponse<ClawHubSkill>, String> {
        #[derive(Deserialize)]
        struct ResponsePayload {
            items: Vec<RawClawHubSkill>,
            #[serde(default)]
            next_cursor: Option<String>,
        }

        let sort_value = match sort {
            RegistrySortMode::Updated => "updated",
            RegistrySortMode::Downloads => "downloads",
            RegistrySortMode::Name => "updated",
        };

        let mut params = vec![
            ("limit".to_string(), limit.to_string()),
            ("sort".to_string(), sort_value.to_string()),
        ];
        if let Some(cursor) = cursor {
            params.push(("cursor".to_string(), cursor));
        }
        let url = format!("{}/api/v1/skills", self.api_base_url());
        params.push(("nonSuspicious".to_string(), "true".to_string()));
        let response = self
            .send_with_retry(|| self.client.get(&url).query(&params))
            .await?;
        if !response.status().is_success() {
            return Err(format!("Registry request failed: {}", response.status()));
        }
        let payload = response
            .json::<ResponsePayload>()
            .await
            .map_err(|error| error.to_string())?;
        let mut items: Vec<ClawHubSkill> =
            payload.items.into_iter().map(ClawHubSkill::from).collect();
        self.sort_client_side(&mut items, sort);
        Ok(RegistrySkillsResponse {
            items,
            next_cursor: payload.next_cursor,
        })
    }

    async fn search_skills(
        &self,
        query: &str,
        sort: RegistrySortMode,
        limit: u32,
    ) -> Result<RegistrySkillsResponse<ClawHubSkill>, String> {
        #[derive(Deserialize)]
        struct ResponsePayload {
            results: Vec<RawClawHubSkill>,
        }

        let url = format!("{}/api/v1/search", self.api_base_url());
        let params = vec![
            ("q".to_string(), query.to_string()),
            ("limit".to_string(), limit.to_string()),
            ("nonSuspicious".to_string(), "true".to_string()),
        ];
        let response = self
            .send_with_retry(|| self.client.get(&url).query(&params))
            .await?;
        if !response.status().is_success() {
            return Err(format!("Registry search failed: {}", response.status()));
        }
        let payload = response
            .json::<ResponsePayload>()
            .await
            .map_err(|error| error.to_string())?;
        let mut items: Vec<ClawHubSkill> = payload
            .results
            .into_iter()
            .map(ClawHubSkill::from)
            .collect();
        self.sort_client_side(&mut items, sort);
        Ok(RegistrySkillsResponse {
            items,
            next_cursor: None,
        })
    }

    fn sort_client_side(&self, items: &mut [ClawHubSkill], sort: RegistrySortMode) {
        match sort {
            RegistrySortMode::Updated => items.sort_by(|left, right| {
                right
                    .updated_at
                    .unwrap_or_default()
                    .cmp(&left.updated_at.unwrap_or_default())
                    .then_with(|| {
                        left.display_name
                            .to_lowercase()
                            .cmp(&right.display_name.to_lowercase())
                    })
            }),
            RegistrySortMode::Downloads => items.sort_by(|left, right| {
                right.downloads.cmp(&left.downloads).then_with(|| {
                    left.display_name
                        .to_lowercase()
                        .cmp(&right.display_name.to_lowercase())
                })
            }),
            RegistrySortMode::Name => items.sort_by(|left, right| {
                left.display_name
                    .to_lowercase()
                    .cmp(&right.display_name.to_lowercase())
            }),
        }
    }

    pub async fn fetch_skill_detail(&self, slug: &str) -> Result<RegistrySkillDetail, String> {
        let url = format!("{}/api/v1/skills/{}", self.api_base_url(), slug);
        let detail_response = self.send_with_retry(|| self.client.get(&url)).await?;
        if !detail_response.status().is_success() {
            return Err(format!(
                "Failed to load registry skill detail: {}",
                detail_response.status()
            ));
        }
        let payload = detail_response
            .json::<RawDetailResponse>()
            .await
            .map_err(|error| error.to_string())?;
        let markdown_body = self.fetch_skill_content(slug).await.unwrap_or_default();
        let skill = ClawHubSkill::from(payload.skill);
        Ok(RegistrySkillDetail {
            slug: skill.slug,
            display_name: skill.display_name,
            summary: skill.summary,
            markdown_body,
            latest_version: payload
                .latest_version
                .map(|version| version.version)
                .or(skill.latest_version),
            downloads: skill.downloads,
            stars: skill.stars,
            created_at: skill.created_at,
            updated_at: skill.updated_at,
            owner: payload.owner.map(|owner| RegistryOwner {
                handle: owner.handle,
                display_name: owner.display_name,
            }),
        })
    }

    pub async fn fetch_skill_content(&self, slug: &str) -> Result<String, String> {
        let url = format!("{}/api/v1/skills/{}/file", self.api_base_url(), slug);
        let response = self
            .send_with_retry(|| self.client.get(&url).query(&[("path", "SKILL.md")]))
            .await?;
        if !response.status().is_success() {
            return Err(format!(
                "Failed to load registry skill content: {}",
                response.status()
            ));
        }
        response.text().await.map_err(|error| error.to_string())
    }

    pub async fn download_skill_zip(
        &self,
        slug: &str,
        version_or_tag: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let url = format!("{}/api/v1/download", self.api_base_url());
        let mut params = vec![("slug".to_string(), slug.to_string())];
        if let Some(value) = version_or_tag {
            if value.eq_ignore_ascii_case("latest") {
                params.push(("tag".to_string(), "latest".to_string()));
            } else {
                params.push(("version".to_string(), value.to_string()));
            }
        }
        let response = self
            .send_with_retry(|| self.client.get(&url).query(&params))
            .await?;
        if !response.status().is_success() {
            return Err(format!(
                "Failed to download registry skill: {}",
                response.status()
            ));
        }
        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|error| error.to_string())
    }
}
