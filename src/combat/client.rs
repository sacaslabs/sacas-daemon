// sacas-daemon/src/combat/client.rs
// Combat HTTP client for battle and defense configuration

use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefenseConfig {
    pub l1: u64,
    pub l2: u64,
    pub l3: u64,
}

#[derive(Debug, Deserialize)]
pub struct DefenseStatus {
    pub defense: DefenseConfig,
    pub total_combat_points: u64,
   pub last_configured: Option<String>,
    pub cooldown: CooldownInfo,
}

#[derive(Debug, Deserialize)]
pub struct CooldownInfo {
    pub active: bool,
    pub ends_at: Option<String>,
    pub remaining_seconds: u64,
}

#[derive(Debug, Deserialize)]
pub struct BattleResult {
    pub success: bool,
    pub battle_id: String,
    pub outcome: String, // "PARASITIZED" or "REPELLED"
    pub layers: BattleLayers,
    pub loot: LootInfo,
}

#[derive(Debug, Deserialize)]
pub struct BattleLayers {
    pub l1: LayerResult,
    pub l2: LayerResult,
    pub l3: LayerResult,
}

#[derive(Debug, Deserialize)]
pub struct LayerResult {
    pub success: bool,
    pub attack: u64,
    pub defense: u64,
    pub threshold: Option<u64>,
    pub success_rate: Option<f64>,
    pub roll: Option<f64>,
    pub miss: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct LootInfo {
    pub entropy_looted: String,
    pub attacker_entropy_after: String,
    pub defender_entropy_after: String,
}

#[derive(Debug, Deserialize)]
pub struct BattleSimulation {
    pub probabilities: Probabilities,
    pub expected_loot: String,
}

#[derive(Debug, Deserialize)]
pub struct Probabilities {
    pub l1_win: f64,
    pub l2_success: f64,
    pub l3_parasitize: f64,
}

pub struct CombatClient {
    client: Client,
    api_base: String,
    device_id: String,
    private_key: ed25519_dalek::SigningKey,
}

impl CombatClient {
    pub fn new(
        api_base: String,
        device_id: String,
        private_key: ed25519_dalek::SigningKey,
    ) -> Self {
        Self {
            client: Client::new(),
            api_base,
            device_id,
            private_key,
        }
    }

    /// Configure defense allocation (L1/L2/L3)
    pub async fn configure_defense(&self, config: DefenseConfig) -> Result<serde_json::Value> {
        let url = format!("{}/api/game/defense/configure", self.api_base);
        
        let body = serde_json::json!({
            "l1": config.l1,
            "l2": config.l2,
            "l3": config.l3
        });

        let response = self.signed_post(&url, &body).await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("Defense configuration failed ({}): {}", status, text);
        }

        let result: serde_json::Value = response.json().await?;
        info!("Defense configured: L1={}, L2={}, L3={}", config.l1, config.l2, config.l3);
        
        Ok(result)
    }

    /// Get current defense status
    pub async fn get_defense_status(&self) -> Result<DefenseStatus> {
        let url = format!("{}/api/game/defense/status", self.api_base);
        
        let response = self.signed_get(&url).await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("Failed to get defense status ({}): {}", status, text);
        }

        let status: DefenseStatus = response.json().await?;
        Ok(status)
    }

    /// Attack a target device
    pub async fn attack(&self, target_id: &str) -> Result<BattleResult> {
        let url = format!("{}/api/game/battle/attack", self.api_base);
        
        let body = serde_json::json!({
            "target_id": target_id
        });

        let response = self.signed_post(&url, &body).await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("Attack failed ({}): {}", status, text);
        }

        let result: BattleResult = response.json().await?;
        info!("Battle {} - Outcome: {}", result.battle_id, result.outcome);
        
        Ok(result)
    }

    /// Simulate battle without executing
    pub async fn simulate_battle(&self, target_id: &str) -> Result<BattleSimulation> {
        let url = format!("{}/api/game/battle/simulate", self.api_base);
        
        let body = serde_json::json!({
            "target_id": target_id
        });

        let response = self.signed_post(&url, &body).await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("Simulation failed ({}): {}", status, text);
        }

        let result: BattleSimulation = response.json().await?;
        Ok(result)
    }

    /// Sign and send POST request with Ed25519 signature
    async fn signed_post(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        use ed25519_dalek::Signer;
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        let nonce = uuid::Uuid::new_v4().to_string();

        let body_str = body.to_string();
        let message = format!("POST|{}|{}|{}|{}", 
            url.split("/api/").nth(1).unwrap_or(""),
            body_str,
            timestamp,
            nonce
        );

        let signature = self.private_key.sign(message.as_bytes());
        let sig_hex = hex::encode(signature.to_bytes());

        let response = self.client
            .post(url)
            .header("X-Device-ID", &self.device_id)
            .header("X-Signature", sig_hex)
            .header("X-Timestamp", timestamp.to_string())
            .header("X-Nonce", nonce)
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .await?;

        Ok(response)
    }

    /// Sign and send GET request with Ed25519 signature
    async fn signed_get(&self, url: &str) -> Result<reqwest::Response> {
        use ed25519_dalek::Signer;
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        let nonce = uuid::Uuid::new_v4().to_string();

        let message = format!("GET|{}||{}|{}", 
            url.split("/api/").nth(1).unwrap_or(""),
            timestamp,
            nonce
        );

        let signature = self.private_key.sign(message.as_bytes());
        let sig_hex = hex::encode(signature.to_bytes());

        let response = self.client
            .get(url)
            .header("X-Device-ID", &self.device_id)
            .header("X-Signature", sig_hex)
            .header("X-Timestamp", timestamp.to_string())
            .header("X-Nonce", nonce)
            .send()
            .await?;

        Ok(response)
    }
}
