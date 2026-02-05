use tokio::time::{interval, Duration};
use tracing::{info, error};
use std::path::PathBuf;
use anyhow::Result;
use crate::moltbook::MoltbookClient;
use crate::config::Config;

pub struct KarmaSyncService {
    moltbook_client: MoltbookClient,
    config_path: PathBuf,
    sync_interval_hours: u64,
}

impl KarmaSyncService {
    pub fn new(
        moltbook_client: MoltbookClient,
        config_path: PathBuf,
        sync_interval_hours: u64,
    ) -> Self {
        Self {
            moltbook_client,
            config_path,
            sync_interval_hours,
        }
    }
    
    pub async fn run(&self) {
        let interval_duration = Duration::from_secs(self.sync_interval_hours * 3600);
        let mut ticker = interval(interval_duration);
        
        info!(
            "🔄 Karma sync service started (interval: {}h)",
            self.sync_interval_hours
        );
        
        // First sync immediately
        if let Err(e) = self.sync_once().await {
            error!("❌ Initial karma sync failed: {}", e);
        }
        
        // Periodic sync
        loop {
            ticker.tick().await;
            
            info!("⏰ Running scheduled karma sync...");
            
            if let Err(e) = self.sync_once().await {
                error!("❌ Karma sync failed: {}", e);
            }
        }
    }
    
    async fn sync_once(&self) -> Result<()> {
        // 1. Fetch latest Karma from Moltbook
        let karma = self.moltbook_client.fetch_karma().await?;
        
        // 2. Load current configuration
        let mut config = Config::load()?;
        
        let karma_before = config.karma;
        
        // 3. Update Karma
        config.karma = karma;
        if let Some(ref mut mb_config) = config.moltbook {
            mb_config.last_karma_sync = chrono::Utc::now();
        }
        
        // 4. Save configuration
        config.save(&self.config_path)?;
        
        if karma != karma_before {
            info!(
                "✅ Karma updated: {} → {} (Δ{})",
                karma_before,
                karma,
                karma as i64 - karma_before as i64
            );
        } else {
            info!("✅ Karma unchanged: {}", karma);
        }
        
        Ok(())
    }
}
