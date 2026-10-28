// This example shows a retry-with-backoff pattern for unreliable operations.
//
// Why backoff? If a service is flaky, hammering it every 100ms wastes
// resources. Doubling the delay gives it time to recover: 1s, 2s, 4s, 8s...

use tokio::time::{sleep, Duration};

// Simulates a worker that fails on the first two attempts
// but succeeds on attempt #3.
async fn run_worker(attempt: u32) -> Result<(), String> {
    println!("Worker attempt {attempt} started");

    // Simulate some work (e.g., a network call).
    sleep(Duration::from_millis(200)).await;

    if attempt < 3 {
        return Err(format!("simulated failure on attempt {attempt}"));
    }

    println!("Worker completed successfully");
    Ok(())
}

#[tokio::main]
async fn main() {
    let max_attempts = 3;
    let mut delay_secs = 1;

    for attempt in 1..=max_attempts {
        match run_worker(attempt).await {
            Ok(()) => {
                // Success — stop trying.
                println!("Supervisor: worker finished");
                return;
            }
            Err(error) => {
                println!("Supervisor: {error}");

                if attempt == max_attempts {
                    println!("Supervisor: restart limit reached");
                    break;
                }

                // Wait before the next attempt.
                println!("Supervisor: retrying in {delay_secs}s");
                sleep(Duration::from_secs(delay_secs)).await;

                // Double the delay for the next retry (exponential backoff).
                // Cap it at 8 seconds so we don't wait forever.
                delay_secs = (delay_secs * 2).min(8);
            }
        }
    }

    println!("Supervisor: giving up; operator attention may be needed");
}
