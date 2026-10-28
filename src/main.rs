use axum::{
    routing::{get, post},
    Router,
};

use chainflow::{
    api::transactions::{create_transaction, health},
    config::create_pool,
    repositories::transaction_repository::TransactionRepository,
    services::transaction_service::TransactionService,
    state::AppState,
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let pool = create_pool().await;
    let repository = TransactionRepository::new(pool.clone());
    let transaction_service = TransactionService::new(repository);

    let app = Router::new()
        .route("/", get(|| async { "Hello axum" }))
        .route("/health", get(health))
        .route("/api/v1/transactions", post(create_transaction))
        .with_state(AppState {
            transaction_service,
        });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
