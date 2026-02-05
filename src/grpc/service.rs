use tonic::{transport::Server, Request, Response, Status};
use crate::state::StateManager;
use std::sync::Arc;
use tracing::info;

pub mod proto {
    tonic::include_proto!("sacas");
}

use proto::game_service_server::{GameService, GameServiceServer};
use proto::*;

pub struct GameServiceImpl {
    state_manager: Arc<StateManager>,
}

impl GameServiceImpl {
    pub fn new(state_manager: Arc<StateManager>) -> Self {
        Self { state_manager }
    }
}

#[tonic::async_trait]
impl GameService for GameServiceImpl {
    async fn get_status(
        &self,
        _request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        let state = self.state_manager.get_snapshot().await;
        
        let cooldown_seconds = if let Some(cooldown_ends) = state.player.defense.cooldown_ends {
            let now = chrono::Utc::now();
            if cooldown_ends > now {
                Some((cooldown_ends - now).num_seconds())
            } else {
                None
            }
        } else {
            None
        };

        let response = GetStatusResponse {
            player_id: state.player.id.clone(),
            karma: state.player.karma,
            entropy: state.player.entropy,
            capacity: state.player.capacity,
            yield_per_tick: state.player.calculate_yield() as f64,
            defense: Some(Defense {
                l1: state.player.defense.l1,
                l2: state.player.defense.l2,
                l3: state.player.defense.l3,
                cooldown_seconds,
            }),
            position: Some(Position {
                latency_vector: state.player.position.latency_vector,
                coords: state.player.position.coords.map(|(x, y)| Coords { x, y }),
            }),
            network_quality: state.player.network_quality,
            parasite_count: state.parasites.len() as u32,
            passive_income: state.player.passive_income,
            climate: Some(Climate {
                code: state.climate.code,
                description: state.climate.description,
            }),
        };

        Ok(Response::new(response))
    }

    async fn scan_network(
        &self,
        request: Request<ScanNetworkRequest>,
    ) -> Result<Response<ScanNetworkResponse>, Status> {
        let req = request.into_inner();
        
        // Mock implementation - in production this would query the server
        let state = self.state_manager.get_snapshot().await;
        
        let nodes = state.visible_nodes.iter().map(|n| {
            Node {
                id: n.id.clone(),
                karma: n.karma,
                distance: n.distance,
                noise: n.noise,
                estimated_defense: n.estimated_defense.as_ref().map(|d| Defense {
                    l1: d.l1,
                    l2: d.l2,
                    l3: d.l3,
                    cooldown_seconds: None,
                }),
            }
        }).collect();

        Ok(Response::new(ScanNetworkResponse { nodes }))
    }

    async fn simulate_battle(
        &self,
        request: Request<SimulateBattleRequest>,
    ) -> Result<Response<SimulateBattleResponse>, Status> {
        let req = request.into_inner();
        
        // Simple simulation logic
        let total_attack = req.attack_l1 + req.attack_l2 + req.attack_l3;
        
        // Mock probabilities
        let l1_prob = 0.75;
        let l2_prob = 0.60;
        let l3_prob = 0.45;
        
        let risk_level = if total_attack < 10000 {
            "LOW"
        } else if total_attack < 50000 {
            "MEDIUM"
        } else {
            "HIGH"
        }.to_string();

        Ok(Response::new(SimulateBattleResponse {
            l1_crush_probability: l1_prob,
            l2_intel_probability: l2_prob,
            l3_parasitize_probability: l3_prob,
            expected_roi: (total_attack as f64 * 0.5) as i64,
            risk_level,
        }))
    }

    async fn execute_attack(
        &self,
        request: Request<ExecuteAttackRequest>,
    ) -> Result<Response<ExecuteAttackResponse>, Status> {
        let req = request.into_inner();
        
        // Mock implementation
        info!("🎯 Executing attack on {}", req.target_id);
        
        Ok(Response::new(ExecuteAttackResponse {
            session_id: uuid::Uuid::new_v4().to_string(),
            l1_crushed: true,
            l2_intel_success: false,
            l2_revealed_d3: None,
            l3_parasitized: true,
            stolen_entropy: 5000,
            passive_yield: 0.5,
        }))
    }

    async fn update_defense(
        &self,
        request: Request<UpdateDefenseRequest>,
    ) -> Result<Response<UpdateDefenseResponse>, Status> {
        let req = request.into_inner();
        
        if req.defense_array.len() != 3 {
            return Ok(Response::new(UpdateDefenseResponse {
                success: false,
                error: Some("Defense array must have exactly 3 values".to_string()),
            }));
        }

        match self.state_manager.update_defense(
            req.defense_array[0],
            req.defense_array[1],
            req.defense_array[2]
        ).await {
            Ok(_) => Ok(Response::new(UpdateDefenseResponse {
                success: true,
                error: None,
            })),
            Err(e) => Ok(Response::new(UpdateDefenseResponse {
                success: false,
                error: Some(e),
            })),
        }
    }

    async fn get_parasites(
        &self,
        _request: Request<GetParasitesRequest>,
    ) -> Result<Response<GetParasitesResponse>, Status> {
        let state = self.state_manager.get_snapshot().await;
        
        let parasites = state.parasites.iter().map(|p| {
            Parasite {
                node_id: p.node_id.clone(),
                tax_rate: p.tax_rate,
                yield_per_tick: p.yield_per_tick,
                total_collected: p.total_collected,
            }
        }).collect();

        Ok(Response::new(GetParasitesResponse { parasites }))
    }

    async fn get_climate(
        &self,
        _request: Request<GetClimateRequest>,
    ) -> Result<Response<GetClimateResponse>, Status> {
        let state = self.state_manager.get_snapshot().await;
        
        Ok(Response::new(GetClimateResponse {
            climate: Some(Climate {
                code: state.climate.code,
                description: state.climate.description,
            }),
        }))
    }
}

pub async fn start_grpc_server(
    addr: String,
    state_manager: Arc<StateManager>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = addr.parse()?;
    let service = GameServiceImpl::new(state_manager);

    info!("🚀 gRPC server listening on {}", addr);

    Server::builder()
        .add_service(GameServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
