use anyhow::Result;
use tracing::{info, error, warn};
use tracing_subscriber;

mod config;
mod daemon;
mod network;
mod mining;
mod grpc;
mod state;
mod types;
mod sync;  // New: periodic sync

// New modules
mod device;
mod moltbook;
mod karma_sync;
mod combat;  // Combat system
mod radar;   // Radar scanning
mod websocket;  // WebSocket client

use crate::config::Config;
use crate::daemon::OmniDaemon;
use crate::device::{MacHardwareInfo, MacValidator, VMDetector, register_device};
use crate::moltbook::MoltbookClient;
use crate::karma_sync::KarmaSyncService;
use crate::sync::start_sync_loop;
use crate::websocket::WebSocketClient;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("sacas_daemon=debug,info")
        .init();

    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║                  SACAS DAEMON v1.0.0                  ║");
    println!("║           The Entropy Protocol - Mac Edition          ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    info!("🚀 SACAS Daemon starting...");
    
    // ========================================
    // Phase 1: Hardware Validation
    // ========================================
    info!("🔍 Phase 1: Hardware Validation");
    
    let hw_info = match MacHardwareInfo::collect() {
        Ok(info) => {
            info!("  ✓ Hardware UUID: {}", info.hardware_uuid);
            info!("  ✓ Serial Number: {}", info.serial_number);
            info!("  ✓ Model: {}", info.model_identifier);
            info!("  ✓ CPU: {}", info.cpu_brand);
            info!("  ✓ Device Fingerprint: {}", info.generate_fingerprint());
            info
        }
        Err(e) => {
            error!("❌ Failed to collect hardware information:");
            error!("   {}", e);
            error!("\n🚫 Cannot start SACAS without proper hardware identification.");
            std::process::exit(1);
        }
    };
    
    // ========================================
    // Phase 2: Mac Model Validation
    // ========================================
    if let Err(e) = MacValidator::validate(&hw_info) {
        error!("\n{}", e);
        error!("\n🚫 SACAS only runs on genuine Apple Silicon Mac computers.");
        error!("   Supported models: Mac mini, MacBook Pro, MacBook Air, iMac, Mac Studio, Mac Pro");
        std::process::exit(1);
    }
    
    let friendly_name = MacValidator::get_friendly_name(&hw_info.model_identifier);
    info!("✅ Mac validation passed: {}", friendly_name);
    
    // ========================================
    // Phase 3: Virtual Machine Detection
    // ========================================
    match VMDetector::detect() {
        Ok(warnings) if !warnings.is_empty() => {
            error!("\n❌ Virtual machine detected:");
            for warning in &warnings {
                error!("   - {}", warning);
            }
            error!("\n🚫 SACAS does not support virtual machines.");
            error!("   Please run SACAS on a real Mac computer.");
            std::process::exit(1);
        }
        Ok(_) => {
            info!("✅ VM detection passed - Running on real hardware");
        }
        Err(e) => {
            warn!("⚠️  VM detection error: {}", e);
            warn!("   Proceeding with caution...");
        }
    }
    
    // ========================================
    // Phase 4: Configuration Load/Create
    // ========================================
    info!("\n📝 Phase 2: Configuration");
    
    let config_path = Config::config_path();
    let config = if config_path.exists() {
        // Load existing configuration
        info!("Loading existing configuration...");
        let cfg = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                error!("❌ Failed to load configuration: {}", e);
                std::process::exit(1);
            }
        };
        
        // Verify device binding
        let current_fp = hw_info.generate_fingerprint();
        if cfg.device.device_fingerprint != current_fp {
            error!("\n❌ DEVICE FINGERPRINT MISMATCH!");
            error!("   Expected: {}", cfg.device.device_fingerprint);
            error!("   Actual:   {}", current_fp);
            error!("\n🚫 This configuration is bound to a different device.");
            error!("   Original device: {} ({})", cfg.device.model_identifier, cfg.device.serial_number);
            error!("   Current device:  {} ({})", hw_info.model_identifier, hw_info.serial_number);
            error!("\n   If you've replaced your hardware, please delete: {:?}", config_path);
            std::process::exit(1);
        }
        
        info!("✅ Device binding verified");
        info!("   Bound to: {} ({})", cfg.device.model_identifier, cfg.device.serial_number);
        info!("   First seen: {}", cfg.device.first_seen.format("%Y-%m-%d %H:%M:%S"));
        
        cfg
    } else {
        // First run: create new configuration
        info!("🆕 First time setup - creating configuration");
        
        // Generate or load device identity
        let identity_path = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".sacas")
            .join("device.key");
        
        let identity = match device::DeviceIdentity::load_or_generate(&identity_path) {
            Ok(i) => i,
            Err(e) => {
                error!("❌ Failed to generate device identity: {}", e);
                std::process::exit(1);
            }
        };
        
        info!("✅ Device identity ready");
        info!("   Public Key: {}", identity.public_key_base64());
        
        match Config::create_with_device(hw_info.clone(), identity).await {
            Ok(c) => c,
            Err(e) => {
                error!("❌ Failed to create configuration: {}", e);
                std::process::exit(1);
            }
        }
    };
    
    info!("\n✓ Configuration loaded");
    info!("  Device ID: {:?}", config.device_id);
    info!("  Display Name: {:?}", config.display_name);
    
    // Check if moltbook config exists before accessing
    if let Some(ref moltbook_cfg) = config.moltbook {
        info!("  Last Karma Sync: {}", moltbook_cfg.last_karma_sync.format("%Y-%m-%d %H:%M:%S"));
    }
    
    // ========================================
    // Phase 4.5: Device Registration
    // ========================================
    let mut config = config;  // Make mutable for registration update
    
    if config.device_id.is_none() {
        // Note: Device is now auto-registered through create_with_device
        // No manual registration step needed here anymore
        info!("\n📱 Device auto-registered during first run");
    } else {
        info!("\n✅ Device already registered");
        info!("   Device ID: {}", config.device_id.as_ref().unwrap());
        if let Some(name) = &config.display_name {
            info!("   Display Name: {}", name);
        }
    }
    
    // ========================================
    // Phase 5: Karma Synchronization Service
    // ========================================
    info!("\n🔄 Phase 3: Karma Synchronization");
    
    // Start Karma Sync Service if Moltbook is configured
    if let Some(ref mb_config) = config.moltbook {
        let moltbook_client = MoltbookClient::new(
            mb_config.api_url.clone(),
            mb_config.api_key.clone(),
            mb_config.agent_name.clone(),
        );
        let karma_sync = KarmaSyncService::new(
            moltbook_client,
            Config::config_path(),
            mb_config.sync_interval_hours,
        );
        
        info!("✓ Karma sync enabled (interval: {}h)", mb_config.sync_interval_hours);
        
        // Start Karma sync service (background task)
        tokio::spawn(async move {
            karma_sync.run().await;
        });
        
        info!("✓ Karma sync task started");
    } else {
        info!("⊘ Karma sync disabled (no Moltbook config)");
    }
  
    // ========================================
    // Phase 6: Start Game Daemon
    // ========================================
    info!("\n🎮 Phase 4: Game Daemon");
    
    let daemon = OmniDaemon::new(config.clone()).await?;
    info!("✓ Daemon initialized");
    
    // ========================================
    // Phase 7: Start Device Sync Loop
    // ========================================
    if config.device_id.is_some() {
        info!("\n🔄 Starting device sync loop...");
        let sync_config = config.clone();
        let sync_state = daemon.get_state().get_handle();
        
        // Load identity for signed sync
        let identity_path = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".sacas")
            .join("device.key");
        
        let sync_identity = device::DeviceIdentity::load_or_generate(&identity_path)?;
        
        tokio::spawn(async move {
            if let Err(e) = start_sync_loop(sync_config, sync_state, sync_identity).await {
                error!("❌ Sync loop error: {}", e);
            }
        });
        
        info!("✓ Signed sync loop started (5-minute intervals)");
    } else {
        warn!("⚠️  Sync loop disabled - Device not registered");
    }

    info!("\n🚀 All systems ready - Starting game loops...\n");
    daemon.run().await?;

    Ok(())
}

/// Display prominent claim instructions for unclaimed devices
fn display_unclaimed_device_notice(claim_code: &str, device_id: &str) {
    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║  ⚠️  DEVICE NOT CLAIMED - ACTION REQUIRED            ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!("\n  Your device is mining but not linked to an account!");
    println!("  \n  📋 Claim Code: {}\n", claim_code);
    println!("  To claim your rewards:");
    println!("  1. Visit: https://sacas.ai/claim");
    println!("  2. Log in or create an account");
    println!("  3. Enter the code above\n");
    println!("  Device ID: {}\n", device_id);
}
