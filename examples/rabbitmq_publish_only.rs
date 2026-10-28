use lapin::{
    options::{BasicPublishOptions, ConfirmSelectOptions, QueueDeclareOptions},
    types::FieldTable,
    BasicProperties, Confirmation, Connection, ConnectionProperties,
};
use std::error::Error;

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

    channel
        .confirm_select(ConfirmSelectOptions::default())
        .await?;

    let payload = br#"{"transaction_id":"txn-demo-002","event":"process_transaction"}"#;

    let confirmation = channel
        .basic_publish(
            "".into(),
            queue.into(),
            BasicPublishOptions::default(),
            payload,
            BasicProperties::default().with_delivery_mode(2),
        )
        .await?
        .await?;

    match confirmation {
        Confirmation::Ack(_) => println!("Message confirmed and queued"),
        other => return Err(format!("Unexpected confirmation: {other:?}").into()),
    }

    Ok(())
}
