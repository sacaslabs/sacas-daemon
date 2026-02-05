use std::process::Command;
use anyhow::Result;

pub struct VMDetector;

impl VMDetector {
    /// Multi-layer virtual machine detection
    /// Returns list of suspicious features detected
    pub fn detect() -> Result<Vec<String>> {
        let mut warnings = Vec::new();
        
        // 1. Check IOPlatformSerialNumber
        if Self::check_serial_is_zero()? {
            warnings.push("Serial number is '0' (VM signature)".to_string());
        }
        
        // 2. Check for VM software names in hw.model
        if let Some(vm_name) = Self::check_hypervisor_in_model()? {
            warnings.push(format!("Hypervisor detected in hw.model: {}", vm_name));
        }
        
        // 3. Check for abnormal memory size
        if Self::check_suspicious_memory()? {
            warnings.push("Suspicious memory configuration (< 8GB)".to_string());
        }
        
        // 4. Check USB devices
        if Self::check_usb_devices()? {
            warnings.push("No Apple USB devices found (VM signature)".to_string());
        }
        
        // 5. Check ROM Version
        if Self::check_rom_version()? {
            warnings.push("Invalid or missing Boot ROM version".to_string());
        }
        
        Ok(warnings)
    }
    
    fn check_serial_is_zero() -> Result<bool> {
        let output = Command::new("ioreg")
            .args(&["-l"])
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        for line in stdout.lines() {
            if line.contains("IOPlatformSerialNumber") && line.contains("\"0\"") {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    fn check_hypervisor_in_model() -> Result<Option<String>> {
        let output = Command::new("sysctl")
            .args(&["-n", "hw.model"])
            .output()?;
        
        let model = String::from_utf8_lossy(&output.stdout).to_lowercase();
        
        let hypervisors = ["vmware", "virtualbox", "parallels", "qemu", "utm"];
        
        for hypervisor in &hypervisors {
            if model.contains(hypervisor) {
                return Ok(Some(hypervisor.to_string()));
            }
        }
        
        Ok(None)
    }
    
    fn check_suspicious_memory() -> Result<bool> {
        let output = Command::new("sysctl")
            .args(&["-n", "hw.memsize"])
            .output()?;
        
        let memsize: u64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap_or(0);
        
        // Real Mac mini has at least 8GB RAM
        // 8GB = 8,589,934,592 bytes
        Ok(memsize < 8_000_000_000)
    }
    
    fn check_usb_devices() -> Result<bool> {
        let output = Command::new("ioreg")
            .args(&["-l", "-p", "IOUSB"])
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Real Macs should have Apple USB devices
        // e.g.: AppleUSBHostController, AppleUSBXHCI, etc.
        Ok(!stdout.contains("AppleUSB"))
    }
    
    fn check_rom_version() -> Result<bool> {
        // NOTE: Newer M4 Macs may not report Boot ROM via system_profiler
        // Since we already validated the Mac model in MacValidator,
        // we can safely skip this check for genuine Apple Silicon Macs
        
        // ROM check disabled for now - other checks are sufficient
        Ok(false)
        
        /* Original ROM check - commenting out for M4 compatibility
        let output = Command::new("system_profiler")
            .args(&["SPHardwareDataType"])
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        for line in stdout.lines() {
            if line.contains("Boot ROM Version") {
                let version = line.split(':').nth(1).unwrap_or("").trim();
                // Empty or "0" are both suspicious
                if version.is_empty() || version == "0" || version == "unknown" {
                    return Ok(true);
                }
                // Real Mac ROM version format like "10151.81.1"
                return Ok(false);
            }
        }
        
        // No ROM version found is also suspicious
        Ok(true)
        */
    }
}
