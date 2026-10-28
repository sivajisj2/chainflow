use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{interval, sleep, MissedTickBehavior};

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
            let mut connection = client.get_multiplexed_async_connection().await?;

            renew_lock(&mut connection, &lock_key, &lock_token).await
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

async fn process_job(worker_name: &str, mut stop_rx: watch::Receiver<bool>) -> bool {
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

async fn acquire_lock(
    connection: &mut redis::aio::MultiplexedConnection,
    lock_key: &str,
    owner: &str,
) -> redis::RedisResult<bool> {
    let result: Option<String> = redis::cmd("SET")
        .arg(lock_key)
        .arg(owner)
        .arg("NX")
        .arg("EX")
        .arg(30)
        .query_async(connection)
        .await?;

    Ok(result.is_some())
}

async fn release_lock(
    connection: &mut redis::aio::MultiplexedConnection,
    lock_key: &str,
    owner: &str,
) -> redis::RedisResult<bool> {
    let script = redis::Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("DEL", KEYS[1])
        else
            return 0
        end
        "#,
    );

    let deleted: i32 = script
        .key(lock_key)
        .arg(owner)
        .invoke_async(connection)
        .await?;

    Ok(deleted == 1)
}

async fn run_worker(worker_name: &'static str, client: redis::Client) -> redis::RedisResult<()> {
    let lock_token = uuid::Uuid::new_v4().to_string();

    println!("{worker_name}: Token = {lock_token}");
    let mut connection = client.get_multiplexed_async_connection().await?;

    let acquired = acquire_lock(&mut connection, "lock:txn-500", &lock_token).await?;

    if acquired {
        println!("{worker_name}: Lock acquired");

        let (stop_tx, stop_rx) = watch::channel(false);

        let heartbeat_handle = tokio::spawn(heartbeat(
            client.clone(),
            "lock:txn-500".to_string(),
            lock_token.clone(),
            stop_tx.clone(),
        ));

        let processing_completed = process_job(worker_name, stop_rx).await;

        // Ask the heartbeat task to stop after processing.
        heartbeat_handle.abort();
        let _ = heartbeat_handle.await;

        if processing_completed {
            println!("{worker_name}: processing completed");
        } else {
            println!("{worker_name}: processing stopped");
        }

        let released = release_lock(&mut connection, "lock:txn-500", &lock_token).await?;

        println!("{worker_name}: lock released = {released}");

        println!("{worker_name}: Finished processing");

        let released = release_lock(&mut connection, "lock:txn-500", &lock_token).await?;

        if released {
            println!("{worker_name}: Lock released safely");
        } else {
            println!("{worker_name}: Lock not released - ownership lost");
        }
    } else {
        println!("{worker_name}: Lock unavailable, skipping");
    }

    Ok(())
}

async fn renew_lock(
    connection: &mut redis::aio::MultiplexedConnection,
    lock_key: &str,
    lock_token: &str,
) -> redis::RedisResult<bool> {
    let script = redis::Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("EXPIRE", KEYS[1], 30)
        else
            return 0
        end
        "#,
    );

    let renewed: i32 = script
        .key(lock_key)
        .arg(lock_token)
        .invoke_async(connection)
        .await?;

    Ok(renewed == 1)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = redis::Client::open("redis://127.0.0.1:6379/")?;

    let worker_a = tokio::spawn(run_worker("worker-A", client.clone()));

    let worker_b = tokio::spawn(run_worker("worker-B", client.clone()));

    worker_a.await??;
    worker_b.await??;

    println!("Both workers completed");

    Ok(())
}
