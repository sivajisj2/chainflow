use async_rs::Runtime;
use futures_lite::stream::StreamExt;
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, ConfirmSelectOptions,
        QueueDeclareOptions,
    },
    types::FieldTable,
    BasicProperties, Confirmation, Connection, ConnectionProperties,
};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".to_string());

    let runtime = Runtime::tokio_current();

    let connection =
        Connection::connect_with_runtime(&addr, ConnectionProperties::default(), runtime).await?;

    let channel = connection.create_channel().await?;

    let queue_name = "chainflow.transaction_jobs";

    channel
        .queue_declare(
            queue_name.into(),
            QueueDeclareOptions::durable(),
            FieldTable::default(),
        )
        .await?;

    // Ask RabbitMQ to confirm published messages.
    channel
        .confirm_select(ConfirmSelectOptions::default())
        .await?;

    let payload = br#"{
        "transaction_id": "txn-demo-001",
        "event": "process_transaction"
    }"#;

    let confirmation = channel
        .basic_publish(
            "".into(),
            queue_name.into(),
            BasicPublishOptions::default(),
            payload,
            BasicProperties::default()
                .with_delivery_mode(2)
                .with_content_type("application/json".into()),
        )
        .await?
        .await?;

    match confirmation {
        Confirmation::Ack(_) => println!("Publisher confirmed the message"),
        other => {
            return Err(format!("Unexpected publisher confirmation: {other:?}").into());
        }
    }

    println!("Published transaction job");

    let mut consumer = channel
        .basic_consume(
            queue_name.into(),
            "chainflow-worker".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    if let Some(delivery_result) = consumer.next().await {
        let delivery = delivery_result?;

        let body = String::from_utf8_lossy(&delivery.data);
        println!("Received job: {body}");

        // Demo processing. A real worker must persist its outcome first.
        let processed = !delivery.data.is_empty();

        if processed {
            delivery.ack(BasicAckOptions::default()).await?;
            println!("Job processed and acknowledged");
        } else {
            println!("Processing failed; leaving the message unacknowledged");
        }
    }

    Ok(())
}
