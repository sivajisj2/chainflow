use axum::{
    middleware,
    routing::{get, post},
    Router,
};
mod metrics;
use chainflow::{
    api::transactions::{create_transaction, health},
    config::create_pool,
    repositories::transaction_repository::TransactionRepository,
    services::transaction_service::TransactionService,
    state::AppState,
};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info")),
        )
        .init();

    tracing::info!("Starting ChainFlow API");

    let pool = create_pool().await;
    let repository = TransactionRepository::new(pool.clone());
    let transaction_service = TransactionService::new(repository);

    let app = Router::new()
        .route("/", get(|| async { "Hello axum" }))
        .route("/health", get(health))
        .route("/api/v1/transactions", post(create_transaction))
        .route("/metrics", get(metrics::metrics_handler))
        .with_state(AppState {
            transaction_service,
        })
        .layer(middleware::from_fn(metrics::metrics_middleware))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
