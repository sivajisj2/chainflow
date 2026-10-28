use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

const TRANSFER_TOPIC: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rpc_url = std::env::var("ETH_RPC_URL")
        .unwrap_or_else(|_| "https://ethereum-rpc.publicnode.com".to_string());

    let client = Client::builder().timeout(Duration::from_secs(15)).build()?;

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_getLogs",
        "params": [{
            "fromBlock": "latest",
            "toBlock": "latest",
            "address": USDC,
            "topics": [TRANSFER_TOPIC]
        }],
        "id": 1
    });

    let response: Value = client
        .post(&rpc_url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if let Some(error) = response.get("error") {
        return Err(format!("RPC error: {error}").into());
    }

    let logs = response["result"].as_array().ok_or("Missing logs array")?;

    println!("Found {} USDC Transfer events", logs.len());

    for log in logs.iter().take(5) {
        println!("Transaction: {}", log["transactionHash"]);
        println!("Block: {}", log["blockNumber"]);
        println!("Log index: {}", log["logIndex"]);
        println!("Topics: {}", log["topics"]);
        println!("Data: {}", log["data"]);
        println!("--------------------------");
    }

    Ok(())
}
