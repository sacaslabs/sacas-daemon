use reqwest;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
pub struct MoltbookOwner {
    pub x_handle: Option<String>,
    pub x_name: Option<String>,
    pub x_avatar: Option<String>,
    pub x_bio: Option<String>,
    pub x_follower_count: Option<u32>,
    pub x_following_count: Option<u32>,
    pub x_verified: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct MoltbookAgent {
    pub name: String,
    pub description: Option<String>,
    pub karma: u64,
    pub follower_count: Option<u32>,
    pub following_count: Option<u32>,
    pub is_claimed: bool,
    pub is_active: bool,
    pub created_at: Option<String>,
    pub last_active: Option<String>,
    pub owner: Option<MoltbookOwner>,
}

#[derive(Debug, Deserialize)]
pub struct MoltbookProfileResponse {
    pub success: bool,
    pub agent: MoltbookAgent,
}

#[derive(Clone)]
pub struct MoltbookClient {
    api_url: String,
    api_key: String,
    agent_name: String,
    client: reqwest::Client,
}

impl MoltbookClient {
    pub fn new(api_url: String, api_key: String, agent_name: String) -> Self {
        Self {
            api_url,
            api_key,
            agent_name,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }
    
    /// Fetch Karma value from Moltbook API
    pub async fn fetch_karma(&self) -> Result<u64> {
        let url = format!(
            "{}/api/v1/agents/profile?name={}",
            self.api_url,
            urlencoding::encode(&self.agent_name)
        );
        
        info!("📡 Fetching Karma from Moltbook for agent: {}", self.agent_name);
        
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .context("Failed to send request to Moltbook API")?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Moltbook API error {}: {}", status, error_text);
        }
        
        let profile: MoltbookProfileResponse = response
            .json()
            .await
            .context("Failed to parse Moltbook API response")?;
        
        if !profile.success {
            anyhow::bail!("Moltbook API returned success: false");
        }
        
        if !profile.agent.is_claimed {
            warn!("⚠️  Agent '{}' is not claimed (karma: {})", profile.agent.name, profile.agent.karma);
        }
        
        if !profile.agent.is_active {
            warn!("⚠️  Agent '{}' is not active (karma: {})", profile.agent.name, profile.agent.karma);
        }
        
        info!(
            "✅ Karma fetched from Moltbook: {} (followers: {}, active: {})",
            profile.agent.karma,
            profile.agent.follower_count.unwrap_or(0),
            profile.agent.is_active
        );
        
        Ok(profile.agent.karma)
    }
}
