use crate::config::Anchor;
use crate::types::LatencyVector;
use anyhow::{Result, Context};
use surge_ping::{Client, Config as PingConfig, PingIdentifier, PingSequence, ICMP};
use std::net::IpAddr;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, warn};
use chrono::Utc;

pub struct NetworkProbe {
    anchors: Vec<Anchor>,
    ping_client: Client,
}

impl NetworkProbe {
    pub fn new(anchors: Vec<Anchor>) -> Result<Self> {
        let config = PingConfig::default();
        let ping_client = Client::new(&config)
            .context("Failed to create ping client")?;

        Ok(Self {
            anchors,
            ping_client,
        })
    }

    pub async fn build_latency_vector(&self) -> Result<LatencyVector> {
        let mut latencies = Vec::new();

        for anchor in &self.anchors {
            let latency = self.ping_anchor(anchor).await;
            latencies.push(latency);
        }

        debug!("Latency vector: {:?}", latencies);

        let signature = self.sign_vector(&latencies);
        
        Ok(LatencyVector {
            timestamp: Utc::now(),
            data: latencies,
            signature,
        })
    }

    async fn ping_anchor(&self, anchor: &Anchor) -> f64 {
        let ip: IpAddr = match anchor.ip.parse() {
            Ok(ip) => ip,
            Err(e) => {
                warn!("Invalid IP for anchor {}: {}", anchor.id, e);
                return 999.0; // Return high latency for invalid IPs
            }
        };

        // Try to ping 3 times and take median
        let mut results = Vec::new();

        for i in 0..3 {
            match self.ping_once(ip, i).await {
                Ok(latency) => results.push(latency),
                Err(e) => {
                    debug!("Ping failed for {} (attempt {}): {}", anchor.id, i + 1, e);
                }
            }

            // Small delay between pings
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if results.is_empty() {
            warn!("All pings failed for anchor {}", anchor.id);
            return 999.0;
        }

        // Return median
        results.sort_by(|a, b| a.partial_cmp(b).unwrap());
        results[results.len() / 2]
    }

    async fn ping_once(&self, ip: IpAddr, sequence: u16) -> Result<f64> {
        let payload = [0; 8];
        
        let mut pinger = self.ping_client.pinger(ip, PingIdentifier(1234)).await;
        pinger.timeout(Duration::from_secs(2));

        let start = std::time::Instant::now();
        
        match timeout(
            Duration::from_secs(3),
            pinger.ping(PingSequence(sequence), &payload)
        ).await {
            Ok(Ok((_, duration))) => {
                Ok(duration.as_millis() as f64)
            }
            Ok(Err(e)) => {
                Err(anyhow::anyhow!("Ping error: {}", e))
            }
            Err(_) => {
                Err(anyhow::anyhow!("Ping timeout"))
            }
        }
    }

    fn sign_vector(&self, vector: &[f64]) -> String {
        // Simplified signing - in production use ed25519
        use sha2::{Sha256, Digest};
        let data = serde_json::to_string(vector).unwrap();
        let hash = Sha256::digest(data.as_bytes());
        base64::encode(hash)
    }

    pub fn calculate_network_quality(&self, latencies: &[f64]) -> f64 {
        // Calculate network quality based on latencies
        let avg_latency: f64 = latencies.iter().sum::<f64>() / latencies.len() as f64;
        
        // Quality score: 1.5 for <30ms, 1.0 for ~100ms, 0.1 for >500ms
        if avg_latency < 30.0 {
            1.5
        } else if avg_latency < 100.0 {
            1.2
        } else if avg_latency < 200.0 {
            1.0
        } else if avg_latency < 500.0 {
            0.5
        } else {
            0.1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ping() {
        let anchors = vec![
            Anchor {
                id: "cloudflare".to_string(),
                ip: "1.1.1.1".to_string(),
                region: "global".to_string(),
            }
        ];

        let probe = NetworkProbe::new(anchors).unwrap();
        let vector = probe.build_latency_vector().await.unwrap();

        assert_eq!(vector.data.len(), 1);
        println!("Latency: {:?}", vector.data);
    }
}
