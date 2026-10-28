use tonic::{transport::Server, Request, Response, Status};

pub mod chainflow {
    tonic::include_proto!("chainflow");
}

use chainflow::transaction_service_server::{TransactionService, TransactionServiceServer};

use chainflow::{TransactionRequest, TransactionResponse};

#[derive(Default)]
struct MyTransactionService;

#[tonic::async_trait]
impl TransactionService for MyTransactionService {
    async fn get_transaction_status(
        &self,
        request: Request<TransactionRequest>,
    ) -> Result<Response<TransactionResponse>, Status> {
        let transaction_id = request.into_inner().transaction_id;

        if transaction_id.trim().is_empty() {
            return Err(Status::invalid_argument("transaction_id cannot be empty"));
        }

        println!("Received transaction: {transaction_id}");

        // tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let response = TransactionResponse {
            transaction_id,
            status: "PENDING".to_string(),
        };

        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = "127.0.0.1:50051".parse()?;

    let service = MyTransactionService::default();

    println!("ChainFlow gRPC server listening on {address}");

    Server::builder()
        .add_service(TransactionServiceServer::new(service))
        .serve(address)
        .await?;

    Ok(())
}
