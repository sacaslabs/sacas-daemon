use crate::types::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;

pub struct StateManager {
    pub state: Arc<RwLock<GameState>>,
}

impl StateManager {
    pub fn new(player_id: String, karma: u64) -> Self {
        let state = GameState {
            player: Player::new(player_id, karma),
            visible_nodes: vec![],
            parasites: vec![],
            climate: Climate {
                code: "NORMAL".to_string(),
                description: "Normal network conditions".to_string(),
                modifiers: serde_json::json!({}),
                start_time: Utc::now(),
            },
        };

        Self {
            state: Arc::new(RwLock::new(state)),
        }
    }

    pub fn get_handle(&self) -> Arc<RwLock<GameState>> {
        self.state.clone()
    }

    pub async fn update_entropy(&self, delta: i64) {
        let mut state = self.state.write().await;
        
        if delta >= 0 {
            state.player.entropy += delta as u64;
        } else {
            state.player.entropy = state.player.entropy.saturating_sub(delta.abs() as u64);
        }

        // Check for decay
        if state.player.entropy > state.player.capacity {
            let excess = state.player.entropy - state.player.capacity;
            let decay = (excess as f64 * 0.02) as u64;
            state.player.entropy = state.player.entropy.saturating_sub(decay);
        }

        state.player.last_update = Utc::now();
    }

    pub async fn update_defense(&self, l1: u64, l2: u64, l3: u64) -> Result<(), String> {
        let mut state = self.state.write().await;

        // Check cooldown time
        if let Some(cooldown_ends) = state.player.defense.cooldown_ends {
            if Utc::now() < cooldown_ends {
                let remaining = (cooldown_ends - Utc::now()).num_seconds();
                return Err(format!("Defense on cooldown for {} seconds", remaining));
            }
        }

        // Check if there's enough Ω
        let total = l1 + l2 + l3;
        if total > state.player.entropy {
            return Err("Insufficient Entropy".to_string());
        }

        // Update defense
        state.player.defense.l1 = l1;
        state.player.defense.l2 = l2;
        state.player.defense.l3 = l3;
        state.player.defense.last_update = Utc::now();

        // Set cooldown time
        let inertia_seconds = state.player.calculate_inertia_seconds();
        state.player.defense.cooldown_ends = Some(
            Utc::now() + chrono::Duration::seconds(inertia_seconds as i64)
        );

        Ok(())
    }

    pub async fn update_network_quality(&self, quality: f64) {
        let mut state = self.state.write().await;
        state.player.network_quality = quality.clamp(0.1, 1.5);
    }

    pub async fn update_karma(&self, new_karma: u64) {
        let mut state = self.state.write().await;
        state.player.karma = new_karma;
        // Recalculate capacity when karma changes
        state.player.capacity = new_karma * 100;
    }

    pub async fn add_parasite(&self, parasite: Parasite) {
        let mut state = self.state.write().await;
        state.parasites.push(parasite);
        
        // Recalculate passive income
        state.player.passive_income = state.parasites
            .iter()
            .map(|p| p.yield_per_tick)
            .sum();
    }

    pub async fn update_visible_nodes(&self, nodes: Vec<Node>) {
        let mut state = self.state.write().await;
        state.visible_nodes = nodes;
    }

    pub async fn update_climate(&self, climate: Climate) {
        let mut state = self.state.write().await;
        state.climate = climate;
    }

    pub async fn get_snapshot(&self) -> GameState {
        self.state.read().await.clone()
    }
}
