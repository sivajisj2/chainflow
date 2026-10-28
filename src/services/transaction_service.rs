use axum::response::IntoResponse;

use crate::{
    domain::transaction::{Transaction, TransactionStatus},
    repositories::transaction_repository::TransactionRepository,
};

#[derive(Clone)]
pub struct TransactionService {
    repository: TransactionRepository,
}
#[derive(Debug, Clone)]
pub enum TransactionServiceError {
    Database(String),
    IdempotencyConflict,
    InvalidInput(String),
    InvalidTransition {
        current: TransactionStatus,
        next: TransactionStatus,
    },
    TransactionNotFound,
    StateConflict {
        current: TransactionStatus,
        expected: TransactionStatus,
    },
    DatabaseError(String),
}

impl IntoResponse for TransactionServiceError {
    fn into_response(self) -> axum::response::Response {
        match self {
            TransactionServiceError::Database(_) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Database error",
            )
                .into_response(),
            TransactionServiceError::IdempotencyConflict => (
                axum::http::StatusCode::CONFLICT,
                "Idempotency conflict: transaction with the same idempotency key already exists with different data",
            )
                .into_response(),
            TransactionServiceError::InvalidInput(msg) => (
                axum::http::StatusCode::BAD_REQUEST,
                msg,
            )
                .into_response(),
            TransactionServiceError::InvalidTransition { current, next } => (
                axum::http::StatusCode::BAD_REQUEST,
                format!("Invalid transition from {} to {}", current, next),
            )
                .into_response(),
            TransactionServiceError::TransactionNotFound => (
                axum::http::StatusCode::NOT_FOUND,
                "Transaction not found",
            )
                .into_response(),
            TransactionServiceError::StateConflict { current, expected } => (
                axum::http::StatusCode::CONFLICT,
                format!("State conflict: expected {}, got {}", expected, current),
            )
                .into_response(),
            TransactionServiceError::DatabaseError(msg) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                msg,
            )
                .into_response(),
        }
    }
}
impl TransactionService {
    pub fn new(repository: TransactionRepository) -> Self {
        Self { repository }
    }

    pub async fn create_transaction(
        &self,
        amount: u64,
        from: &str,
        to: &str,
        idempotency_key: &str,
    ) -> Result<Transaction, TransactionServiceError> {
        let amount = i64::try_from(amount).map_err(|_| {
            TransactionServiceError::InvalidInput("Amount exceeds i64 range".to_string())
        })?;

        let transaction = Transaction {
            id: uuid::Uuid::new_v4(),
            amount,
            from: from.to_string(),
            to: to.to_string(),
            status: TransactionStatus::CREATED,
            idempotency_key: idempotency_key.to_string(),
        };

        let tx1 = self
            .repository
            .create_transaction(transaction)
            .await
            .map_err(|err| TransactionServiceError::Database(err.to_string()))?;

        if tx1.amount == amount
            && tx1.from == from
            && tx1.to == to
            && tx1.idempotency_key == idempotency_key
        {
            Ok(tx1)
        } else {
            Err(TransactionServiceError::IdempotencyConflict)
        }
    }

    pub async fn update_transaction_status(
        &self,
        id: &uuid::Uuid,
        current_status: &TransactionStatus,
        new_status: &TransactionStatus,
    ) -> Result<Transaction, TransactionServiceError> {
        // Step 1 — domain rule, no DB touched yet.
        if !current_status.can_transition_to(new_status.clone()) {
            return Err(TransactionServiceError::InvalidTransition {
                current: current_status.clone(),
                next: new_status.clone(),
            });
        }

        // Step 2 — conditional update.
        let updated = self
            .repository
            .transition_status_with_lock(*id, *current_status, *new_status)
            .await
            .map_err(|err| TransactionServiceError::Database(err.to_string()))?;

        if let Some(transaction) = updated {
            return Ok(transaction);
        }

        // Step 3 — no row updated: find out why.
        let existing = self
            .repository
            .get_transaction_by_id(id)
            .await
            .map_err(|err| TransactionServiceError::Database(err.to_string()))?;

        match existing {
            None => Err(TransactionServiceError::TransactionNotFound),
            Some(found) => Err(TransactionServiceError::StateConflict {
                current: found.status,
                expected: current_status.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::transaction_repository::TransactionRepository;
    use uuid::Uuid;

    fn sample_transaction(idempotency_key: &str, status: TransactionStatus) -> Transaction {
        Transaction {
            id: Uuid::new_v4(),
            amount: 100,
            from: "alice".to_string(),
            to: "bob".to_string(),
            status,
            idempotency_key: idempotency_key.to_string(),
        }
    }

    #[sqlx::test]
    async fn invalid_transition_is_rejected_before_touching_db(pool: sqlx::PgPool) {
        let service = TransactionService::new(TransactionRepository::new(pool));
        let id = Uuid::new_v4();

        let result = service
            .update_transaction_status(
                &id,
                &TransactionStatus::SUCCESS,
                &TransactionStatus::PROCESSING,
            )
            .await;

        assert!(matches!(
            result,
            Err(TransactionServiceError::InvalidTransition { .. })
        ));
    }

    #[sqlx::test]
    async fn missing_transaction_returns_not_found(pool: sqlx::PgPool) {
        let service = TransactionService::new(TransactionRepository::new(pool));
        let missing_id = Uuid::new_v4();

        let result = service
            .update_transaction_status(
                &missing_id,
                &TransactionStatus::PROCESSING,
                &TransactionStatus::SUCCESS,
            )
            .await;

        assert!(matches!(
            result,
            Err(TransactionServiceError::TransactionNotFound)
        ));
    }

    #[sqlx::test]
    async fn stale_expected_status_returns_state_conflict(pool: sqlx::PgPool) {
        let repository = TransactionRepository::new(pool);
        let tx = repository
            .create_transaction(sample_transaction(
                "key-conflict",
                TransactionStatus::SUCCESS,
            ))
            .await
            .unwrap();

        let service = TransactionService::new(repository);

        let result = service
            .update_transaction_status(
                &tx.id,
                &TransactionStatus::PROCESSING,
                &TransactionStatus::FAILED,
            )
            .await;

        assert!(matches!(
            result,
            Err(TransactionServiceError::StateConflict {
                current: TransactionStatus::SUCCESS,
                ..
            })
        ));
    }

    #[sqlx::test]
    async fn matching_update_succeeds(pool: sqlx::PgPool) {
        let repository = TransactionRepository::new(pool);
        let tx = repository
            .create_transaction(sample_transaction(
                "key-success",
                TransactionStatus::PROCESSING,
            ))
            .await
            .unwrap();

        let service = TransactionService::new(repository);

        let result = service
            .update_transaction_status(
                &tx.id,
                &TransactionStatus::PROCESSING,
                &TransactionStatus::SUCCESS,
            )
            .await
            .unwrap();

        assert_eq!(result.status, TransactionStatus::SUCCESS);
    }

    #[sqlx::test]
    async fn database_failure_is_distinguishable_from_business_errors(pool: sqlx::PgPool) {
        let repository = TransactionRepository::new(pool.clone());
        let service = TransactionService::new(repository);

        pool.close().await; // force every subsequent query to fail

        let result = service
            .update_transaction_status(
                &Uuid::new_v4(),
                &TransactionStatus::PROCESSING,
                &TransactionStatus::SUCCESS,
            )
            .await;

        assert!(matches!(result, Err(TransactionServiceError::Database(_))));
    }
    #[sqlx::test]
    async fn concurrent_writers_only_one_succeeds(pool: sqlx::PgPool) {
        let repository = TransactionRepository::new(pool.clone());
        let tx = repository
            .create_transaction(sample_transaction(
                "key-race",
                TransactionStatus::PROCESSING,
            ))
            .await
            .unwrap();

        let service = std::sync::Arc::new(TransactionService::new(repository));
        let id = tx.id;

        let service_a = service.clone();
        let service_b = service.clone();

        let worker_a = tokio::spawn(async move {
            service_a
                .update_transaction_status(
                    &id,
                    &TransactionStatus::PROCESSING,
                    &TransactionStatus::SUCCESS,
                )
                .await
        });
        let worker_b = tokio::spawn(async move {
            service_b
                .update_transaction_status(
                    &id,
                    &TransactionStatus::PROCESSING,
                    &TransactionStatus::FAILED,
                )
                .await
        });

        let (result_a, result_b) = tokio::join!(worker_a, worker_b);
        let result_a = result_a.unwrap();
        let result_b = result_b.unwrap();

        let successes = [result_a.is_ok(), result_b.is_ok()]
            .into_iter()
            .filter(|ok| *ok)
            .count();
        assert_eq!(
            successes, 1,
            "exactly one competing transition should succeed"
        );

        let verification_repo = TransactionRepository::new(pool);
        let final_tx = verification_repo
            .get_transaction_by_id(&id)
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(
            final_tx.status,
            TransactionStatus::SUCCESS | TransactionStatus::FAILED
        ));
    }
}
