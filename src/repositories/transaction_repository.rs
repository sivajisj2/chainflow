use crate::domain::transaction::{Transaction, TransactionRow, TransactionStatus};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct TransactionRepository {
    pool: PgPool,
}

impl TransactionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_transaction_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<Transaction>, sqlx::Error> {
        let row = sqlx::query_as::<_, TransactionRow>(
            r#"
            SELECT
                id,
                amount,
                from_address,
                to_address,
                status,
                idempotency_key
            FROM
                transactions
            WHERE
                idempotency_key = $1
            "#,
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn get_transaction_by_id(
        &self,
        id: &Uuid,
    ) -> Result<Option<Transaction>, sqlx::Error> {
        let row = sqlx::query_as::<_, TransactionRow>(
            r#"
            SELECT 
            id,
            amount,
            from_address,
            to_address,
            status,
            idempotency_key
            FROM 
            transactions WHERE  id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.into()))
    }

    pub async fn create_transaction(&self, tx: Transaction) -> Result<Transaction, sqlx::Error> {
        let row = sqlx::query_as::<_, TransactionRow>(
            r#"
            INSERT INTO transactions
                (id, amount, from_address, to_address, status, idempotency_key)
            VALUES
                ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (idempotency_key) DO NOTHING
            RETURNING
                id,
                amount,
                from_address,
                to_address,
                status,
                idempotency_key
            "#,
        )
        .bind(tx.id)
        .bind(tx.amount)
        .bind(&tx.from)
        .bind(&tx.to)
        .bind(&tx.status)
        .bind(&tx.idempotency_key)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            return Ok(row.into());
        }

        if let Some(existing_tx) = self
            .get_transaction_by_idempotency_key(&tx.idempotency_key)
            .await?
        {
            return Ok(existing_tx);
        }

        Err(sqlx::Error::RowNotFound)
    }

    pub async fn update_transaction_status(
        &self,
        id: &Uuid,
        current_status: &TransactionStatus,
        new_status: &TransactionStatus,
    ) -> Result<Option<Transaction>, sqlx::Error> {
        let row = sqlx::query_as::<_, TransactionRow>(
            r#"
        UPDATE transactions
        SET status = $1
        WHERE id = $2 AND status = $3
        RETURNING
            id,
            amount,
            from_address,
            to_address,
            status,
            idempotency_key
        "#,
        )
        .bind(new_status) // $1
        .bind(id) // $2
        .bind(current_status) // $3
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn transition_status_with_lock(
        &self,
        id: uuid::Uuid,
        expected_status: TransactionStatus,
        next_status: TransactionStatus,
    ) -> Result<Option<Transaction>, sqlx::Error> {
        let mut db_tx = self.pool.begin().await?;

        // 1. Find the row and lock it until commit or rollback.
        let row = sqlx::query_as::<_, TransactionRow>(
            r#"
        SELECT
            id,
            amount,
            from_address,
            to_address,
            status,
            idempotency_key
        FROM transactions
        WHERE id = $1
        FOR UPDATE
        "#,
        )
        .bind(id)
        .fetch_optional(&mut *db_tx)
        .await?;

        let Some(row) = row else {
            db_tx.commit().await?;
            return Ok(None);
        };

        // 2. Check the actual state after acquiring the lock.
        if row.status != expected_status {
            db_tx.commit().await?;
            return Ok(None);
        }

        // 3. Update only if the expected state still matches.
        let updated_row = sqlx::query_as::<_, TransactionRow>(
            r#"
        UPDATE transactions
        SET status = $1,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = $2
          AND status = $3
        RETURNING
            id,
            amount,
            from_address,
            to_address,
            status,
            idempotency_key
        "#,
        )
        .bind(next_status)
        .bind(id)
        .bind(expected_status)
        .fetch_optional(&mut *db_tx)
        .await?;

        // 4. Commit the operation before returning.
        db_tx.commit().await?;

        Ok(updated_row.map(Into::into))
    }
}
