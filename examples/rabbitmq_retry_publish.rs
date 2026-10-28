use lapin::{
    options::{BasicPublishOptions, ConfirmSelectOptions},
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

    channel
        .confirm_select(ConfirmSelectOptions::default())
        .await?;

    let payload = br#"{"transaction_id":"txn-retry-001","event":"process_transaction"}"#;

    let confirmation = channel
        .basic_publish(
            "chainflow.retry.exchange".into(),
            "process".into(),
            BasicPublishOptions::default(),
            payload,
            BasicProperties::default()
                .with_delivery_mode(2)
                .with_headers(FieldTable::default()),
        )
        .await?
        .await?;

    match confirmation {
        Confirmation::Ack(_) => println!("Test job published"),
        other => return Err(format!("Publish not confirmed: {other:?}").into()),
    }

    Ok(())
}
