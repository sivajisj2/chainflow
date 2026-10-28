use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

async fn fetch_latest_block(
    client: &Client,
    rpc_url: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_getBlockByNumber",
        "params": ["latest", false],
        "id": 1
    });

    let response: Value = client
        .post(rpc_url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if let Some(error) = response.get("error") {
        return Err(format!("RPC error: {error}").into());
    }

    let block = response
        .get("result")
        .filter(|value| !value.is_null())
        .ok_or("Missing block result")?;

    Ok(block.clone())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rpc_url = std::env::var("ETH_RPC_URL")
        .unwrap_or_else(|_| "https://ethereum-rpc.publicnode.com".to_string());

    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    let block = fetch_latest_block(&client, &rpc_url).await?;

    println!("Block number: {}", block["number"]);
    println!("Block hash: {}", block["hash"]);
    println!("Parent hash: {}", block["parentHash"]);
    println!("Timestamp: {}", block["timestamp"]);

    let tx_count = block["transactions"]
        .as_array()
        .map(|transactions| transactions.len())
        .unwrap_or(0);

    println!("Transactions: {tx_count}");

    Ok(())
}
