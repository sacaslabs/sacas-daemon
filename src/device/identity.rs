use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer};
use anyhow::{Result, Context};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Device cryptographic identity manager
pub struct DeviceIdentity {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl DeviceIdentity {
    /// Load or generate device identity
    pub fn load_or_generate(key_path: &Path) -> Result<Self> {
        if key_path.exists() {
            Self::load(key_path)
        } else {
            info!("🔐 Generating new Ed25519 device identity...");
            let identity = Self::generate()?;
            identity.save(key_path)?;
            info!("✅ Device identity generated and saved");
            Ok(identity)
        }
    }

    /// Generate new Ed25519 key pair
    fn generate() -> Result<Self> {
        let mut rng = rand::rngs::OsRng;
        let signing_key = SigningKey::from_bytes(&rand::random());
        let verifying_key = signing_key.verifying_key();
        
        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Load existing key from file
    fn load(path: &Path) -> Result<Self> {
        let key_bytes = fs::read(path)
            .context("Failed to read device key file")?;
        
        if key_bytes.len() != 32 {
            anyhow::bail!("Invalid key file: expected 32 bytes, got {}", key_bytes.len());
        }

        let key_array: [u8; 32] = key_bytes.try_into()
            .map_err(|_| anyhow::anyhow!("Failed to convert key bytes"))?;
        
        let signing_key = SigningKey::from_bytes(&key_array);
        let verifying_key = signing_key.verifying_key();
        
        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Save private key to file (with restricted permissions)
    fn save(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write private key
        fs::write(path, self.signing_key.to_bytes())?;

        // Set restrictive permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o600); // Read/write for owner only
            fs::set_permissions(path, perms)?;
        }

        Ok(())
    }

    /// Get public key as base64 string
    pub fn public_key_base64(&self) -> String {
        base64::encode(self.verifying_key.to_bytes())
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Sign and encode signature as base64
    pub fn sign_base64(&self, message: &[u8]) -> String {
        let signature = self.sign(message);
        base64::encode(signature.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_and_load() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("device.key");

        // Generate
        let identity1 = DeviceIdentity::load_or_generate(&key_path).unwrap();
        let pubkey1 = identity1.public_key_base64();

        // Load
        let identity2 = DeviceIdentity::load_or_generate(&key_path).unwrap();
        let pubkey2 = identity2.public_key_base64();

        // Should be the same
        assert_eq!(pubkey1, pubkey2);
    }

    #[test]
    fn test_sign_and_verify() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("device.key");
        let identity = DeviceIdentity::load_or_generate(&key_path).unwrap();

        let message = b"Hello, SACAS!";
        let signature = identity.sign(message);

        // Verify
        use ed25519_dalek::Verifier;
        assert!(identity.verifying_key.verify(message, &signature).is_ok());
    }
}
