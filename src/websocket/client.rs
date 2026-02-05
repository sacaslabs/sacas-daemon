// sacas-daemon/src/websocket/client.rs
// WebSocket client with Ed25519 authentication

use anyhow::{anyhow, Context, Result};
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

#[derive(Debug, Serialize)]
struct AuthMessage {
    r#type: String,
    device_id: String,
    timestamp: i64,
    nonce: String,
    signature: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ServerMessage {
    #[serde(rename = "AUTH_SUCCESS")]
    AuthSuccess {
        device_id: String,
        subscriptions: Vec<String>,
        server_time: i64,
    },
    #[serde(rename = "PING")]
    Ping { timestamp: i64 },
    #[serde(rename = "PONG")]
    Pong { timestamp: i64 },
    
    // Device-specific events
    #[serde(rename = "battle_result")]
    BattleResult {
        channel: String,
        data: BattleResultData,
    },
    #[serde(rename = "battle_attacked")]
    BattleAttacked {
        channel: String,
        data: BattleAttackedData,
    },
    
    // Global broadcast events
    #[serde(rename = "epic_battle")]
    EpicBattle {
        channel: String,
        broadcast_channel: String,
        data: EpicBattleData,
    },
    
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct BattleResultData {
    battle_id: String,
    outcome: String,
    entropy_looted: String,
}

#[derive(Debug, Deserialize)]
struct BattleAttackedData {
    battle_id: String,
    attacker_id: String,
    outcome: String,
    entropy_lost: String,
    parasitized: bool,
}

#[derive(Debug, Deserialize)]
struct EpicBattleData {
    battle_id: String,
    attacker_id: String,
    defender_id: String,
    outcome: String,
    entropy_looted: String,
}

pub struct WebSocketClient {
    server_url: String,
    device_id: String,
    signing_key: SigningKey,
}

impl WebSocketClient {
    pub fn new(server_url: String, device_id: String, private_key_base64: &str) -> Result<Self> {
        // Decode private key
        let private_key_bytes = base64::decode(private_key_base64)
            .context("Failed to decode private key")?;
        
        let signing_key = SigningKey::from_bytes(
            private_key_bytes.as_slice().try_into()
                .map_err(|_| anyhow!("Invalid private key length"))?
        );

        Ok(Self {
            server_url,
            device_id,
            signing_key,
        })
    }

    /// Create Ed25519 signature for WebSocket authentication
    fn create_auth_signature(&self) -> Result<(i64, String, String)> {
        let timestamp = chrono::Utc::now().timestamp();
        let nonce = uuid::Uuid::new_v4().to_string();

        // Canonical message: WS|/ws|AUTH|timestamp|nonce
        let canonical = format!("WS|/ws|AUTH|{}|{}", timestamp, nonce);
        
        debug!("📝 Canonical message: {}", canonical);

        // Sign with Ed25519
        let signature = self.signing_key.sign(canonical.as_bytes());
        let signature_base64 = base64::encode(signature.to_bytes());

        Ok((timestamp, nonce, signature_base64))
    }

    /// Connect and authenticate to WebSocket server
    pub async fn connect_and_listen(&self) -> Result<()> {
        let ws_url = self.server_url.replace("https://", "wss://").replace("http://", "ws://");
        let full_url = format!("{}/ws", ws_url);

        info!("📡 Connecting to WebSocket: {}", full_url);

        // Connect
        let (ws_stream, _) = connect_async(&full_url)
            .await
            .context("Failed to connect to WebSocket server")?;

        info!("✅ WebSocket connected");

        let (mut write, mut read) = ws_stream.split();

        // Create and send authentication message
        let (timestamp, nonce, signature) = self.create_auth_signature()?;
        
        let auth_msg = AuthMessage {
            r#type: "AUTH".to_string(),
            device_id: self.device_id.clone(),
            timestamp,
            nonce,
            signature,
        };

        let auth_json = serde_json::to_string(&auth_msg)?;
        write.send(Message::Text(auth_json)).await?;

        info!("🔐 Sent authentication");

        // Wait for auth response
        let mut authenticated = false;

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<ServerMessage>(&text) {
                        Ok(ServerMessage::AuthSuccess { device_id, subscriptions, .. }) => {
                            info!("✅ Authenticated as: {}", device_id);
                            info!("📢 Auto-subscribed to: {:?}", subscriptions);
                            authenticated = true;
                            break;
                        }
                        Ok(_) => {
                            warn!("Unexpected message before auth: {}", text);
                        }
                        Err(e) => {
                            error!("Failed to parse auth response: {}", e);
                            return Err(anyhow!("Authentication failed"));
                        }
                    }
                }
                Ok(Message::Close(frame)) => {
                    error!("Connection closed before auth: {:?}", frame);
                    return Err(anyhow!("Connection closed before authentication"));
                }
                Err(e) => {
                    error!("WebSocket error during auth: {}", e);
                    return Err(e.into());
                }
                _ => {}
            }
        }

        if !authenticated {
            return Err(anyhow!("Authentication timeout"));
        }

        // Listen for events
        info!("👂 Listening for events...");

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    self.handle_message(&text).await;
                }
                Ok(Message::Ping(_)) => {
                    // Auto-handled by tungstenite
                }
                Ok(Message::Close(frame)) => {
                    warn!("📴 Connection closed: {:?}", frame);
                    break;
                }
                Err(e) => {
                    error!("❌ WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        info!("WebSocket connection ended");
        Ok(())
    }

    /// Handle incoming WebSocket messages
    async fn handle_message(&self, text: &str) {
        match serde_json::from_str::<ServerMessage>(text) {
            Ok(msg) => {
                match msg {
                    ServerMessage::BattleResult { data, .. } => {
                        info!("⚔️  BATTLE RESULT: {} - Looted: {} entropy",
                            data.outcome, data.entropy_looted);
                        
                        // macOS notification support (future feature)
                        self.show_notification(
                            "Battle Result",
                            &format!("You {} and looted {} entropy!", 
                                data.outcome.to_lowercase(), data.entropy_looted)
                        );
                    }
                    
                    ServerMessage::BattleAttacked { data, .. } => {
                        warn!("🚨 UNDER ATTACK by {}! Lost: {} entropy (Parasitized: {})",
                            data.attacker_id, data.entropy_lost, data.parasitized);
                        
                        // macOS notification support (future feature)
                        self.show_notification(
                            "⚠️ Under Attack!",
                            &format!("Attacker: {}\nLost: {} entropy\nResult: {}",
                                &data.attacker_id[..8], data.entropy_lost, data.outcome)
                        );
                    }
                    
                    ServerMessage::EpicBattle { data, .. } => {
                        info!("🏆 EPIC BATTLE: {} vs {} - {} entropy looted!",
                            &data.attacker_id[..8], &data.defender_id[..8], data.entropy_looted);
                    }
                    
                    ServerMessage::Ping { .. } => {
                        // Respond to ping
                        debug!("📡 Received PING");
                    }
                    
                    ServerMessage::Pong { .. } => {
                        debug!("📡 Received PONG");
                    }
                    
                    ServerMessage::AuthSuccess { .. } => {
                        // Already handled
                    }
                    
                    ServerMessage::Unknown => {
                        debug!("❓ Unknown message: {}", text);
                    }
                }
            }
            Err(e) => {
                error!("Failed to parse message: {} - Error: {}", text, e);
            }
        }
    }

    /// Show macOS notification
    #[cfg(target_os = "macos")]
    fn show_notification(&self, title: &str, body: &str) {
        use std::process::Command;
        
        let script = format!(
            r#"display notification "{}" with title "SACAS Daemon" subtitle "{}""#,
            body.replace('"', r#"\""#),
            title.replace('"', r#"\""#)
        );
        
        let _ = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .spawn();
    }

    #[cfg(not(target_os = "macos"))]
    fn show_notification(&self, _title: &str, _body: &str) {
        // No-op for non-macOS
    }
}
