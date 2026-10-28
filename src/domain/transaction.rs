use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::fmt;
use uuid::Uuid;

#[derive(Serialize, Debug)]
pub struct Transaction {
    pub id: Uuid,
    pub amount: i64,
    pub from: String,
    pub to: String,
    pub status: TransactionStatus,
    pub idempotency_key: String,
}

#[derive(sqlx::FromRow, Debug)]
pub struct TransactionRow {
    pub id: Uuid,
    pub amount: i64,
    pub from_address: String,
    pub to_address: String,
    pub status: TransactionStatus,
    pub idempotency_key: String,
}

impl From<TransactionRow> for Transaction {
    fn from(row: TransactionRow) -> Self {
        Self {
            id: row.id,
            amount: row.amount,
            from: row.from_address,
            to: row.to_address,
            status: row.status,
            idempotency_key: row.idempotency_key,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, Type, PartialEq, Eq)]
#[sqlx(type_name = "transaction_status", rename_all = "UPPERCASE")]
pub enum TransactionStatus {
    CREATED,
    PENDING,
    PROCESSING,
    RETRYING,
    SUCCESS,
    FAILED,
    UNKNOWN,
    RECONCILING,
}

impl fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            TransactionStatus::CREATED => "CREATED",
            TransactionStatus::PENDING => "PENDING",
            TransactionStatus::PROCESSING => "PROCESSING",
            TransactionStatus::RETRYING => "RETRYING",
            TransactionStatus::SUCCESS => "SUCCESS",
            TransactionStatus::FAILED => "FAILED",
            TransactionStatus::UNKNOWN => "UNKNOWN",
            TransactionStatus::RECONCILING => "RECONCILING",
        };
        write!(f, "{value}")
    }
}

impl TransactionStatus {
    pub fn can_transition_to(&self, next: TransactionStatus) -> bool {
        match self {
            TransactionStatus::CREATED => {
                matches!(next, TransactionStatus::PENDING | TransactionStatus::FAILED)
            }
            TransactionStatus::PENDING => matches!(
                next,
                TransactionStatus::PROCESSING | TransactionStatus::FAILED
            ),
            TransactionStatus::PROCESSING => matches!(
                next,
                TransactionStatus::SUCCESS
                    | TransactionStatus::FAILED
                    | TransactionStatus::RETRYING
                    | TransactionStatus::UNKNOWN
            ),
            TransactionStatus::RETRYING => matches!(
                next,
                TransactionStatus::PROCESSING
                    | TransactionStatus::FAILED
                    | TransactionStatus::UNKNOWN
            ),
            TransactionStatus::UNKNOWN => matches!(next, TransactionStatus::RECONCILING),
            TransactionStatus::FAILED => matches!(next, TransactionStatus::RECONCILING),
            TransactionStatus::SUCCESS => matches!(next, TransactionStatus::RECONCILING),
            TransactionStatus::RECONCILING => matches!(
                next,
                TransactionStatus::SUCCESS
                    | TransactionStatus::FAILED
                    | TransactionStatus::UNKNOWN
                    | TransactionStatus::RETRYING
            ),
        }
    }
}
