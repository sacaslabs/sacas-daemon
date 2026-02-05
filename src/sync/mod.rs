pub mod signed_sync;

use anyhow::{Result, Context};
use std::time::Duration;
use std::sync::Arc;
use tokio::time;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::config::Config;
use crate::types::GameState;
use crate::device::DeviceIdentity;
use signed_sync::{SignedSyncRequest, SyncResponse};

/// Start periodic sync loop with Ed25519 signatures
pub async fn start_sync_loop(
    config: Config,
    state: Arc<RwLock<GameState>>,
    identity: DeviceIdentity,
) -> Result<()> {
    let device_id = match &config.device_id {
        Some(id) => id.clone(),
        None => {
            error!("No device_id in config. Cannot start sync loop.");
            anyhow::bail!("Device not registered");
        }
    };

    info!("Starting signed sync loop (every 5 minutes)");
    
    let mut interval = time::interval(Duration::from_secs(300)); // 5 minutes
    let mut last_synced_entropy: i64 = 0;
    let start_time = std::time::Instant::now();

    loop {
        interval.tick().await;

        // Get current entropy from state
        let current_entropy = {
            let state_lock = state.read().await;
            let entropy = state_lock.player.entropy as i64;
            info!("🔍 Sync check: current_entropy={}, last_synced={}", entropy, last_synced_entropy);
            entropy
        };

        let entropy_delta = current_entropy - last_synced_entropy;
        
        info!("📊 Entropy delta: {} Ω", entropy_delta);

        if entropy_delta == 0 {
            warn!("⚠️  No new entropy to sync (current: {}, last: {})", 
                  current_entropy, last_synced_entropy);
            continue;
        }

        // Calculate uptime
        let uptime_seconds = start_time.elapsed().as_secs();

        // Create signed sync request
        let signed_request = SignedSyncRequest::create_and_sign(
            &device_id,
            entropy_delta,
            1.0, // Network quality (currently fixed at 1.0)
            uptime_seconds,
            &identity,
        );

        // Attempt sync
        match sync_to_server(&config.server_url, signed_request).await {
            Ok(response) => {
                info!("✅ Synced +{} Ω to server (signed)", entropy_delta);
                info!("   Device total: {} Ω", response.device_entropy);
                
                // Update karma from server (in case it changed)
                {
                    let state_mgr = crate::state::StateManager {
                        state: state.clone(),
                    };
                    state_mgr.update_karma(response.device_karma as u64).await;
                }
                info!("   Karma updated: {}", response.device_karma);
                
                if response.managed {
                    info!("   📊 Device linked to human account");
                } else {
                    info!("   🤖 Device operating autonomously");
                }

                // Warn if anomaly detected
                if let Some(warning) = &response.warning {
                    warn!("⚠️  Anomaly detected (confidence: {:.1}%)", warning.confidence * 100.0);
                    for reason in &warning.reasons {
                        warn!("   - {}", reason);
                    }
                }

                last_synced_entropy = current_entropy;
            }
            Err(e) => {
                warn!("❌ Sync failed: {}. Will retry in 5 minutes", e);
            }
        }
    }
}

/// Sync device data to server with Ed25519 signature
async fn sync_to_server(
    server_url: &str,
    signed_request: SignedSyncRequest,
) -> Result<SyncResponse> {
    let client = reqwest::Client::new();
    
    // Build request with signature headers
    // CRITICAL: Use body_string() to send the EXACT JSON used for signing
    // Using .json() would re-serialize and could change format (1.0 -> 1)
    let mut request_builder = client
        .post(&format!("{}/api/devices/{}/sync", server_url, signed_request.device_id))
        .header("content-type", "application/json")
        .body(signed_request.body_string().to_string());

    // Add signature headers
    for (key, value) in signed_request.headers() {
        request_builder = request_builder.header(key, value);
    }

    let response = request_builder
        .send()
        .await
        .context("Failed to send signed sync request")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Sync failed with status {}: {}", status, error_text);
    }

    let sync_response: SyncResponse = response
        .json()
        .await
        .context("Failed to parse sync response")?;

    Ok(sync_response)
}
