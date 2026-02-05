use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use reqwest;
use tracing::{info, warn, error};

use crate::device::{MacHardwareInfo, DeviceIdentity};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRegistration {
    pub device_id: String,
    pub display_name: String,
}

#[derive(Debug, Serialize)]
struct RegisterRequest {
    fingerprint: String,
    model: String,
    serial_hash: String,
    public_key: String,
}

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    device_id: String,
    display_name: Option<String>,
    message: Option<String>,
}

/// Register device with SACAS backend (device-centric)
pub async fn register_device(
    hw_info: &MacHardwareInfo,
    identity: &DeviceIdentity,
    server_url: &str,
) -> Result<DeviceRegistration> {
    info!("🤖 Registering autonomous device with server...");
    
    let fingerprint = hw_info.generate_fingerprint();
    let public_key = identity.public_key_base64();
    
    // Generate serial_hash (SHA256 of serial number)
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(hw_info.serial_number.as_bytes());
    let serial_hash = format!("{:x}", hasher.finalize());
    
    let request = RegisterRequest {
        fingerprint: fingerprint.clone(),
        model: hw_info.model_identifier.clone(),
        serial_hash,
        public_key,
    };
    
    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/api/devices/register", server_url))
        .json(&request)
        .send()
        .await
        .context("Failed to send registration request")?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Registration failed with status {}: {}", status, error_text);
    }
    
    let reg_response: RegisterResponse = response
        .json()
        .await
        .context("Failed to parse registration response")?;
    
    let display_name = reg_response.display_name
        .unwrap_or_else(|| format!("device_{}", &reg_response.device_id[..8]));
    
    info!("✅ Device registered successfully");
    info!("   Device ID: {}", reg_response.device_id);
    info!("   Display Name: {}", display_name);
    
    if let Some(msg) = reg_response.message {
        info!("   Message: {}", msg);
    }
    
    Ok(DeviceRegistration {
        device_id: reg_response.device_id,
        display_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registration_struct() {
        let reg = DeviceRegistration {
            device_id: "test-id".to_string(),
            display_name: "test-device".to_string(),
        };
        
        assert_eq!(reg.device_id, "test-id");
        assert_eq!(reg.display_name, "test-device");
    }
}
