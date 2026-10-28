use axum::{Json, response::IntoResponse};
use serde::Serialize;

use crate::hledger::Hledger;

/// Response for /api/payees
#[derive(Debug, Serialize)]
pub struct PayeesResponse {
    pub payees: Vec<String>,
}

/// Handler for /api/payees endpoint
pub async fn get_payees() -> impl IntoResponse {
    let hledger = Hledger::new();
    
    match hledger.payees() {
        Ok(payees) => Json(PayeesResponse { payees }).into_response(),
        Err(e) => {
            tracing::error!("Failed to get payees: {}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
