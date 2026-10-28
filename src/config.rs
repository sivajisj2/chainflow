use std::time::Duration;

use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn create_pool() -> PgPool {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    PgPoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(300)))
        .max_lifetime(Some(Duration::from_secs(1800)))
        .connect(&db_url)
        .await
        .expect("Failed to connect to database")
}
