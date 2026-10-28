# Step 12 — Complete Distributed Coordination

We'll finish the remaining concepts in Step 12 as one connected practical lesson, without skipping the difficult parts.

You have already implemented Redis lock acquisition, UUID ownership, atomic Lua-based release, and sequential lease renewal.

Now we'll extend your existing `examples/distributed_lock.rs` to cover:

1. Independent Tokio heartbeat tasks.
2. Cancellation and lease-loss detection.
3. Worker crashes and automatic recovery.
4. Fencing tokens and stale-worker protection.
5. Coordination across multiple ChainFlow instances.

## Part 1 — Why our existing heartbeat isn't enough

Your current implementation looks approximately like this:

```
for second in 1..=4 {
    tokio::time::sleep(Duration::from_secs(10)).await;

    let renewed = renew_lock(
        &mut connection,
        "lock:txn-500",
        &lock_token,
    ).await?;
}
```

This works for learning, but the worker is responsible for both processing and renewing.

Imagine that the worker starts a blockchain RPC request that takes 45 seconds. If the renewal code cannot run until that request finishes, the 30-second lease expires.

A better design separates the responsibilities:

ChainFlow Worker A

Acquires a Redis lease with a unique token

Processing task

Performs blockchain operations and database work

Heartbeat task

Renews the Redis lease every 10 seconds

Lease-loss signal

If renewal fails, stop or cancel processing and prevent stale writes

Tokio can schedule both tasks concurrently, even on a single runtime thread, provided neither task blocks the executor.

## Part 2 — Independent heartbeat and cancellation

We'll use `tokio::sync::watch` to communicate between the heartbeat and the processing task.

A watch channel stores the latest value. Here, `true` means the worker has been told to stop.

### 2.1 Add the imports

At the top of `examples/distributed_lock.rs`:

```
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{interval, sleep, MissedTickBehavior};
```

Why these imports?

- `Duration` represents a time interval.
- `watch` lets one task signal another.
- `interval` creates periodic heartbeat ticks.
- `sleep` simulates asynchronous work.
- `MissedTickBehavior` controls how missed timer ticks are handled.

### 2.2 Create the heartbeat function

Keep your existing `renew_lock()` function, then add:

```
async fn heartbeat(
    client: redis::Client,
    lock_key: String,
    lock_token: String,
    stop_tx: watch::Sender<bool>,
) {
    let mut ticker = interval(Duration::from_secs(10));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    // Skip the immediate first tick.
    ticker.tick().await;

    loop {
        ticker.tick().await;

        let result = async {
            let mut connection = client
                .get_multiplexed_async_connection()
                .await?;

            renew_lock(
                &mut connection,
                &lock_key,
                &lock_token,
            )
            .await
        }
        .await;

        match result {
            Ok(true) => {
                println!("Heartbeat: lease renewed");
            }

            Ok(false) => {
                println!("Heartbeat: ownership lost");
                let _ = stop_tx.send(true);
                break;
            }

            Err(error) => {
                println!("Heartbeat: Redis error: {error}");
                let _ = stop_tx.send(true);
                break;
            }
        }
    }
}
```

### Understand the key lines

`client: redis::Client`

The heartbeat receives its own Redis client handle. It does not need to borrow the processing worker's connection.

`lock_key: String` and `lock_token: String`

The heartbeat owns these strings, which makes them suitable for a separately spawned task.

`stop_tx: watch::Sender<bool>`

This is the sending side of a watch channel. When the heartbeat detects a problem, it sends `true`.

`interval(Duration::from_secs(10))`

Creates a timer that ticks every 10 seconds.

`ticker.tick().await` before the loop

Tokio's first interval tick is immediate. We consume it so the first actual renewal happens approximately 10 seconds later.

`match result`

There are three outcomes:

| Result       | Meaning                          | Action              |
| ------------ | -------------------------------- | ------------------- |
| `Ok(true)`   | Renewal succeeded                | Continue            |
| `Ok(false)`  | Token no longer owns the lock    | Signal cancellation |
| `Err(error)` | Cannot verify or renew ownership | Signal cancellation |

Notice that we treat a Redis error as a reason to stop processing. We must not assume that the lease is still valid when ownership cannot be verified.

This example deliberately stops on the first renewal error. A production implementation may use bounded retries within a carefully managed lease deadline, but must fail closed before protected writes.

## Part 3 — Make the processing task respond to cancellation

Add a separate function:

```
async fn process_job(
    worker_name: &str,
    mut stop_rx: watch::Receiver<bool>,
) -> bool {
    for step in 1..=12 {
        tokio::select! {
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    println!("{worker_name}: processing cancelled");
                    return false;
                }
            }

            _ = sleep(Duration::from_secs(5)) => {
                println!("{worker_name}: completed step {step}");
            }
        }
    }

    true
}
```

### What is `tokio::select!`?

This is an important Rust async concept.

`tokio::select!` waits for multiple asynchronous operations. Whichever branch becomes ready first executes, and the other branch's pending future is dropped.

In our example:

- One branch waits for a cancellation signal.
- The other waits five seconds to simulate work.

If the heartbeat detects a lost lease, it sends `true`. The processing task wakes up and stops instead of continuing all 12 steps.

Important limitation: `select!` does not magically undo external side effects. If an RPC request or database operation has already completed, cancellation cannot reverse it. Protected writes still need database-level safeguards.

## Part 4 — Connect heartbeat and processing

In your existing `run_worker()`, replace the old four-iteration renewal loop with the following code inside `if acquired`:

```
let (stop_tx, stop_rx) = watch::channel(false);

let heartbeat_handle = tokio::spawn(heartbeat(
    client.clone(),
    "lock:txn-500".to_string(),
    lock_token.clone(),
    stop_tx.clone(),
));

let processing_completed = process_job(
    worker_name,
    stop_rx,
)
.await;

// Ask the heartbeat task to stop after processing.
heartbeat_handle.abort();
let _ = heartbeat_handle.await;

if processing_completed {
    println!("{worker_name}: processing completed");
} else {
    println!("{worker_name}: processing stopped");
}

let released = release_lock(
    &mut connection,
    "lock:txn-500",
    &lock_token,
)
.await?;

println!("{worker_name}: lock released = {released}");
```

The important change is:

```
tokio::spawn(heartbeat(...))
```

This launches the heartbeat independently.

Meanwhile:

```
process_job(...).await
```

runs the simulated processing.

The heartbeat can renew the lease while processing is waiting on asynchronous operations.

This is still a learning implementation: `abort()` stops the heartbeat task at a cancellation point, and the final release remains ownership-checked. For production, we'll also require a final lease-validity check and protect the business transaction against stale owners.

### Run it

```
redis-cli DEL lock:txn-500
cargo run --example distributed_lock
```

Expected behaviour:

- One worker acquires the lock.
- The other worker skips processing.
- The winning worker processes for approximately 60 seconds.
- The heartbeat renews the lease approximately every 10 seconds.
- After processing, the worker stops the heartbeat and releases the lock.

## Part 5 — Simulate lease loss

Now let's deliberately break the lease.

While the worker is running, open a second terminal.

Wait until you see the first successful heartbeat, then execute:

```
redis-cli DEL lock:txn-500
```

This manually removes the lock.

On its next renewal attempt, the heartbeat should receive `false`, because its token no longer exists in Redis.

Expected output will resemble:

```
worker-A: completed step 1
worker-A: completed step 2
Heartbeat: lease renewed
worker-A: completed step 3
Heartbeat: ownership lost
worker-A: processing cancelled
worker-A: processing stopped
worker-A: lock released = false
```

The precise ordering may differ.

This demonstrates lease-loss detection and cooperative cancellation.

### What if the entire worker crashes?

A process crash also stops the heartbeat. Redis will eventually expire the lock when its remaining TTL reaches zero.

Another worker can then acquire it.

However, a lock expiring does not automatically recreate a RabbitMQ message or restart a job. Recovery requires the queue's redelivery mechanism or a separate database-backed recovery process.

For example, with RabbitMQ manual acknowledgements, a worker process dying before ACK normally causes its unacknowledged message to become eligible for redelivery after the broker detects the lost connection.

## Part 6 — Fencing tokens: protecting against stale workers

This is one of the most important distributed-systems concepts.

Imagine:

- Worker A acquires a lease and starts processing.
- Worker A pauses for a long time.
- Its lease expires.
- Worker B acquires a new lease and writes updated data.
- Worker A resumes and tries to overwrite Worker B's data.

Unique UUIDs prevent Worker A from releasing Worker B's Redis lock, but they do not automatically prevent Worker A from writing to PostgreSQL.

A fencing token solves this by giving each new lease a monotonically increasing number.

| Worker           | Fencing token | Database decision  |
| ---------------- | ------------- | ------------------ |
| Worker A         | 41            | Initially accepted |
| Worker B         | 42            | Accepted as newer  |
| Worker A resumes | 41            | Rejected as stale  |

The database must enforce the ordering. A Redis counter alone is not sufficient unless token allocation is correctly tied to successful lock ownership and the counter cannot reset or move backwards.

For ChainFlow, we can implement the stronger version using PostgreSQL:

- Store a generation number for each protected job.
- Atomically increment it when a worker claims the job.
- Require the generation number to match when updating job state.
- Reject updates from older generations.

For example, suppose a job has `generation = 42`.

Worker A tries:

```
UPDATE jobs
SET status = 'COMPLETED'
WHERE id = $1
  AND generation = 41;
```

PostgreSQL updates zero rows.

Worker B uses generation 42 and can update the row, provided the other business conditions are satisfied.

This is an example of database-enforced fencing. The generation must be assigned through a transactionally safe claim operation, not by guessing a number.

## Part 7 — Coordination across multiple instances

Now consider ChainFlow running on three servers.

ChainFlow A

ChainFlow B

ChainFlow C

Shared Redis

Coordinates lease ownership across instances

PostgreSQL

Enforces idempotency, state transitions and fencing

All three instances must coordinate through shared infrastructure. An in-memory Rust `Mutex` would only protect tasks within one process; it cannot coordinate separate servers.

Even with Redis locks, the final correctness guarantees must come from the business-state constraints and transactional operations that protect your data.

## Step 12 completion checklist

Hands-on verification

0/5 verified

Independent heartbeat renews the lease during long-running work

Processing receives and responds to lease-loss cancellation

Worker crash and TTL recovery are tested

Stale-worker writes are rejected using database fencing

Multiple ChainFlow instances coordinate through shared infrastructure

### Where we stand

The lesson now covers the remaining Step 12 concepts, but we should only mark the step fully complete after testing the heartbeat, cancellation, crash recovery, fencing and multi-instance behaviour.

Start by running the independent-heartbeat example and then the manual lease-loss test. Share the output, and we'll verify the behaviour before implementing the PostgreSQL fencing exercise and moving on to Step 13.