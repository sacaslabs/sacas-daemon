use anyhow::Result;
use super::fingerprint::MacHardwareInfo;

// Whitelist of allowed Mac models (only genuine Macs allowed)
const ALLOWED_MAC_MODELS: &[&str] = &[
    // Mac mini
    "Macmini9,1",  // Mac mini (M1, 2020)
    "Mac14,3",     // Mac mini (M2, 2023)
    "Mac14,12",    // Mac mini (M2 Pro, 2023)
    "Mac16,10",    // Mac mini (M4, 2024)
    
    // MacBook Pro - M1 Series (2021)
    "MacBookPro18,1",  // MacBook Pro (14-inch, M1 Pro, 2021)
    "MacBookPro18,2",  // MacBook Pro (14-inch, M1 Max, 2021)
    "MacBookPro18,3",  // MacBook Pro (16-inch, M1 Pro, 2021)
    "MacBookPro18,4",  // MacBook Pro (16-inch, M1 Max, 2021)
    
    // MacBook Pro - M2 Series (2022-2023)
    "Mac14,7",    // MacBook Pro (13-inch, M2, 2022)
    "Mac14,5",    // MacBook Pro (14-inch, M2 Pro, 2023)
    "Mac14,9",    // MacBook Pro (14-inch, M2 Max, 2023)
    "Mac14,6",    // MacBook Pro (16-inch, M2 Pro, 2023)
    "Mac14,10",   // MacBook Pro (16-inch, M2 Max, 2023)
    
    // MacBook Pro - M3 Series (2023)
    "Mac15,3",    // MacBook Pro (14-inch, M3, 2023)
    "Mac15,6",    // MacBook Pro (14-inch, M3 Pro, 2023)
    "Mac15,7",    // MacBook Pro (14-inch, M3 Max, 2023)
    "Mac15,8",    // MacBook Pro (16-inch, M3 Pro, 2023)
    "Mac15,9",    // MacBook Pro (16-inch, M3 Pro, 2023)
    "Mac15,10",   // MacBook Pro (16-inch, M3 Max, 2023)
    "Mac15,11",   // MacBook Pro (16-inch, M3 Max, 2023)
    
    // MacBook Pro - M4 Series (2024)
    "Mac16,1",    // MacBook Pro (14-inch, M4, 2024)
    "Mac16,2",    // MacBook Pro (14-inch, M4 Pro, 2024)
    "Mac16,3",    // MacBook Pro (14-inch, M4 Max, 2024)
    "Mac16,4",    // MacBook Pro (14-inch, M4 Max, 2024) - High config
    "Mac16,5",    // MacBook Pro (16-inch, M4 Pro, 2024)
    "Mac16,6",    // MacBook Pro (16-inch, M4 Max, 2024)
    "Mac16,7",    // MacBook Pro (16-inch, M4 Pro, 2024) - High config
    
    // MacBook Air - M1 Series (2020)
    "MacBookAir10,1",  // MacBook Air (M1, 2020)
    
    // MacBook Air - M2 Series (2022-2023)
    "Mac14,2",     // MacBook Air (13-inch, M2, 2022)
    "Mac14,15",    // MacBook Air (15-inch, M2, 2023)
    
    // MacBook Air - M3 Series (2024)
    "Mac15,12",    // MacBook Air (13-inch, M3, 2024)
    "Mac15,13",    // MacBook Air (15-inch, M3, 2024)
    
    // MacBook Air - M4 Series (2025, predicted)
    "Mac16,12",    // MacBook Air (13-inch, M4, 2025)
    "Mac16,13",    // MacBook Air (15-inch, M4, 2025)
    
    // iMac - M1 Series (2021)
    "iMac21,1",    // iMac (24-inch, M1, 2021, 2 ports)
    "iMac21,2",    // iMac (24-inch, M1, 2021, 4 ports)
    
    // iMac - M3 Series (2023)
    "Mac15,4",     // iMac (24-inch, M3, 2023, 2 ports)
    "Mac15,5",     // iMac (24-inch, M3, 2023, 4 ports)
    
    // iMac - M4 Series (2024)
    "Mac16,8",     // iMac (24-inch, M4, 2024)
    "Mac16,9",     // iMac (24-inch, M4, 2024)
    
    // Mac Studio - M1 Series (2022)
    "Mac13,1",     // Mac Studio (M1 Max, 2022)
    "Mac13,2",     // Mac Studio (M1 Ultra, 2022)
    
    // Mac Studio - M2 Series (2023)
    "Mac14,13",    // Mac Studio (M2 Max, 2023)
    "Mac14,14",    // Mac Studio (M2 Ultra, 2023)
    
    // Mac Studio - M3/M4 Series (2025, predicted)
    "Mac16,14",    // Mac Studio (M3/M4 Max, 2025)
    "Mac16,15",    // Mac Studio (M3/M4 Ultra, 2025)
    
    // Mac Pro - M2 Series (2023)
    "Mac14,8",     // Mac Pro (M2 Ultra, 2023)
    
    // Mac Pro - M4 Series (2025, predicted)
    "Mac16,16",    // Mac Pro (M4 Ultra, 2025)
];

pub struct MacValidator;

impl MacValidator {
    /// Validate if this is a genuine Mac computer
    pub fn validate(hw_info: &MacHardwareInfo) -> Result<()> {
        // 1. Check if model is in whitelist
        if !ALLOWED_MAC_MODELS.contains(&hw_info.model_identifier.as_str()) {
            anyhow::bail!(
                "❌ Invalid Mac model: '{}'\n\n\
                 Only genuine Mac computers are allowed to run SACAS.\n\
                 Supported models: Mac mini, MacBook Pro, MacBook Air, iMac, Mac Studio, Mac Pro (Apple Silicon only)",
                hw_info.model_identifier
            );
        }
        
        // 2. Check Serial Number is not "0" (VM signature)
        if hw_info.serial_number == "0" || hw_info.serial_number.is_empty() {
            anyhow::bail!("❌ Invalid serial number. Virtual machines are not allowed.");
        }
        
        
        // 3. Check Board ID for VM signatures
        // Real Macs have various board ID formats (Mac-, model numbers, etc.)
        // We only reject obvious VM signatures: "0", empty, or "unknown"
        // This ensures compatibility with all Mac models past, present, and future
        if hw_info.board_id == "0" 
            || hw_info.board_id.is_empty() 
            || hw_info.board_id == "unknown" {
            anyhow::bail!("❌ Invalid board ID: Virtual machines not allowed");
        }
        // Accept any other board ID format (real Macs have diverse formats)
        
        // 4. Check CPU Brand (must be Apple Silicon)
        if !hw_info.cpu_brand.contains("Apple") {
            anyhow::bail!(
                "❌ Invalid CPU: '{}'\n\n\
                 SACAS only supports Apple Silicon Macs.\n\
                 Intel-based Macs are not supported.",
                hw_info.cpu_brand
            );
        }
        
        Ok(())
    }
    
    /// Get friendly name for Mac model
    pub fn get_friendly_name(model: &str) -> &'static str {
        match model {
            "Macmini9,1" => "Mac mini (M1, 2020)",
            "Mac14,3" => "Mac mini (M2, 2023)",
            "Mac14,12" => "Mac mini (M2 Pro, 2023)",
            "Mac16,10" => "Mac mini (M4, 2024)",
            _ if model.starts_with("MacBookPro") => "MacBook Pro (Apple Silicon)",
            _ if model.starts_with("MacBookAir") => "MacBook Air (Apple Silicon)",
            _ if model.starts_with("iMac") => "iMac (Apple Silicon)",
            _ if model.starts_with("Mac13") || model.starts_with("Mac14,13") || model.starts_with("Mac14,14") || model.starts_with("Mac16,14") || model.starts_with("Mac16,15") => "Mac Studio",
            _ if model == "Mac14,8" || model == "Mac16,16" => "Mac Pro (Apple Silicon)",
            _ => "Mac (Apple Silicon)",
        }
    }
}
