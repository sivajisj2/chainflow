use futures_lite::stream::StreamExt;
use lapin::{
    options::{BasicConsumeOptions, QueueDeclareOptions},
    types::FieldTable,
    Connection, ConnectionProperties,
};
use std::{error::Error, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".to_string());

    let connection = Connection::connect(&addr, ConnectionProperties::default()).await?;

    let channel = connection.create_channel().await?;

    let queue = "chainflow.transaction_jobs";

    channel
        .queue_declare(
            queue.into(),
            QueueDeclareOptions::durable(),
            FieldTable::default(),
        )
        .await?;

    let mut consumer = channel
        .basic_consume(
            queue.into(),
            "chainflow-crash-worker".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    println!("Crash worker waiting for a message...");

    if let Some(result) = consumer.next().await {
        let delivery = result?;
        println!(
            "Received message: {}",
            String::from_utf8_lossy(&delivery.data)
        );

        println!("Simulating a crash before ACK...");
        tokio::time::sleep(Duration::from_secs(30)).await;
    }

    Ok(())
}
