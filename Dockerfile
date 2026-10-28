# Stage 1: Build the Rust application

FROM rust:1-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*
    
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY src ./src
COPY proto ./proto
COPY migrations ./migrations

RUN cargo build --release --bin chainflow

# Stage 2: Runtime the compiled binary
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/chainflow /usr/local/bin/chainflow
COPY --from=builder /app/migrations ./migrations

ENV RUST_LOG=info

EXPOSE 3000

USER 65532:65532

CMD ["chainflow"]
