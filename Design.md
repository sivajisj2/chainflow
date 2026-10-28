# ChainFlow Backend Interview Guide

This is an interview workbook built around the ChainFlow codebase. A strong interview answer should do four things: define the concept, connect it to this system, explain a trade-off, and name a failure or test case. The answers below are intentionally phrased as responses you could give aloud, not as dictionary definitions.

## Rust Fundamentals and Design

1. **Q: Why use Rust for a transaction service?**  
   **A:** I would choose Rust when correctness, predictable latency, and safe concurrency matter as much as raw throughput. Ownership prevents an entire class of memory bugs, while the type system makes states and errors explicit. The trade-off is a steeper learning curve and longer compile times than a garbage-collected language. In ChainFlow, that trade-off is worthwhile because transaction processing must not silently corrupt shared state.

2. **Q: What does ownership protect us from?**  
   **A:** Ownership gives every value one clear lifetime and prevents use-after-free, double-free, and data races at compile time. Borrowing lets a function inspect data without taking ownership, while moving transfers responsibility. In this project, request fields can be borrowed by the service and cloned only when they must outlive the request. That is safer than relying on runtime discipline.

3. **Q: What is the difference between `String` and `&str`?**  
   **A:** `String` owns heap-allocated UTF-8 data and can grow; `&str` is a borrowed view into UTF-8 data owned elsewhere. I use `&str` for read-only service inputs and convert to `String` when constructing a persistent `Transaction`. That avoids unnecessary allocation at API boundaries while still giving the domain object independent ownership.

4. **Q: Why does `TransactionStatus` derive `Copy`?**  
   **A:** `TransactionStatus` is a small enum with no heap allocation, so copying it is cheap and semantically clear. Deriving `Copy` lets the transition logic compare or store current and next states without moving them out of a borrowed transaction. I would not derive `Copy` for a large or resource-owning type because copying could hide an expensive operation.

5. **Q: When should a function return `Result`?**  
   **A:** I return `Result` when the caller can recover, classify, retry, or report the failure. Database timeouts, validation failures, and state conflicts are expected operational outcomes, so they should not panic. A panic may be defensible for an impossible programmer invariant, but `create_pool()` currently panics on startup failure and should eventually use bounded retries or a controlled startup error.

6. **Q: What is the difference between `Option` and `Result`?**  
   **A:** `Option<T>` says that a value may be absent, with no error explanation. `Result<T, E>` says an operation either succeeded or failed with a reason that can be mapped, logged, retried, or returned to a caller. Repository lookups naturally use `Option`, while database operations use `Result`; combining them lets the service distinguish “not found” from “database unavailable.”

7. **Q: What does `async` change in Rust?**  
   **A:** An async function returns a future that can suspend at `.await`, allowing Tokio to run other tasks while network or database I/O is pending. It does not make CPU-heavy or blocking code automatically non-blocking. In ChainFlow, SQLx and Axum are async, but a blocking cryptographic or file operation should move to `spawn_blocking` or a dedicated worker pool.

8. **Q: Why use `Arc` or clone application state?**  
   **A:** Axum serves many requests concurrently, so handlers need shared dependencies that are safe to access from multiple tasks. Connection pools and service handles are cheap to clone because they internally share resources. I avoid putting mutable business state in a global variable; if mutable state is required, I choose a deliberate synchronization strategy and measure contention.

9. **Q: What is a trait useful for in this project?**  
   **A:** A trait defines behavior independently of one implementation. For example, a payment provider, clock, repository, or message publisher can have a production implementation and a deterministic test double. The current repository is concrete, which is acceptable for a small project, but a trait becomes valuable when external dependencies make service tests slow or nondeterministic.

10. **Probe: How would you make an amount type-safe?**  
    **A:** I would create a validated newtype such as `Amount(i64)` with a constructor that rejects zero, negative values, and overflow before the value reaches SQL. The type could expose formatting and conversion methods while keeping the inner integer private. This prevents one caller from accidentally passing database units where another expects smallest blockchain units.

## Axum and HTTP

11. **Q: Why separate routes, handlers, services, and repositories?**  
    **A:** The separation keeps transport concerns from becoming business rules. A handler parses headers and JSON, the service validates invariants and orchestrates work, and the repository performs SQL. That makes a state-transition test independent of HTTP and lets a future gRPC handler reuse the same service. The trade-off is more types and files, but changes become easier to localize.

12. **Q: What does `State<AppState>` do?**  
    **A:** `State<AppState>` is Axum’s typed dependency-injection mechanism. The router owns an `AppState`, and each handler extracts the service it needs without using global mutable state. This also makes startup wiring visible in `main.rs`: configuration creates a pool, the pool creates a repository, and the repository creates the service.

13. **Q: Why should handlers avoid business logic?**  
    **A:** Handlers should translate HTTP concepts into domain inputs and domain results back into HTTP responses. If a handler decides state transitions or duplicate behavior, those rules are difficult to reuse from workers and easy to bypass in another transport. Keeping them in `TransactionService` makes the rule testable with ordinary Rust values and keeps HTTP tests smaller.

14. **Q: What does a readiness probe measure?**  
    **A:** Readiness answers, “Should this instance receive traffic right now?” It should fail when a required dependency such as PostgreSQL is unavailable, but it should not restart the process. Liveness answers, “Is the process stuck and should Kubernetes restart it?” Mixing the two can create restart loops during a temporary database outage.

15. **Q: Why is the current `/health` endpoint limited?**  
    **A:** It returns `OK` as long as the HTTP process can answer, so it is a useful liveness signal but not a complete readiness check. If the database pool is broken, traffic can still reach the handler and fail later. I would add a separate readiness endpoint that performs a bounded `SELECT 1`, while keeping liveness dependency-light.

16. **Q: How should errors map to HTTP status codes?**  
    **A:** I map invalid input to 400, missing resources to 404, duplicate or state conflicts to 409, missing or invalid credentials to 401, authenticated-but-forbidden access to 403, and temporary dependency failures to 503 when the client can retry. Unexpected failures become 500 with a safe public message. The internal error should retain a correlation ID for operators.

17. **Q: Why should an API return a stable error schema?**  
    **A:** Clients should not parse changing human-readable strings to decide whether to retry. A response such as `{code, message, request_id, details}` gives clients stable behavior and gives support teams a way to find the request in logs. The public message must not contain SQL errors, credentials, stack traces, or internal hostnames.

18. **Q: What is middleware?**  
    **A:** Middleware wraps request processing and is ideal for behavior that applies across handlers: authentication, tracing, metrics, timeouts, compression, and request IDs. Ordering matters. For example, a request ID should be created before logging, authentication should run before protected business routes, and a timeout should cover the full downstream operation.

19. **Q: Why use `TraceLayer`?**  
    **A:** `TraceLayer` produces consistent request spans containing method, route, status, and latency instead of scattered `println!` calls. It lets us correlate slow requests with database or downstream timings once those operations are instrumented. It is not distributed tracing by itself; propagation and span context still need to be added across queues and services.

20. **Probe: How would you protect only transaction routes?**  
    **A:** I would create a protected sub-router containing `/api/v1/transactions`, apply `route_layer(middleware::from_fn(require_api_key))`, and merge it with public `/health` and `/metrics` routes. This avoids accidentally protecting health checks needed by orchestration. I would add tests for missing, invalid, and valid keys and verify that authentication failure does not touch PostgreSQL.

## API Security and Validation

21. **Q: Is an API key authentication or authorization?**  
    **A:** An API key authenticates possession of a credential; it answers who or what presented the request. Authorization is the separate decision about which tenant, account, or transaction that identity may access. The `auth.rs` example only performs the first step. Production authorization must bind the authenticated principal to every resource query and state change.

22. **Q: Why compare API keys in constant time?**  
    **A:** A normal string comparison may return sooner when an early byte differs, which can create a timing side channel. `subtle::ConstantTimeEq` reduces that signal. It is only one layer: the key still needs TLS, secure storage, rotation, revocation, rate limiting, and audit logging. Constant-time comparison cannot protect a key sent over plaintext HTTP.

23. **Q: What is wrong with logging `DATABASE_URL`?**  
    **A:** A database URL often contains username, password, host, port, and database name. Logs are copied to aggregators and retained, so one accidental log line can become a credential leak. I would log a redacted host or connection status, never the full URL, and configure tracing filters so secrets cannot enter fields or error messages.

24. **Q: What input validation is missing from the transaction handler?**  
    **A:** The handler currently deserializes an amount and strings but does not enforce business limits or address syntax. I would reject zero and excessive amounts, validate address format, cap string lengths, normalize or reject invalid Unicode where appropriate, and enforce an idempotency-key length limit. Validation should happen before allocating large values or opening a database transaction.

25. **Q: Why validate at both API and domain boundaries?**  
    **A:** API validation gives a precise client-facing error, but the service can also be called by a worker, test, CLI, or future gRPC endpoint. If only the HTTP layer validates, another caller can bypass the invariant. Domain constructors should enforce correctness; handlers can add transport-specific constraints such as content type and payload size.

26. **Q: What is the difference between 401 and 403?**  
    **A:** `401 Unauthorized` means the caller has not supplied valid authentication credentials, and the response may include a challenge. `403 Forbidden` means the server knows who the caller is but that identity is not allowed to perform the operation. Returning 404 instead of 403 can sometimes prevent resource-existence disclosure, but that should be a deliberate policy.

27. **Q: How should credentials be rotated?**  
    **A:** I would support an overlap period where old and new credentials are accepted, publish the new secret, update clients, observe old-key usage, and revoke the old key after migration. A deployment should not require changing every client simultaneously. Secrets belong in a secret manager or orchestrator secret, not in Git, Docker layers, shell history, or error logs.

28. **Q: How do you prevent replay of a signed request?**  
    **A:** Bind the signature to the HTTP method, path, body hash, timestamp, and a unique nonce or idempotency key. Reject requests outside a time window and persist consumed nonces atomically for the identity. A valid signature alone does not prevent replay; the server must remember enough state to detect reuse.

29. **Q: What security issue exists in exposing development PostgreSQL ports?**  
    **A:** Binding `5433:5432` to all interfaces makes the development database reachable from other hosts if the firewall permits it. That is why the configuration is convenient for Minikube but unsafe as a production pattern. I would bind to loopback for normal local work, use a cluster Service for Kubernetes, and require strong credentials and network policy.

30. **Probe: Design a secure transaction endpoint.**  
    **A:** I would terminate TLS at a trusted gateway, authenticate the caller, authorize the account, validate amount and addresses, require an idempotency key, rate-limit by principal, and write an audit event. The service would perform an atomic database operation and return a request ID. Metrics and logs would record outcome and latency without recording secrets or full payloads.

## PostgreSQL and SQLx

31. **Q: Why use SQLx instead of building SQL strings?**  
    **A:** SQLx keeps SQL explicit while parameter binding prevents injection and its async integration fits Tokio. Query macros can validate SQL against database metadata during development. Building SQL by concatenating user input is unsafe and makes query behavior difficult to review. The trade-off is that schema changes and offline metadata need deliberate build management.

32. **Q: What does a connection pool provide?**  
    **A:** A pool reuses established connections, limits concurrent database work, and avoids paying connection setup cost per request. Acquisition timeout prevents requests from waiting forever under saturation. Pooling is not unlimited capacity: a pool that is too large overloads PostgreSQL, while one that is too small creates avoidable queueing and latency.

33. **Q: Why is pool size a system-wide limit?**  
    **A:** The configured five connections apply per process, not per deployment. Three replicas may use fifteen connections, and migrations, admin tools, and workers need additional capacity. I would calculate `replicas × max_connections`, reserve headroom, observe pool wait time, and use a pooler only after measuring a real connection-management bottleneck.

34. **Q: What does `ON CONFLICT DO NOTHING` accomplish?**  
    **A:** It lets PostgreSQL arbitrate a duplicate unique-key insert atomically. The first request inserts; a concurrent duplicate becomes a no-op, after which the service can fetch the existing row. This is safer than a separate “does it exist?” query. The application must still compare the original request fingerprint and return a conflict if the payload differs.

35. **Q: Why must idempotency keys have a unique constraint?**  
    **A:** Two requests can both perform an application-level existence check before either inserts. A unique constraint serializes the final decision inside PostgreSQL and survives multiple application replicas. The index should usually include tenant or principal scope, and the key should have an expiry or cleanup strategy if the table grows indefinitely.

36. **Q: What is optimistic concurrency?**  
    **A:** Optimistic concurrency assumes conflicts are relatively uncommon and detects them at update time. The update includes the expected status or version in its `WHERE` clause; zero affected rows means another writer changed the record or it is missing. It avoids holding long locks, but callers need a clear conflict response and possibly a refresh/retry policy.

37. **Q: What does `SELECT ... FOR UPDATE` do?**  
    **A:** It locks selected rows until the surrounding transaction commits, so another transaction trying to update those rows waits or fails according to its lock behavior. It is useful when a worker must read and mutate a row as one serialized decision. The cost is blocking and deadlock risk, so lock order and transaction duration must be controlled.

38. **Q: When is `SKIP LOCKED` useful?**  
    **A:** A queue worker can select available jobs while skipping jobs another worker has already claimed. That improves throughput because workers do not wait behind one slow job. The trade-off is that older locked jobs can be delayed, so the design needs visibility timeouts, retry timestamps, and monitoring for starvation.

39. **Q: What is a transaction isolation level?**  
    **A:** Isolation controls which concurrent changes a transaction can observe and which anomalies are possible. Read committed is a common default; repeatable read or serializable can provide stronger guarantees but may produce retries or more contention. I choose the weakest level that preserves the business invariant and test serialization failures explicitly.

40. **Probe: Diagnose a slow status query.**  
    **A:** I would capture the exact SQL and representative parameters, run `EXPLAIN (ANALYZE, BUFFERS)`, compare estimated and actual row counts, and check whether the predicate uses an appropriate index. Then I would benchmark before and after with realistic data, inspect lock and pool wait time, and confirm that an index does not make writes unacceptably slower.

## Idempotency and State Machines

41. **Q: What does idempotency mean for POST?**  
    **A:** It means repeated delivery of the same logical command produces one business effect and a stable result. HTTP POST is not inherently idempotent, so the server must implement the behavior using a client key, request fingerprint, durable record, and atomic constraint. It is especially important when clients retry after a timeout and cannot know whether the first request committed.

42. **Q: What should happen when a key is reused with different data?**  
    **A:** I return a conflict because the key already identifies a different logical command. Returning the old response would hide a client bug, while processing the new request would violate the key’s promise. The comparison should use a canonical request fingerprint, not just one field, and the response should explain that the key cannot be reused.

43. **Q: What race exists in check-then-insert logic?**  
    **A:** Request A and request B can both check for absence before either inserts, so both believe they own the key. A database uniqueness constraint closes the race; `ON CONFLICT` converts the losing insert into a lookup path. This is a good interview example of why application logic alone cannot provide concurrency guarantees.

44. **Q: What should an idempotency record contain?**  
    **A:** I would store the principal or tenant, key, canonical request hash, lifecycle status, result reference or response, created/updated timestamps, and expiry. Storing the response can make retries fast, while storing a resource ID avoids duplicating large payloads. The record must be durable and written in the same transaction as the first business effect when possible.

45. **Q: Why scope keys by tenant?**  
    **A:** A key such as `payment-123` is usually unique from the client’s perspective, not globally unique for every customer. A composite unique key `(tenant_id, idempotency_key)` avoids cross-tenant collisions and prevents one tenant from probing another tenant’s request history. Authorization must still be checked before returning an existing result.

46. **Q: How should an in-progress duplicate request behave?**  
    **A:** The contract should be explicit: return the existing processing state, wait briefly for completion, or return `202 Accepted` with a status URL. It must never execute the external side effect a second time. A bounded wait prevents request threads from accumulating, and clients should use polling or callbacks for long-running operations.

47. **Q: Why model transaction states explicitly?**  
    **A:** An enum and a central transition function make the lifecycle visible: a reviewer can see which transitions are legal and tests can enumerate them. Strings scattered through handlers are vulnerable to typos and contradictory rules. A state machine should also document terminal states, retryable states, ownership, and which actor may perform each transition.

48. **Q: What is a state conflict?**  
    **A:** It means the caller’s expected state is stale because another writer changed the record first. In ChainFlow, a conditional update returning no row leads to a lookup that distinguishes not-found from current-state mismatch. Returning 409 lets the client refresh and decide; returning 500 would incorrectly suggest an infrastructure failure.

49. **Q: Why perform the domain transition check before SQL?**  
    **A:** It rejects impossible transitions without a database round trip and keeps the domain rule deterministic. The database check is still required because another writer may change the state after the in-memory check. This illustrates a useful pattern: validate early for efficiency, enforce atomically at the persistence boundary for correctness.

50. **Probe: Add a version column.**  
    **A:** Add `version BIGINT NOT NULL`, read the expected version, and execute `UPDATE ... SET status = $1, version = version + 1 WHERE id = $2 AND version = $3`. If the affected-row count is zero, fetch the record and classify not-found versus concurrent conflict. Tests should run two writers and prove that exactly one wins.

## Concurrency and Messaging

51. **Q: What does Tokio concurrency guarantee?**  
    **A:** Tokio schedules futures and lets I/O-bound tasks make progress while others wait. It does not make shared mutable state safe, prevent duplicate messages, or guarantee fairness. I still need database constraints, synchronization, cancellation handling, and bounded concurrency. A service can be memory-safe and still be logically racy.

52. **Q: Why can a Rust `Mutex` not coordinate multiple replicas?**  
    **A:** A mutex protects memory within one process. If three Pods each have their own mutex, all three can enter the critical section independently. Cross-process ownership requires a database lock, queue claim, or carefully designed distributed lease with fencing. Even a distributed lock should not replace database constraints for correctness.

53. **Q: What is at-least-once delivery?**  
    **A:** At-least-once means the broker tries not to lose a message, so a message can be delivered again when the consumer crashes before acknowledgement. The application must assume duplicates and make its handler idempotent. Exactly-once delivery is generally not end-to-end once an external database or blockchain is involved.

54. **Q: When should a RabbitMQ consumer acknowledge?**  
    **A:** Acknowledge after the business outcome is durable, not merely after parsing the message. Acknowledge-before-work risks losing work on a crash; acknowledge-after-work enables redelivery but requires deduplication. The consumer should also use a bounded prefetch so a process does not hold more unacknowledged work than it can safely recover.

55. **Q: What happens if a worker crashes after the chain accepts a transaction but before ACK?**  
    **A:** RabbitMQ can redeliver the message, but the blockchain side effect may already exist. The retry path must query the durable operation record and chain state, reuse the same signed transaction or nonce when appropriate, and mark the original outcome. Blindly submitting a new transaction can create a duplicate payment.

56. **Q: What is a dead-letter queue?**  
    **A:** A dead-letter queue receives messages that expire, are rejected, or exceed retry limits. It prevents a deterministic poison message from blocking normal work forever and gives operators a controlled inspection and replay path. It should include the failure reason and original metadata, while replay must remain idempotent.

57. **Q: Why use exponential backoff with jitter?**  
    **A:** Immediate retries increase load precisely when a dependency is unhealthy. Exponential backoff spaces attempts farther apart, and random jitter prevents thousands of clients from retrying at the same instant. I would cap the delay and total attempts, classify errors as retryable or terminal, and expose retry counts as metrics.

58. **Q: What is the outbox pattern?**  
    **A:** The transaction and an event row are committed together in PostgreSQL, then a publisher reads unsent outbox rows and sends them to RabbitMQ. This avoids the dual-write gap where the database commits but publishing fails. Publishing is still at least once, so the event needs a stable ID and consumers must deduplicate.

59. **Q: What is a poison message?**  
    **A:** It is a message that will fail repeatedly because of invalid data, an unsupported state, or a deterministic bug. Retrying forever wastes capacity and creates noisy alerts. After bounded retries, route it to a DLQ, alert with enough context to investigate, and provide a safe replay process after the cause is fixed.

60. **Probe: Design a safe worker loop.**  
    **A:** Consume manually, validate and authorize the message, claim work atomically, perform an idempotent side effect, persist the outcome, and ACK only after durable completion. Transient failures use bounded backoff; terminal failures go to a DLQ. The loop must handle cancellation, connection loss, lease expiry, and graceful shutdown without acknowledging unfinished work.

## Docker and Kubernetes

61. **Q: Why use a multi-stage Docker build?**  
    **A:** The builder stage needs Rust, `protoc`, headers, and compilers, while the runtime needs only the binary, certificates, SSL libraries, and healthcheck tools. Keeping build tools out of the final image reduces size and attack surface. I also run the binary as a non-root numeric user and pin the build inputs where reproducibility matters.

62. **Q: Why must `proto` be copied into the builder?**  
    **A:** `build.rs` calls `tonic_prost_build::compile_protos("proto/chainflow.proto")`. Docker only has files explicitly copied into the image, so omitting `proto` produces a misleading `protoc` “file not found” error. The general lesson is to inspect the build context, Docker ignore rules, and working directory rather than assuming a host file exists in the image.

63. **Q: What does a Docker healthcheck do?**  
    **A:** It executes a command inside the container and reports healthy or unhealthy to Compose. It does not repair the process, validate every dependency, or replace application-level monitoring. The check must use a tool installed in the runtime image; otherwise a healthy server can appear unhealthy simply because `curl` is missing.

64. **Q: Why did an old image return 404 for `/metrics`?**  
    **A:** Source code and a Docker image are separate artifacts. Adding a route changes the source but not the binary already running in the container. The fix is to rebuild `chainflow:dev` and recreate the service, then curl the endpoint directly before diagnosing Prometheus configuration. This is a common deployment-artifact mismatch.

65. **Q: What does a Kubernetes Deployment manage?**  
    **A:** A Deployment declares the desired number and version of stateless Pods and manages ReplicaSets during rolling updates. It replaces failed Pods and gradually changes versions according to surge and availability settings. It does not provide durable database semantics, message exactly-once behavior, or application-level transaction recovery.

66. **Q: What does a Service provide?**  
    **A:** A Service gives clients a stable virtual address and selects ready Pods by labels. It decouples callers from changing Pod IPs and distributes traffic across endpoints. A Service does not make an unhealthy Pod healthy; readiness probes determine whether that Pod is eligible to receive traffic.

67. **Q: Why is `imagePullPolicy: IfNotPresent` useful in Minikube?**  
    **A:** It allows a locally loaded image to run without pulling from a registry, which is convenient for development. The risk is that the same tag can refer to different binaries, so production should use immutable version tags or image digests and a registry. Local workflows must explicitly reload the image after rebuilding.

68. **Q: What is the difference between readiness and liveness?**  
    **A:** Readiness controls routing and should fail when the instance cannot serve useful traffic. Liveness controls restarts and should fail only when the process is stuck or irrecoverable. If liveness checks the database, a temporary database outage can cause every Pod to restart and worsen recovery; dependency failure usually belongs in readiness.

69. **Q: Why should PostgreSQL not be casually scaled with a Deployment?**  
    **A:** Multiple PostgreSQL Pods with independent writable volumes are not a replicated database; they can diverge or corrupt an application’s assumptions. Production PostgreSQL requires replication, durable volumes, backups, failover, and an operator or managed service. The ChainFlow API is stateless and can scale horizontally, while the database needs a separate operational design.

70. **Probe: Explain a CrashLoopBackOff.**  
    **A:** I start with `kubectl describe pod` for events and probe failures, then inspect current and `--previous` logs, exit code, image, environment, Secret, and dependency DNS/network reachability. In this project the root cause was `PoolTimedOut`: the Pod could start but could not reach PostgreSQL. I fix the dependency endpoint, verify the Secret, and watch rollout status.

## CI, Testing, and Operations

71. **Q: Why run Clippy with `-D warnings`?**  
    **A:** Warnings such as needless clones may not break behavior today, but they increase maintenance cost and can hide new defects. `-D warnings` makes the CI contract deterministic: every warning must be understood or fixed. Developers should still run ordinary Clippy locally while iterating, then run the strict command before pushing.

72. **Q: Why test both Rust and Docker?**  
    **A:** A host build can pass while the Docker build fails because files, system libraries, `protoc`, users, or runtime tools differ. Conversely, an image can build while tests or lint fail. Separate jobs catch both classes and a dependency gate ensures the image is built only after Rust checks pass.

73. **Q: What should unit tests cover here?**  
    **A:** Unit tests should cover legal and illegal state transitions, overflow conversion, idempotency conflict classification, and concurrent-writer outcomes that can be expressed with test fixtures. They should be fast and deterministic. A unit test should not pretend to verify PostgreSQL locking behavior that only an integration test can prove.

74. **Q: What should integration tests cover?**  
    **A:** Integration tests should use an isolated PostgreSQL instance to verify migrations, unique constraints, SQL behavior, HTTP serialization, authentication, and real transaction conflicts. They should clean up data or use disposable databases. The CI environment must provision dependencies explicitly; otherwise tests can pass on a developer laptop because of an accidental local service.

75. **Q: What is a smoke test?**  
    **A:** A smoke test is a small, high-signal check after deployment: the process starts, `/health` responds, `/metrics` is reachable, and one representative authenticated request works. It is not a replacement for the full test suite. Its purpose is to catch wrong images, missing Secrets, broken routes, and obvious dependency failures quickly.

76. **Q: Why pin action versions and image versions?**  
    **A:** Mutable tags can change without a source commit, producing a non-reproducible build or a supply-chain surprise. I pin reviewed GitHub Action major versions at minimum and prefer image digests for production. Dependency updates then happen through an intentional review instead of silently changing every CI run.

77. **Q: What belongs in a deployment gate?**  
    **A:** I would gate on formatting, strict linting, unit and integration tests, Docker build, vulnerability policy, migration compatibility, and rollout health. The gate should also run smoke tests against the deployed artifact, not merely against a locally compiled binary. Approval and production credentials should be separated from ordinary pull-request permissions.

78. **Q: How should a failed rollout be investigated?**  
    **A:** First preserve evidence: rollout status, events, ReplicaSets, Pod logs, image digest, configuration, and failing probe output. Then classify the issue as code, image, configuration, dependency, or capacity. Roll back when user impact is increasing, but continue root-cause analysis because rollback does not fix a bad migration or an external dependency.

79. **Q: What is a safe database migration strategy?**  
    **A:** Use expand-and-contract: add compatible schema first, deploy code that works with both old and new shapes, backfill, switch reads and writes, and remove obsolete schema only after old binaries are retired. Rolling deployments mean old and new versions overlap. A migration that only one version understands can make a healthy rollout fail midway.

80. **Probe: Design a rollback test.**  
    **A:** Deploy a known-good immutable image, deploy a deliberately failing image, assert that readiness never reaches the desired state, run `kubectl rollout undo`, and verify both Pods and smoke tests recover. Also test database compatibility: rolling back code must not assume a column or enum value that the forward migration removed.

## Observability and Reliability

81. **Q: What are logs, metrics, and traces for?**  
    **A:** Logs explain individual events, metrics show aggregate behavior over time, and traces connect one request across services and dependencies. For a slow ChainFlow request, a metric can show a latency regression, a trace can locate time in PostgreSQL or RPC, and logs can provide the detailed error context. None replaces the others.

82. **Q: Why use route templates as metric labels?**  
    **A:** Labeling by raw paths creates a new time series for every transaction or UUID and can overwhelm Prometheus. Route templates such as `/api/v1/transactions/{id}` keep cardinality bounded. I keep high-cardinality values in logs or traces instead, and I monitor the number of active series as part of observability health.

83. **Q: What should an HTTP histogram measure?**  
    **A:** I would record request duration by method and normalized route, plus a counter by status class. Histograms support p50, p95, and p99 latency, while counters support error rate and throughput. I avoid user IDs, request IDs, and transaction IDs as labels because they create unbounded cardinality.

84. **Q: What is a useful SLI for this API?**  
    **A:** A useful SLI is the proportion of valid transaction commands that return the promised outcome within a latency target. I would separate deliberate client errors from server and dependency failures, but not hide database timeouts. The SLI must match user experience; a green process health check is not enough if transaction creation is failing.

85. **Q: What is an SLO?**  
    **A:** An SLO is a target for an SLI over a defined window, such as 99.9% of eligible transaction requests completing under 300 ms in a month. It turns “reliable” into an observable contract. The target must be based on user needs and capacity measurements, not chosen arbitrarily because a round number sounds impressive.

86. **Q: What is an error budget?**  
    **A:** An error budget is the amount of unreliability allowed by the SLO. If the monthly target is 99.9%, the budget is roughly 0.1% of eligible time or requests. Teams can spend that budget on releases and experiments, but when it is exhausted they should prioritize reliability, rollback, and capacity work over risky features.

87. **Q: Why should `/health` and `/metrics` be handled carefully?**  
    **A:** They are frequently polled, so they must be cheap and protected from accidental expensive database work. Metrics can reveal route names, dependency behavior, and business volume; health endpoints can reveal deployment state. I expose them only on the intended network, avoid secrets in labels, and use separate liveness and readiness semantics.

88. **Q: What makes a useful request ID?**  
    **A:** It should be unique enough for an operation, accepted or generated at the edge, returned in the response, and included in logs and downstream message metadata. It is for correlation, not authentication. I would sanitize client-supplied IDs, cap their length, and use a trace ID when full distributed tracing is available.

89. **Q: Why are retries dangerous?**  
    **A:** A retry can duplicate a side effect and multiply load during an outage. I retry only errors classified as transient, use exponential backoff with jitter and a deadline, and require an idempotency or deduplication mechanism before retrying a command. The client and server should coordinate retry semantics rather than both retrying aggressively.

90. **Probe: A Prometheus target reports `health=down` and 404.**  
    **A:** A 404 proves network connectivity but means the running server does not expose the configured path. I curl the endpoint from the Prometheus network, inspect the image digest and route source, rebuild the image, recreate the container, and check the target again. Changing Prometheus configuration first would hide the likely stale-image problem.

## Advanced System Design

91. **Q: Design ChainFlow for 10,000 requests per minute.**  
    **A:** Ten thousand per minute is about 167 requests per second on average, but bursts and blockchain latency determine the real design. I would use an API gateway for TLS, auth, and rate limits; horizontally scaled stateless Axum instances; bounded database pools; PostgreSQL for durable intent; an outbox and RabbitMQ for asynchronous work; idempotent workers; and RPC failover. I would load-test realistic transaction flows, not only `/health`.

92. **Q: How do you prevent duplicate blockchain payments?**  
    **A:** Persist an immutable payment intent with a tenant-scoped idempotency key before submitting to the chain. Store the signed transaction identity, nonce or equivalent chain reference, and lifecycle state. If a worker crashes after acceptance, reconciliation first checks chain state and reuses the same operation rather than creating a new payment. The database constraint prevents duplicate intents; reconciliation handles uncertain external outcomes.

93. **Q: How do you handle a blockchain reorganization?**  
    **A:** I distinguish submitted, included, confirmed, and final states instead of treating the first inclusion as permanent. A reorg detector rescans affected blocks, marks reverted observations, and reconciles the canonical chain after the required confirmation depth. All updates must be idempotent because block notifications and recovery scans can overlap.

94. **Q: How do you isolate tenants?**  
    **A:** Every authenticated request carries a tenant or principal identity into the service, and every repository query includes that scope. Composite uniqueness and foreign keys prevent accidental cross-tenant collisions. I would add authorization tests that attempt to read and mutate another tenant’s transaction, consider PostgreSQL row-level security for defense in depth, and avoid trusting a tenant ID supplied only in the body.

95. **Q: How do you handle a database outage?**  
    **A:** At startup, use bounded retries with exponential backoff and jitter, then fail clearly if the dependency never becomes available. During runtime, readiness should fail so traffic drains, while liveness should avoid restart storms. Requests should return a retryable dependency error, queued work should remain durable, and alerts should distinguish database reachability, pool exhaustion, and query latency.

96. **Q: How do you scale workers safely?**  
    **A:** Use queue prefetch limits and bounded task concurrency so workers do not overload PostgreSQL or an RPC provider. Claim each job atomically, enforce a visibility timeout or lease, make processing idempotent, and route terminal failures to a DLQ. Monitor queue age, retry rate, processing latency, and provider rate-limit responses, not just the number of queued messages.

97. **Q: How do you protect PostgreSQL at high replica counts?**  
    **A:** First calculate the maximum connections implied by replicas, workers, migrations, and administrative traffic. Cap application pools, reserve operational headroom, optimize slow queries and indexes, and use a connection pooler only when measurements justify it. Autoscaling the API without a database connection budget can turn a traffic spike into a database outage.

98. **Q: How do you design a zero-downtime release?**  
    **A:** Build and scan an immutable image, apply backward-compatible migrations, deploy with readiness-gated rolling updates, and drain old Pods only after replacements are healthy. Run smoke tests against the deployed version and keep the previous image available for rollback. I also verify that queue consumers and old API versions can coexist during the deployment window.

99. **Q: What consistency model does an outbox provide?**  
    **A:** The outbox gives atomic consistency between the local database change and the intent to publish an event. It does not give exactly-once delivery: the publisher can send an event and crash before marking it sent. Therefore events need stable IDs, consumers deduplicate, and a repair process detects stuck or missing outbox rows.

100. **Final system-design probe: What would you improve first in this repository?**  
     **A:** I would first protect the transaction route with the API-key middleware, remove hardcoded development credentials from shared configuration, and replace ad-hoc error strings with a stable error envelope and request IDs. Next I would add a dependency-aware readiness check, integration tests for PostgreSQL and HTTP auth, migration automation, immutable image publishing, and failure-injection tests. Those changes improve security and operational correctness before adding more features.

## How to Practice

For each question, answer aloud in 60 to 120 seconds. Start with the direct answer, then connect it to a file or flow in ChainFlow, state one trade-off, and finish with a test or failure mode. For every probe, sketch the data model and concurrency boundary before writing code. A strong candidate does not claim that a library guarantees correctness; they explain which invariant the application and database enforce together.
