use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use tracing::{info, warn};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    // Device-centric: Device IS the player
    pub device_id: Option<String>,
    pub display_name: Option<String>,
    
    pub karma: u64,
    pub server_url: String,
    pub grpc_port: u16,
    
    // New: Moltbook configuration (optional)
    pub moltbook: Option<MoltbookConfig>,
    
    // New: Device binding configuration
    pub device: DeviceConfig,
    
    pub network: NetworkConfig,
    pub mining: MiningConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MoltbookConfig {
    pub api_url: String,
    pub api_key: String,         // Bearer token for API authentication
    pub agent_name: String,       // Moltbook agent name (e.g., "ClawdClawderberg")
    pub last_karma_sync: DateTime<Utc>,
    pub sync_interval_hours: u64,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceConfig {
    pub hardware_uuid: String,
    pub serial_number: String,
    pub model_identifier: String,
    pub device_fingerprint: String,
    pub is_verified: bool,
    pub first_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    pub probe_interval_secs: u64,
    pub anchors: Vec<Anchor>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Anchor {
    pub id: String,
    pub ip: String,
    pub region: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MiningConfig {
    pub tick_interval_secs: u64,
    pub base_multiplier: f64,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();
        
        if !config_path.exists() {
            anyhow::bail!(
                "Configuration file not found at: {:?}\n\n\
                 This appears to be the first run. Please use create_with_device() instead.",
                config_path
            );
        }

        let config_str = fs::read_to_string(&config_path)
            .context("Failed to read config file")?;
        
        let config: Config = toml::from_str(&config_str)
            .context("Failed to parse config file")?;

        Ok(config)
    }

    pub fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".sacas")
            .join("config.toml")
    }
    
    pub fn save(&self, path: &Path) -> Result<()> {
        let config_str = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        
        fs::write(path, config_str)
            .context("Failed to write config file")?;
        
        Ok(())
    }

    /// Create configuration on first run (device-centric)
    pub async fn create_with_device(
        hw_info: crate::device::MacHardwareInfo,
        identity: crate::device::DeviceIdentity,
    ) -> Result<Self> {
        use crate::moltbook::MoltbookClient;
        use crate::device::register_device;
        
        info!("Creating new configuration for autonomous device...");
        
        // 1. Register device with SACAS backend
        let server_url = std::env::var("SACAS_API_URL")
            .unwrap_or_else(|_| "https://sacas.ai".to_string());
        
        let registration = register_device(&hw_info, &identity, &server_url).await?;
        
        info!("✅ Device registered successfully");
        info!("   Device ID: {}", registration.device_id);
        info!("   Display Name: {}", registration.display_name);
        
        // Moltbook integration is available but abstracted from user interaction
        // Default karma is 1000 for all new installations
        let bot_token = String::new(); // No user prompt
        
        if bot_token.is_empty() {
            info!("Using default Karma: 1000");
        }
        
        // 3. Fetch initial Karma from Moltbook (if provided)
        let karma = if !bot_token.is_empty() {
            let moltbook_client = MoltbookClient::new(
                "https://api.moltbook.com".to_string(),
                bot_token.clone(),
                "sacas-agent".to_string(),
            );
            
            match moltbook_client.fetch_karma().await {
                Ok(k) => {
                    info!("✅ Fetched Karma from Moltbook: {}", k);
                    k
                }
                Err(e) => {
                    warn!("⚠️  Failed to fetch Karma from Moltbook: {}", e);
                    warn!("Using default Karma: 1000");
                    1000
                }
            }
        } else {
            1000
        };
        
        // 4. Create configuration
        let config = Config {
            // Device-centric fields
            device_id: Some(registration.device_id.clone()),
            display_name: Some(registration.display_name.clone()),
            
            karma,
            server_url,
            grpc_port: 50051,
            
            // Moltbook is optional
            moltbook: if !bot_token.is_empty() {
               let agent_name = bot_token.split('@').next().unwrap_or("sacas-agent").to_string();
            
                Some(MoltbookConfig {
                    api_url: "https://www.moltbook.com".to_string(),
                    api_key: bot_token.clone(),
                    agent_name,
                    last_karma_sync: Utc::now(),
                    sync_interval_hours: 1,
                })
            } else {
                None
            },
            
            device: DeviceConfig {
                hardware_uuid: hw_info.hardware_uuid.clone(),
                serial_number: hw_info.serial_number.clone(),
                model_identifier: hw_info.model_identifier.clone(),
                device_fingerprint: hw_info.generate_fingerprint(),
                is_verified: true,
                first_seen: Utc::now(),
            },
            
            network: NetworkConfig {
                probe_interval_secs: 60,
                anchors: Self::default_anchors(),
            },
            
            mining: MiningConfig {
                tick_interval_secs: 5,
                base_multiplier: 0.5,
            },
        };
        
        // 5. Save configuration
        let config_path = Self::config_path();
        let config_dir = config_path.parent().unwrap();
        fs::create_dir_all(config_dir)?;
        
        config.save(&config_path)?;
        
        info!("✅ Configuration saved to: {:?}", Self::config_path());
        info!("  Device ID: {}", registration.device_id);
        info!("  Display Name: {}", registration.display_name);
        info!("  Karma: {}", karma);
        info!("  Device: {}", hw_info.model_identifier);
        
        Ok(config)
    }

    fn default_anchors() -> Vec<Anchor> {
        vec![
            Anchor {
                id: "cloudflare-1".to_string(),
                ip: "1.1.1.1".to_string(),
                region: "global".to_string(),
            },
            Anchor {
                id: "cloudflare-2".to_string(),
                ip: "1.0.0.1".to_string(),
                region: "global".to_string(),
            },
            Anchor {
                id: "google-dns-1".to_string(),
                ip: "8.8.8.8".to_string(),
                region: "global".to_string(),
            },
            Anchor {
                id: "google-dns-2".to_string(),
                ip: "8.8.4.4".to_string(),
                region: "global".to_string(),
            },
            Anchor {
                id: "quad9".to_string(),
                ip: "9.9.9.9".to_string(),
                region: "global".to_string(),
            },
        ]
    }
}
