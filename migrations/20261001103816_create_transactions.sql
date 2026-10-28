CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TYPE transaction_status AS ENUM (
    'CREATED',
    'PENDING',
    'PROCESSING',
    'RETRYING',
    'SUCCESS',
    'FAILED',
    'UNKNOWN',
    'RECONCILING'
);

CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    amount BIGINT NOT NULL,
    from_address VARCHAR(44) NOT NULL,
    to_address VARCHAR(44) NOT NULL,
    status transaction_status NOT NULL DEFAULT 'CREATED',
    idempotency_key VARCHAR(255) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);
