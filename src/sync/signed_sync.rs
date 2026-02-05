use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::Utc;

use crate::device::DeviceIdentity;

#[derive(Debug, Serialize)]
pub struct SignedSyncRequest {
    pub device_id: String,
    pub entropy_delta: i64,
    pub network_quality: f64,
    pub uptime_seconds: i64, // Changed from u64
    
    // Signature fields
    pub timestamp: i64,        // Unix timestamp (seconds)
    pub nonce: String,         // UUID v4
    pub signature: String,     // Base64 Ed25519 signature
    #[serde(skip)] // Don't serialize this field directly, it's for internal caching
    body_json: String,  // Cached JSON string for signature consistency
}

impl SignedSyncRequest {
    /// Create and sign a sync request
    pub fn create_and_sign(
        device_id: &str,
        entropy_delta: i64,
        network_quality: f64,
        uptime_seconds: u64,
        identity: &DeviceIdentity,
    ) -> Self {
        // Generate nonce (UUID v4)
        let nonce = Uuid::new_v4().to_string();
        
        // Get current Unix timestamp
        let timestamp = Utc::now().timestamp();
        
        // Generate body JSON manually to ensure float formatting consistency
        // CRITICAL: Must use exact same format for signing and HTTP sending
        // Using serde_json might normalize floats (1.0 -> 1), breaking signatures
        let body_json = format!(
            r#"{{"entropy_delta":{},"network_quality":{},"uptime_seconds":{}}}"#,
            entropy_delta,
            if network_quality.fract() == 0.0 {
                format!("{:.1}", network_quality)  // Force .0 for whole numbers
            } else {
                network_quality.to_string()
            },
            uptime_seconds
        );
        
        // Create request (without signature)
        let mut request = SignedSyncRequest {
            device_id: device_id.to_string(),
            entropy_delta,
            network_quality,
            uptime_seconds: uptime_seconds as i64,
            timestamp,
            nonce: nonce.clone(),
            signature: String::new(), // Will be filled
            body_json,  // Use the same JSON string
        };
        
        // Create canonical message for signing
        let canonical_message = request.canonical_message();
        
        // Sign the message
        request.signature = identity.sign_base64(canonical_message.as_bytes());
        
        request
    }
    
    /// Create canonical message for signature verification
    /// Format: METHOD|PATH|BODY_JSON|timestamp|nonce
    fn canonical_message(&self) -> String {
        let canonical = format!(
            "POST|/api/devices/{}/sync|{}|{}|{}",
            self.device_id,
            self.body_json,  // Use cached JSON
            self.timestamp,
            self.nonce
        );
        
        canonical
    }
    
    /// Get headers for HTTP request
    pub fn headers(&self) -> Vec<(String, String)> {
        vec![
            ("x-device-id".to_string(), self.device_id.clone()),
            ("x-signature".to_string(), self.signature.clone()),
            ("x-timestamp".to_string(), self.timestamp.to_string()),
            ("x-nonce".to_string(), self.nonce.clone()),
        ]
    }
    
    /// Get request body as JSON string (same as used for signing)
    pub fn body_string(&self) -> &str {
        &self.body_json
    }
}

#[derive(Debug, Deserialize)]
pub struct SyncResponse {
    pub success: bool,
    pub device_entropy: i64,
    pub device_karma: i64,
    pub managed: bool,
    
    #[serde(default)]
    pub warning: Option<AnomalyWarning>,
}

#[derive(Debug, Deserialize)]
pub struct AnomalyWarning {
    pub anomaly_detected: bool,
    pub confidence: f64,
    pub reasons: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::device::DeviceIdentity;
    
    #[test]
    fn test_create_signed_request() {
        let identity_path = PathBuf::from("/tmp/test_key.key");
        let identity = DeviceIdentity::load_or_generate(&identity_path).unwrap();
        
        let request = SignedSyncRequest::create_and_sign(
            "test-device-123",
            1000,
            0.95,
            3600,
            &identity,
        );
        
        assert_eq!(request.device_id, "test-device-123");
        assert_eq!(request.entropy_delta, 1000);
        assert!(!request.signature.is_empty());
        assert!(!request.nonce.is_empty());
        assert!(request.timestamp > 0);
    }
    
    #[test]
    fn test_canonical_message_format() {
        let request = SignedSyncRequest {
            device_id: "dev-123".to_string(),
            entropy_delta: 500,
            network_quality: 1.0,
            uptime_seconds: 60,
            timestamp: 1738576800,
            nonce: "nonce-123".to_string(),
            signature: String::new(),
        };
        
        let canonical = request.canonical_message();
        assert!(canonical.contains("POST|/api/devices/dev-123/sync"));
        assert!(canonical.contains("|1738576800|nonce-123"));
    }
}
