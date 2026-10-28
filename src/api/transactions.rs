use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::domain::transaction::TransactionStatus;
use crate::state::AppState;

#[derive(Deserialize, Debug)]
pub struct CreateTransaction {
    pub amount: u64,
    pub from: String,
    pub to: String,
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub id: String,
    pub amount: u64,
    pub from: String,
    pub to: String,
    pub status: TransactionStatus,
    pub idempotency_key: String,
}

pub async fn health() -> &'static str {
    "OK"
}

pub async fn create_transaction(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<CreateTransaction>,
) -> Result<Json<TransactionResponse>, (StatusCode, &'static str)> {
    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|val| val.to_str().ok());

    let key = idempotency_key.ok_or((StatusCode::BAD_REQUEST, "Missing Idempotency-Key header"))?;

    let tx = state
        .transaction_service
        .create_transaction(body.amount, &body.from, &body.to, key)
        .await
        .map_err(|error| {
            println!("Database error: {:?}", error);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create transaction",
            )
        })?;

    Ok(Json(TransactionResponse {
        id: tx.id.to_string(),
        amount: tx.amount as u64,
        from: tx.from,
        to: tx.to,
        status: tx.status,
        idempotency_key: tx.idempotency_key,
    }))
}
