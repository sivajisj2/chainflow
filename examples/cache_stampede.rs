// This example demonstrates the "cache stampede" problem and how to prevent it.
//
// Cache stampede: when a cached value expires and 100 requests hit your app at
// the same time, all 100 will miss the cache, all 100 will hit the database,
// and all 100 will write back — 100× the necessary load.
//
// The fix: use a per-miss "fill lock" so only ONE task does the expensive
// database fetch while the others wait and get the same result.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

struct Cache {
    values: Mutex<HashMap<String, String>>,

    // A second lock used ONLY when a cache miss happens.
    // It ensures that only one task at a time does the slow database query.
    // All other tasks that miss the cache will wait on this lock, then
    // check the cache again (it will be filled by now).
    fill_lock: Mutex<()>,

    // Simple counter to prove the database was hit only once.
    database_queries: AtomicUsize,
}

impl Cache {
    fn new() -> Self {
        Self {
            values: Mutex::new(HashMap::new()),
            fill_lock: Mutex::new(()),
            database_queries: AtomicUsize::new(0),
        }
    }

    // Fetch a value by key. If it's in the cache, return it immediately.
    // If not, do exactly ONE database query to fill the cache, then return.
    async fn get_status(&self, key: &str) -> String {
        // Step 1 — fast path: check the cache. Hit? Return immediately.
        if let Some(value) = self.values.lock().await.get(key).cloned() {
            return value;
        }

        // Step 2 — cache miss. Grab the fill lock.
        // If 100 tasks miss at once, 99 of them block here waiting for
        // this mutex. Only the first one gets through.
        let _guard = self.fill_lock.lock().await;

        // Step 3 — DOUBLE CHECK.
        // While task #1 was doing the slow DB query, the other 99 tasks
        // were parked at the lock. When task #1 finishes and drops the
        // lock, one of those 99 tasks wakes up. It MUST check the cache
        // again because task #1 may have already filled it. Without this
        // second check, you'd still have N-1 database calls.
        if let Some(value) = self.values.lock().await.get(key).cloned() {
            return value;
        }

        // Step 4 — we are the ONE lucky task. Do the expensive operation.
        self.database_queries.fetch_add(1, Ordering::SeqCst);
        sleep(Duration::from_millis(100)).await;
        let status = "SUCCESS".to_string();

        // Step 5 — write the result into the cache so everyone else can
        // read it without another DB call.
        self.values
            .lock()
            .await
            .insert(key.to_string(), status.clone());

        status
    }
}

#[tokio::main]
async fn main() {
    let cache = Arc::new(Cache::new());
    let mut tasks = Vec::new();

    // Spawn 100 concurrent tasks, all requesting the SAME key.
    for _ in 0..100 {
        let cache = Arc::clone(&cache);
        tasks.push(tokio::spawn(async move {
            cache.get_status("transaction:101").await
        }));
    }

    // Wait for every task to finish.
    for task in tasks {
        let status = task.await.expect("request task failed");
        assert_eq!(status, "SUCCESS");
    }

    // Prove the database was queried only ONCE.
    let queries = cache.database_queries.load(Ordering::SeqCst);
    println!("Concurrent requests: 100");
    println!("Simulated database queries: {queries}");

    assert_eq!(queries, 1, "expected a single cache fill");
    println!("PASS: duplicate database reads were avoided");
}
