use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub player: Player,
    pub visible_nodes: Vec<Node>,
    pub parasites: Vec<Parasite>,
    pub climate: Climate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: String,
    pub karma: u64,
    pub entropy: u64,
    pub capacity: u64,
    pub defense: DefenseArray,
    pub position: TopologyPosition,
    pub network_quality: f64,
    pub passive_income: f64,
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefenseArray {
    pub l1: u64,
    pub l2: u64,
    pub l3: u64,
    pub last_update: DateTime<Utc>,
    pub cooldown_ends: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyPosition {
    pub latency_vector: Vec<f64>,
    pub coords: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub karma: u64,
    pub distance: f64,
    pub noise: f64,
    pub estimated_defense: Option<DefenseArray>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parasite {
    pub node_id: String,
    pub tax_rate: f64,
    pub yield_per_tick: f64,
    pub total_collected: u64,
    pub established_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Climate {
    pub code: String,
    pub description: String,
    pub modifiers: serde_json::Value,
    pub start_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyVector {
    pub timestamp: DateTime<Utc>,
    pub data: Vec<f64>,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleSimulation {
    pub l1_crush_probability: f64,
    pub l2_intel_probability: f64,
    pub l3_parasitize_probability: f64,
    pub expected_roi: i64,
    pub risk_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleResult {
    pub session_id: String,
    pub l1_crushed: bool,
    pub l2_intel_success: bool,
    pub l2_revealed_d3: Option<u64>,
    pub l3_parasitized: bool,
    pub stolen_entropy: u64,
    pub passive_yield: f64,
}

impl Player {
    pub fn new(id: String, karma: u64) -> Self {
        let capacity = karma * 100;
        Self {
            id,
            karma,
            entropy: 0,
            capacity,
            defense: DefenseArray {
                l1: 0,
                l2: 0,
                l3: 0,
                last_update: Utc::now(),
                cooldown_ends: None,
            },
            position: TopologyPosition {
                latency_vector: vec![],
                coords: None,
            },
            network_quality: 1.0,
            passive_income: 0.0,
            last_update: Utc::now(),
        }
    }

    pub fn calculate_yield(&self) -> u64 {
        let base = (self.karma as f64).sqrt();
        (base * self.network_quality * 0.5) as u64
    }

    pub fn calculate_inertia_seconds(&self) -> u64 {
        ((self.karma as f64).ln() * 600.0) as u64
    }
}
