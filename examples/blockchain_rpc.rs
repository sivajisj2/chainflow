use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;
#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    method: &'a str,
    params: Vec<serde_json::Value>,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<String>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Clone)]
struct BlockchainRpc {
    client: Client,
    endpoint: String,
}

impl BlockchainRpc {
    fn new(endpoint: String) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(5))
            .build()?;

        Ok(Self { client, endpoint })
    }

    async fn block_number(&self) -> Result<u64, String> {
        let request = RpcRequest {
            jsonrpc: "2.0",
            method: "eth_blockNumber",
            params: vec![],
            id: 1,
        };

        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Transport error: {e}"))?;

        let response = response
            .error_for_status()
            .map_err(|e| format!("HTTP error: {e}"))?;

        let body: RpcResponse = response
            .json()
            .await
            .map_err(|e| format!("Invalid RPC response: {e}"))?;

        if let Some(error) = body.error {
            return Err(format!("RPC error {}: {}", error.code, error.message));
        }

        let hex = body
            .result
            .ok_or_else(|| "Missing block number".to_string())?;

        u64::from_str_radix(
            hex.strip_prefix("0x")
                .ok_or("Expected hexadecimal block number")?,
            16,
        )
        .map_err(|e| format!("Invalid block number: {e}"))
    }
}

struct RpcGateway {
    providers: Vec<BlockchainRpc>,
    breakers: Vec<Mutex<CircuitBreaker>>,
}

impl RpcGateway {
    async fn latest_block(&self) -> Result<u64, String> {
        let mut errors = Vec::new();

        for (index, provider) in self.providers.iter().enumerate() {
            {
                let breaker = self.breakers[index].lock().await;

                if !breaker.allow_request() {
                    println!("Provider {} circuit is open", index + 1);
                    continue;
                }
            }

            println!("Trying provider {}", index + 1);

            match provider.block_number().await {
                Ok(block) => {
                    let mut breaker = self.breakers[index].lock().await;
                    breaker.record_success();

                    return Ok(block);
                }

                Err(error) => {
                    let mut breaker = self.breakers[index].lock().await;
                    breaker.record_failure();

                    errors.push(format!("Provider {}: {error}", index + 1));
                }
            }
        }

        Err(format!("No provider succeeded: {}", errors.join(" | ")))
    }
}

struct CircuitBreaker {
    consecutive_failures: u32,
    open_until: Option<Instant>,
}

impl CircuitBreaker {
    fn new() -> Self {
        Self {
            consecutive_failures: 0,
            open_until: None,
        }
    }

    fn allow_request(&self) -> bool {
        match self.open_until {
            None => true,
            Some(until) => Instant::now() >= until,
        }
    }

    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.open_until = None;
    }

    fn record_failure(&mut self) {
        self.consecutive_failures += 1;

        if self.consecutive_failures >= 3 {
            self.open_until = Some(Instant::now() + Duration::from_secs(15));
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fallback = std::env::var("ETH_RPC_URL")
        .unwrap_or_else(|_| "https://ethereum-rpc.publicnode.com".to_string());

    let gateway = RpcGateway {
        providers: vec![
            BlockchainRpc::new("http://127.0.0.1:59999".to_string())?,
            BlockchainRpc::new(fallback)?,
        ],
        breakers: vec![
            Mutex::new(CircuitBreaker::new()),
            Mutex::new(CircuitBreaker::new()),
        ],
    };

    for request_number in 1..=5 {
        println!("\nRequest {request_number}");

        match gateway.latest_block().await {
            Ok(block) => println!("Latest block: {block}"),
            Err(error) => println!("Gateway error: {error}"),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_breaker_allows_requests() {
        let breaker = CircuitBreaker::new();

        assert!(breaker.allow_request());
    }

    #[test]
    fn breaker_opens_after_three_failures() {
        let mut breaker = CircuitBreaker::new();

        breaker.record_failure();
        breaker.record_failure();

        assert!(breaker.allow_request());

        breaker.record_failure();

        assert!(!breaker.allow_request());
    }

    #[test]
    fn successful_request_resets_breaker() {
        let mut breaker = CircuitBreaker::new();

        for _ in 0..3 {
            breaker.record_failure();
        }

        assert!(!breaker.allow_request());

        breaker.record_success();

        assert!(breaker.allow_request());
        assert_eq!(breaker.consecutive_failures, 0);
    }

    #[test]
    fn expired_cooldown_allows_probe() {
        let mut breaker = CircuitBreaker::new();

        breaker.record_failure();
        breaker.record_failure();
        breaker.record_failure();

        breaker.open_until = Some(Instant::now() - Duration::from_secs(1));

        assert!(breaker.allow_request());
    }
}
