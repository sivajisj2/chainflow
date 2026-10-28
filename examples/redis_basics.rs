use redis::AsyncCommands;
use std::{error::Error, time::Duration};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = redis::Client::open("redis://127.0.0.1:6379/")?;
    let mut con = client.get_multiplexed_async_connection().await?;

    // Store a sample ChainFlow transaction status for 60 seconds.
    let _: () = con.set_ex("transaction:101", "pending", 200).await?;

    // Read the cached status.
    let status: Option<String> = con.get("transaction:101").await?;
    println!("Cached transaction status: {status:?}");

    // Check the remaining TTL.
    let ttl: i64 = con.ttl("transaction:102").await?;

    println!("Remaining TTL: {ttl} seconds");
    Ok(())
}
