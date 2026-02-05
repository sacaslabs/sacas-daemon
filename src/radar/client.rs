// sacas-daemon/src/radar/client.rs
// Radar HTTP client for network scanning

use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, debug};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarTarget {
    pub device_id: String,
    pub visibility: String, // "LOCKED" or "FUZZY"
    pub distance: f64,
    pub karma: Option<u64>,
    pub karma_range: Option<[u64; 2]>,
    pub defense: Option<DefenseInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefenseInfo {
    pub l1: u64,
    pub l2: u64,
    pub l3: u64,
    pub total: u64,
}

#[derive(Debug, Deserialize)]
pub struct RadarScanResult {
    pub success: bool,
    pub scan_id: String,
    pub cost: u64,
    pub targets: Vec<RadarTarget>,
    pub summary: ScanSummary,
    pub entropy_remaining: u64,
}

#[derive(Debug, Deserialize)]
pub struct ScanSummary {
    pub total: usize,
    pub locked: usize,
    pub fuzzy: usize,
}

pub struct RadarClient {
    client: Client,
    api_base: String,
    device_id: String,
    private_key: ed25519_dalek::SigningKey,
}

impl RadarClient {
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

    /// Scan network for targets
    pub async fn scan(&self, max_distance: Option<u64>) -> Result<RadarScanResult> {
        let url = format!("{}/api/game/radar/scan", self.api_base);
        
        let body = serde_json::json!({
            "max_distance": max_distance.unwrap_or(5000),
            "cost_omega": 10
        });

        let response = self.signed_post(&url, &body).await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("Radar scan failed ({}): {}", status, text);
        }

        let result: RadarScanResult = response.json().await?;
        info!("Radar scan complete: {} targets ({} locked, {} fuzzy)",
            result.summary.total,
            result.summary.locked,
            result.summary.fuzzy
        );
        
        Ok(result)
    }

    /// Get only LOCKED targets (attackable)
    pub fn get_locked_targets(scan: &RadarScanResult) -> Vec<&RadarTarget> {
        scan.targets
            .iter()
            .filter(|t| t.visibility == "LOCKED")
            .collect()
    }

    /// Find best target (lowest total defense)
    pub fn find_weakest_target<'a>(targets: &'a [&'a RadarTarget]) -> Option<&'a RadarTarget> {
        targets
            .iter()
            .filter_map(|t| {
                t.defense.as_ref().map(|d| (*t, d.total))
            })
            .min_by_key(|(_, total)| *total)
            .map(|(target, _)| target)
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
}
