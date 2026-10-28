# ChainFlow Recovery Snapshot

This restores the implementation through the point where a transaction was successfully persisted to PostgreSQL.

## Completed

- Rust fundamentals practiced
- Axum server and health endpoint
- POST transaction endpoint
- JSON request/response
- UUID transaction IDs
- TransactionStatus enum/state model
- Idempotency-Key extraction
- Layered API/domain/service/repository architecture
- AppState dependency injection
- PostgreSQL + SQLx + PgPool
- PostgreSQL custom ENUM and SQLx Type mapping
- SQLx FromRow
- Repository INSERT + RETURNING
- BIGINT ↔ i64 database mapping
- Successful database persistence
- Duplicate idempotency key reproduced as PostgreSQL error 23505

## Last verified behavior

The first request using `test-tx-001` inserted a transaction.

Repeating the same key currently produces the unique constraint error. Proper idempotency behavior is the next milestone.

## Next milestone

Implement database-backed idempotency and reason through:
- existing key lookup
- duplicate requests
- same key with different payload
- `ON CONFLICT`
- concurrent requests/races
