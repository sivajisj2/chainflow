use futures_lite::stream::StreamExt;
use lapin::{
    options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, ConfirmSelectOptions},
    types::{AMQPValue, FieldTable, ShortString},
    BasicProperties, Confirmation, Connection, ConnectionProperties,
};
use std::{error::Error, time::Duration};

const MAIN_QUEUE: &str = "chainflow.retry.main";
const RETRY_EXCHANGE: &str = "chainflow.retry.delay.exchange";
const DLQ_EXCHANGE: &str = "chainflow.retry.dlx";
const MAX_ATTEMPTS: u64 = 3;

fn retry_properties(attempt: u64) -> BasicProperties {
    let mut headers = FieldTable::default();
    headers.insert(
        ShortString::from("x-retry-attempt"),
        AMQPValue::LongLongInt(attempt as i64),
    );

    BasicProperties::default()
        .with_delivery_mode(2)
        .with_content_type("application/json".into())
        .with_headers(headers)
}

async fn publish_confirmed(
    channel: &lapin::Channel,
    exchange: &str,
    routing_key: &str,
    payload: &[u8],
    properties: BasicProperties,
) -> Result<(), Box<dyn Error>> {
    let confirmation = channel
        .basic_publish(
            exchange.into(),
            routing_key.into(),
            BasicPublishOptions::default(),
            payload,
            properties,
        )
        .await?
        .await?;

    match confirmation {
        Confirmation::Ack(_) => Ok(()),
        other => Err(format!("Publish not confirmed: {other:?}").into()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".to_string());

    let connection = Connection::connect(&addr, ConnectionProperties::default()).await?;
    let channel = connection.create_channel().await?;

    channel
        .confirm_select(ConfirmSelectOptions::default())
        .await?;

    let mut consumer = channel
        .basic_consume(
            MAIN_QUEUE.into(),
            "chainflow-retry-worker".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    println!("Retry worker waiting for jobs...");

    while let Some(result) = consumer.next().await {
        let delivery = result?;
        let payload = delivery.data.clone();

        let attempt = delivery
            .properties
            .headers()
            .as_ref()
            .and_then(|headers| headers.inner().get("x-retry-attempt"))
            .and_then(|value| match value {
                AMQPValue::LongLongInt(n) => Some(*n as u64),
                AMQPValue::LongInt(n) => Some(*n as u64),
                _ => None,
            })
            .unwrap_or(0);

        println!("Processing job; previous retry count: {attempt}");

        // Deliberately fail to exercise the retry path.
        let processing_succeeded = false;

        if processing_succeeded {
            delivery.ack(BasicAckOptions::default()).await?;
            println!("Job succeeded and was acknowledged");
        } else if attempt < MAX_ATTEMPTS {
            let next_attempt = attempt + 1;
            println!("Simulated failure; publishing retry {next_attempt}");

            publish_confirmed(
                &channel,
                RETRY_EXCHANGE,
                "retry",
                &payload,
                retry_properties(next_attempt),
            )
            .await?;

            delivery.ack(BasicAckOptions::default()).await?;
            println!("Retry published and original acknowledged");
        } else {
            println!("Retry limit reached; sending job to DLQ");

            publish_confirmed(
                &channel,
                DLQ_EXCHANGE,
                "dead",
                &payload,
                retry_properties(attempt),
            )
            .await?;

            delivery.ack(BasicAckOptions::default()).await?;
            println!("Job sent to DLQ; original acknowledged");
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(())
}
