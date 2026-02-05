use crate::state::StateManager;
use tokio::time::{interval, Duration};
use tracing::{info, debug};

pub struct MiningEngine {
    state_manager: StateManager,
    tick_interval_secs: u64,
}

impl MiningEngine {
    pub fn new(state_manager: StateManager, tick_interval_secs: u64) -> Self {
        Self {
            state_manager,
            tick_interval_secs,
        }
    }

    pub async fn run(&self) {
        let mut ticker = interval(Duration::from_secs(self.tick_interval_secs));
        
        info!("⛏️  Mining engine started (tick every {}s)", self.tick_interval_secs);

        loop {
            ticker.tick().await;
            
            let state = self.state_manager.get_snapshot().await;
            
            // Calculate base yield
            let yield_value = state.player.calculate_yield();
            
            // Add passive income
            let passive = (state.player.passive_income * self.tick_interval_secs as f64) as u64;
            
            let total_income = yield_value + passive;
            
            // Update balance
            self.state_manager.update_entropy(total_income as i64).await;
            
            let new_state = self.state_manager.get_snapshot().await;
            
            debug!(
                "Mining tick: +{} Ω (base: {}, passive: {}) | Total: {} / {} Ω",
                total_income,
                yield_value,
                passive,
                new_state.player.entropy,
                new_state.player.capacity
            );

            // Check for decay
            if new_state.player.entropy > new_state.player.capacity {
                let excess = new_state.player.entropy - new_state.player.capacity;
                info!("⚠️  Entropy exceeds capacity! Decay will occur: -{} Ω/tick", (excess as f64 * 0.02) as u64);
            }
        }
    }
}
