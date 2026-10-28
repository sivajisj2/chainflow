# ChainFlow

ChainFlow is a Rust/Axum transaction API used to practice production backend engineering: PostgreSQL persistence, idempotency, state transitions, Docker, Kubernetes, CI, structured logging, and Prometheus metrics.

## Capabilities

- Axum HTTP API on port `3000`
- PostgreSQL persistence through SQLx
- UUID transaction identifiers
- Required `Idempotency-Key` handling
- Conditional transaction-state transitions
- gRPC/protobuf build support through `tonic` and `prost`
- JSON tracing and Prometheus metrics
- Docker Compose development stack
- Kubernetes Deployment and Service examples
- GitHub Actions Rust and Docker validation

## Architecture

```text
HTTP request
    -> Axum routes and middleware
    -> API handlers
    -> TransactionService (validation and business rules)
    -> TransactionRepository (SQLx/PostgreSQL)
    -> PostgreSQL
```

`src/domain` contains domain types, `src/api` contains HTTP handlers, `src/services` contains business rules, and `src/repositories` contains database access. `src/metrics.rs` exposes `/metrics`. `src/auth.rs` contains an API-key middleware example; it is not enabled by default.

## Prerequisites

- Rust stable and Cargo
- Docker and Docker Compose
- Protobuf compiler (`protoc`) for local builds
- Optional: `kubectl` and Minikube for Kubernetes exercises

## Local Setup

Start dependencies:

```bash
docker compose up -d postgres redis rabbitmq
```

Create a local `.env` file. It is ignored by Git:

```dotenv
DATABASE_URL=postgres://postgres:password@localhost:5433/chainflow
```

Run the application locally:

```bash
cargo run --bin chainflow
```

Or build and run the complete Compose service:

```bash
docker build -t chainflow:dev .
docker compose up -d --force-recreate chainflow
```

## API Examples

Health check:

```bash
curl http://localhost:3000/health
```

Metrics:

```bash
curl http://localhost:3000/metrics
```

Create a transaction:

```bash
curl -X POST http://localhost:3000/api/v1/transactions \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: demo-001' \
  -d '{"amount":100,"from":"alice","to":"bob"}'
```

The same key and identical payload can safely be retried. Reusing it with different transaction data returns a conflict.

## Verification

Run the same checks used by CI:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --bin chainflow
docker build --check .
docker compose config
```

## Observability

Compose includes Prometheus on `http://localhost:9090` and Grafana on `http://localhost:3001`. Prometheus scrapes `http://chainflow:3000/metrics` over the Compose network.

If `/metrics` returns `404`, the running image is stale. Rebuild and recreate the service:

```bash
docker build -t chainflow:dev .
docker compose up -d --force-recreate chainflow
curl -i http://localhost:3000/metrics
curl -s http://localhost:9090/api/v1/targets
```

## Kubernetes

The manifest in `k8s/chainflow.yaml` expects an image named `chainflow:dev` and a Secret named `chainflow-secrets` containing `DATABASE_URL`.

```bash
minikube start --driver=docker
minikube image load chainflow:dev
kubectl create secret generic chainflow-secrets \
  --from-literal=DATABASE_URL='postgres://USER:PASSWORD@REACHABLE_DB_HOST:5432/chainflow'
kubectl apply -f k8s/chainflow.yaml
kubectl rollout status deployment/chainflow
kubectl port-forward service/chainflow 3001:3000
curl http://localhost:3001/health
```

`localhost` inside a Kubernetes Pod means the Pod itself. Use a PostgreSQL Service DNS name or a database endpoint reachable from the cluster. The example manifest does not deploy PostgreSQL.

## Security Notes

- Never commit `.env`, tokens, private keys, or production credentials.
- Compose passwords are development-only placeholders; use environment variables or a secret manager for real deployments.
- Kubernetes Secret values require RBAC and encryption-at-rest controls in production.
- `src/auth.rs` demonstrates constant-time API-key comparison but must be explicitly wired into protected routes.
- Do not log database URLs, API keys, authorization headers, or sensitive request bodies.

## Interview Preparation

`Design.md` contains 100 questions and answers covering Rust, Axum, SQLx, PostgreSQL, idempotency, concurrency, messaging, Docker, Kubernetes, observability, security, testing, and system design.

## License

This repository is an educational ChainFlow backend exercise.
