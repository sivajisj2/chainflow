use std::time::Duration;
use tokio::time::sleep;
use tonic::transport::Endpoint;
use tonic::Code;
use tonic::Request;
pub mod chainflow {
    tonic::include_proto!("chainflow");
}

use chainflow::transaction_service_client::TransactionServiceClient;
use chainflow::TransactionRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = Endpoint::from_static("http://127.0.0.1:50051")
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(2))
        .connect()
        .await?;

    let mut client = TransactionServiceClient::new(channel);

    println!("Connected to ChainFlow gRPC server");

    let max_attempts = 4;

    for attempt in 1..=max_attempts {
        println!("Attempt {attempt}/{max_attempts}");

        let request = Request::new(TransactionRequest {
            transaction_id: "txn-500".to_string(),
        });

        match client.get_transaction_status(request).await {
            Ok(response) => {
                let transaction = response.into_inner();

                println!("Transaction ID: {}", transaction.transaction_id);
                println!("Transaction Status: {}", transaction.status);

                break;
            }

            Err(status) => {
                println!("Request failed: {:?} - {}", status.code(), status.message());

                let retryable = matches!(
                    status.code(),
                    Code::Unavailable | Code::DeadlineExceeded | Code::Cancelled
                );

                if !retryable || attempt == max_attempts {
                    println!("Stopping retries");
                    break;
                }

                let delay_ms = 500_u64 * 2_u64.pow(attempt - 1);

                println!("Retrying in {delay_ms} ms");

                sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }

    Ok(())
}
