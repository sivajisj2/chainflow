# ChainFlow Recovery Notes

Recovered state from the completed implementation.

## Verified database row

The project successfully inserted a row with:
- amount: 1000
- from_address: sender-address
- to_address: receiver-address
- status: CREATED
- idempotency_key: test-tx-001

## Errors already diagnosed and fixed

1. Insert was accidentally targeting the enum name instead of `transactions`.
2. `RETURNING` was misspelled.
3. API/domain `from` and `to` map to DB `from_address` and `to_address`.
4. `FromRow` was required for `query_as`.
5. `sqlx::Type` was required for the PostgreSQL enum.
6. Enum case initially mismatched (`created` vs `CREATED`).
7. PostgreSQL NUMERIC did not map directly to Rust i64.
8. We chose BIGINT + i64 for simple/native SQLx mapping.
9. The first transaction was successfully persisted.
10. Reusing `test-tx-001` produced PostgreSQL 23505 on the UNIQUE idempotency constraint.

## Immediate next step

Implement database-backed idempotency. Do not jump to Kafka/Redis/workers yet.
