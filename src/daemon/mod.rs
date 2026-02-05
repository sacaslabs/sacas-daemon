use crate::config::Config;
use crate::state::StateManager;
use crate::network::NetworkProbe;
use crate::mining::MiningEngine;
use crate::grpc::start_grpc_server;
use anyhow::Result;
use std::sync::Arc;
use tracing::info;

pub struct OmniDaemon {
    config: Config,
    state_manager: Arc<StateManager>,
    network_probe: NetworkProbe,
    mining_engine: MiningEngine,
}

impl OmniDaemon {
    pub async fn new(config: Config) -> Result<Self> {
        // Initialize state manager  
        let state_manager = Arc::new(StateManager::new(
            config.device_id.clone().unwrap_or_default(),
            config.karma,
        ));

        // Initialize network probe
        let network_probe = NetworkProbe::new(config.network.anchors.clone())?;

        // Initialize mining engine (use SAME state_manager instance!)
        let mining_state = StateManager {
            state: state_manager.get_handle(),
        };
        let mining_engine = MiningEngine::new(
            mining_state,
            config.mining.tick_interval_secs,
        );

        Ok(Self {
            config,
            state_manager,
            network_probe,
            mining_engine,
        })
    }

    pub fn get_state(&self) -> Arc<StateManager> {
        self.state_manager.clone()
    }

    pub async fn run(self) -> Result<()> {
        let state_manager = self.state_manager.clone();
        let network_probe = Arc::new(self.network_probe);
        let config = Arc::new(self.config);

        // Spawn gRPC server
        let grpc_addr = format!("127.0.0.1:{}", config.grpc_port);
        let grpc_state = state_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = start_grpc_server(grpc_addr, grpc_state).await {
                tracing::error!("gRPC server error: {}", e);
            }
        });

        // Spawn network probe loop
        let probe_state = state_manager.clone();
        let probe = network_probe.clone();
        let probe_interval = config.network.probe_interval_secs;
        tokio::spawn(async move {
            Self::probe_network_loop(probe, probe_state, probe_interval).await;
        });

        // Spawn mining loop (runs in current task)
        self.mining_engine.run().await;

        Ok(())
    }

    async fn probe_network_loop(
        probe: Arc<NetworkProbe>,
        state_manager: Arc<StateManager>,
        interval_secs: u64,
    ) {
        use tokio::time::{interval, Duration};
        
        let mut ticker = interval(Duration::from_secs(interval_secs));
        
        info!("🌐 Network probe started (interval: {}s)", interval_secs);

        loop {
            ticker.tick().await;
            
            match probe.build_latency_vector().await {
                Ok(vector) => {
                    let quality = probe.calculate_network_quality(&vector.data);
                    state_manager.update_network_quality(quality).await;
                    
                    info!(
                        "Network probe complete: avg_latency={:.1}ms, quality={:.2}",
                        vector.data.iter().sum::<f64>() / vector.data.len() as f64,
                        quality
                    );
                }
                Err(e) => {
                    tracing::error!("Network probe failed: {}", e);
                }
            }
        }
    }
}
