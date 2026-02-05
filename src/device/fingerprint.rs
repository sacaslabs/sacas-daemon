use std::process::Command;
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacHardwareInfo {
    pub hardware_uuid: String,      // IOPlatformUUID
    pub serial_number: String,      // Serial Number
    pub model_identifier: String,   // MacBookPro18,1
    pub board_id: String,           // Mac-xxx
    pub rom_version: String,        // Boot ROM Version
    pub cpu_brand: String,          // Apple M1 Pro
}

impl MacHardwareInfo {
    pub fn collect() -> Result<Self> {
        Ok(Self {
            hardware_uuid: Self::get_hardware_uuid()?,
            serial_number: Self::get_serial_number()?,
            model_identifier: Self::get_model_identifier()?,
            board_id: Self::get_board_id()?,
            rom_version: Self::get_rom_version()?,
            cpu_brand: Self::get_cpu_brand()?,
        })
    }
    
    /// Get Hardware UUID (most reliable unique identifier)
    fn get_hardware_uuid() -> Result<String> {
        let output = Command::new("ioreg")
            .args(&["-d2", "-c", "IOPlatformExpertDevice"])
            .output()
            .context("Failed to run ioreg command")?;
        
        if !output.status.success() {
            anyhow::bail!("ioreg command failed");
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Parse: "IOPlatformUUID" = "XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"
        for line in stdout.lines() {
            if line.contains("IOPlatformUUID") {
                if let Some(uuid) = line.split('"').nth(3) {
                    return Ok(uuid.to_string());
                }
            }
        }
        
        anyhow::bail!("IOPlatformUUID not found in ioreg output")
    }
    
    /// Get Serial Number
    fn get_serial_number() -> Result<String> {
        let output = Command::new("ioreg")
            .args(&["-l"])
            .output()
            .context("Failed to run ioreg command")?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // 解析: "IOPlatformSerialNumber" = "XXXXXXXXXX"
        for line in stdout.lines() {
            if line.contains("IOPlatformSerialNumber") {
                if let Some(serial) = line.split('"').nth(3) {
                    // "0" 表示虚拟机
                    if serial == "0" {
                        anyhow::bail!("Serial number is '0' - Virtual machine detected");
                    }
                    if !serial.is_empty() {
                        return Ok(serial.to_string());
                    }
                }
            }
        }
        
        anyhow::bail!("Serial number not found or invalid")
    }
    
    /// Get Model Identifier (e.g., MacBookPro18,1)
    fn get_model_identifier() -> Result<String> {
        let output = Command::new("sysctl")
            .args(&["-n", "hw.model"])
            .output()
            .context("Failed to run sysctl command")?;
        
        let model = String::from_utf8_lossy(&output.stdout).trim().to_string();
        
        if model.is_empty() {
            anyhow::bail!("hw.model is empty");
        }
        
        Ok(model)
    }
    
    /// Get Board ID
    fn get_board_id() -> Result<String> {
        let output = Command::new("ioreg")
            .args(&["-l"])
            .output()
            .context("Failed to run ioreg command")?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        for line in stdout.lines() {
            if line.contains("board-id") {
                // Format: "board-id" = <"Mac-XXXXXXXXXXXX">
                if let Some(content) = line.split('<').nth(1) {
                    if let Some(board) = content.split('>').nth(0) {
                        let board = board.trim().trim_matches('"');
                        if !board.is_empty() {
                            return Ok(board.to_string());
                        }
                    }
                }
            }
        }
        
        // Fallback: use system_profiler
        let output = Command::new("system_profiler")
            .args(&["SPHardwareDataType"])
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("Model Identifier") {
                if let Some(id) = line.split(':').nth(1) {
                    return Ok(id.trim().to_string());
                }
            }
        }
        
        Ok("unknown".to_string())
    }
    
    /// Get Boot ROM Version
    fn get_rom_version() -> Result<String> {
        let output = Command::new("system_profiler")
            .args(&["SPHardwareDataType"])
            .output()
            .context("Failed to run system_profiler")?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        for line in stdout.lines() {
            if line.contains("Boot ROM Version") {
                if let Some(version) = line.split(':').nth(1) {
                    return Ok(version.trim().to_string());
                }
            }
        }
        
        Ok("unknown".to_string())
    }
    
    /// Get CPU Brand
    fn get_cpu_brand() -> Result<String> {
        let output = Command::new("sysctl")
            .args(&["-n", "machdep.cpu.brand_string"])
            .output()
            .context("Failed to run sysctl command")?;
        
        let cpu = String::from_utf8_lossy(&output.stdout).trim().to_string();
        
        if cpu.is_empty() {
            anyhow::bail!("CPU brand string is empty");
        }
        
        Ok(cpu)
    }
    
    /// Generate unique device fingerprint (SHA256)
    pub fn generate_fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.hardware_uuid.as_bytes());
        hasher.update(self.serial_number.as_bytes());
        hasher.update(self.model_identifier.as_bytes());
        hasher.update(self.board_id.as_bytes());
        
        format!("{:x}", hasher.finalize())
    }
}
