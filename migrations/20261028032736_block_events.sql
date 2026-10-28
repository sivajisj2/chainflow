-- Add migration script here
CREATE TABLE IF NOT EXISTS indexed_blocks (
    block_number BIGINT PRIMARY KEY,
    block_hash TEXT NOT NULL,
    parent_hash TEXT NOT NULL,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS indexed_events (
    block_number BIGINT NOT NULL,
    block_hash TEXT NOT NULL,
    transaction_hash TEXT NOT NULL,
    log_index INTEGER NOT NULL,
    contract_address TEXT NOT NULL,
    topic0 TEXT NOT NULL,
    data TEXT NOT NULL,
    PRIMARY KEY (block_hash, log_index)
);

CREATE INDEX IF NOT EXISTS idx_events_block_number
ON indexed_events(block_number);

CREATE TABLE IF NOT EXISTS indexer_checkpoints (
    indexer_name TEXT PRIMARY KEY,
    last_processed_block BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);