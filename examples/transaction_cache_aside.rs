use redis::AsyncCommands;
use sqlx::PgPool;
use std::error::Error;
use uuid::Uuid;

const CACHE_TTL_SECONDS: u64 = 30;

async fn get_transaction_status(
    pool: &PgPool,
    redis_client: &redis::Client,
    transaction_id: Uuid,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    let key = format!("transaction:{transaction_id}:status");

    // 1. Try Redis. A cache failure must not prevent a DB read.
    let mut connection = match redis_client.get_multiplexed_async_connection().await {
        Ok(connection) => Some(connection),
        Err(error) => {
            eprintln!("Redis unavailable; using PostgreSQL: {error}");
            None
        }
    };

    if let Some(ref mut con) = connection {
        let cached: Result<Option<String>, redis::RedisError> = con.get(&key).await;

        match cached {
            Ok(Some(status)) => {
                println!("Cache HIT: {status}");
                return Ok(Some(status));
            }
            Ok(None) => println!("Cache MISS"),
            Err(error) => {
                eprintln!("Redis read failed; using PostgreSQL: {error}");
            }
        }
    }

    // 2. Cache miss or Redis failure: query PostgreSQL.
    let status =
        sqlx::query_scalar::<_, String>("SELECT status::text FROM transactions WHERE id = $1")
            .bind(transaction_id)
            .fetch_optional(pool)
            .await?;

    // 3. Populate Redis if a row exists.
    // Cache-write failure should not fail an otherwise successful DB read.
    if let (Some(status), Some(ref mut con)) = (status.as_ref(), connection.as_mut()) {
        let result: Result<(), redis::RedisError> =
            con.set_ex(&key, status.as_str(), CACHE_TTL_SECONDS).await;

        if let Err(error) = result {
            eprintln!("Redis write failed: {error}");
        } else {
            println!("Cached status for {CACHE_TTL_SECONDS} seconds");
        }
    }

    Ok(status)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")?;
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string());

    let transaction_id: Uuid = std::env::var("TRANSACTION_ID")?.parse()?;

    let pool = PgPool::connect(&database_url).await?;
    let redis_client = redis::Client::open(redis_url)?;

    // First call: normally a cache miss.
    let first = get_transaction_status(&pool, &redis_client, transaction_id).await?;
    println!("First result: {first:?}");

    // Second call: normally a cache hit.
    let second = get_transaction_status(&pool, &redis_client, transaction_id).await?;
    println!("Second result: {second:?}");

    pool.close().await;
    Ok(())
}
