use crate::services::transaction_service::TransactionService;

#[derive(Clone)]
pub struct AppState {
    pub transaction_service: TransactionService,
}
