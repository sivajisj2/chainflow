use sqlx::PgPool;
use std::error::Error;

async fn was_processed(pool: &PgPool, message_id: &str) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1
            FROM processed_messages
            WHERE message_id = $1
        )",
    )
    .bind(message_id)
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

async fn mark_as_processed(pool: &PgPool, message_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO processed_messages (message_id)
         VALUES ($1)
         ON CONFLICT (message_id) DO NOTHING",
    )
    .bind(message_id)
    .execute(pool)
    .await?;

    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database_url).await?;

    let message_id = "txn-101";

    if was_processed(&pool, message_id).await? {
        println!("Duplicate detected: skip processing");
    } else {
        println!("New message: processing is required");

        // Simulate the message-processing step.
        println!("Processing message: {message_id}");

        mark_as_processed(&pool, message_id).await?;

        println!("Message recorded as processed");
    }

    pool.close().await;
    Ok(())
}
