use axum::{Json, response::IntoResponse};
use serde::Serialize;

use crate::hledger::Hledger;

/// Response for /api/accounts
#[derive(Debug, Serialize)]
pub struct AccountsResponse {
    pub accounts: Vec<String>,
}

/// Handler for /api/accounts endpoint
pub async fn get_accounts() -> impl IntoResponse {
    let hledger = Hledger::new();
    
    match hledger.accounts(None) {
        Ok(accounts) => Json(AccountsResponse { accounts }).into_response(),
        Err(e) => {
            tracing::error!("Failed to get accounts: {}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
