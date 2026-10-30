use axum::http::StatusCode;
use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::file_handler::resolve_target_file;
use crate::journal_write::{
    append_validated, validate_account_name, validate_payee, JournalWriteError,
};

/// Response for /api/transfers (validation result)
#[derive(Debug, Serialize)]
pub struct TransferResponse {
    pub valid: bool,
    pub message: String,
    pub transaction: Option<String>,
}

/// Request body for creating a transfer
#[derive(Debug, Deserialize, Serialize)]
pub struct TransferRequest {
    /// Source account
    pub source_account: String,
    /// Destination account
    pub dest_account: String,
    /// Amount to transfer
    pub amount: f64,
    /// Payee/description for the transfer
    pub payee: String,
    /// Target journal file (defaults to transfer_journal from config, which is required)
    pub target_file: Option<String>,
}

/// Handler for /api/transfers POST endpoint
pub async fn create_transfer(
    axum::extract::State(config): axum::extract::State<std::sync::Arc<Config>>,
    Json(req): Json<TransferRequest>,
) -> impl IntoResponse {
    // A client-supplied target_file is resolved strictly under nas_path;
    // anything escaping it is rejected before any filesystem access.
    let journal_path = match req.target_file.as_ref() {
        Some(file) => match resolve_target_file(&config.nas_path, file) {
            Ok(path) => path,
            Err(msg) => {
                return Json(TransferResponse {
                    valid: false,
                    message: format!("Invalid target_file: {}", msg),
                    transaction: None,
                })
                .into_response()
            }
        },
        None => format!("{}/{}", config.nas_path, config.transfer_journal),
    };

    // Field-level validation: newlines or control characters in the payee
    // or account names would split the built transaction into forged
    // lines, and `;`/`$` in an account name would break posting parsing.
    if let Err(msg) = validate_payee(&req.payee)
        .and_then(|_| validate_account_name(&req.source_account))
        .and_then(|_| validate_account_name(&req.dest_account))
    {
        return Json(TransferResponse {
            valid: false,
            message: format!("Invalid transfer: {}", msg),
            transaction: None,
        })
        .into_response();
    }

    // Build the transfer transaction (use local timezone instead of UTC)
    let date = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
    let payee = &req.payee;
    let amount = req.amount;

    let transaction = format!(
        "{} ! {}\n  {}  ${:.2}\n  {}  -${:.2}\n",
        date, payee, req.dest_account, amount, req.source_account, amount
    );

    // Append through the staged-check gate (see append_validated).
    tracing::info!("Validating transaction: {}", transaction);
    tracing::info!("Journal path: {}", journal_path);
    match create_transfer_impl(&journal_path, &transaction) {
        Ok(()) => Json(TransferResponse {
            valid: true,
            message: "Transfer added successfully".to_string(),
            transaction: Some(transaction),
        })
        .into_response(),
        Err(JournalWriteError::File(msg)) => {
            tracing::error!("Failed to write transfer: {}", msg);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Journal file I/O error: {}", msg),
            )
                .into_response()
        }
        Err(e) => Json(TransferResponse {
            valid: false,
            message: format!("Transfer validation failed: {}", e),
            transaction: None,
        })
        .into_response(),
    }
}


/// Implementation of transfer creation (separate to avoid async)
fn create_transfer_impl(journal_path: &str, transaction: &str) -> Result<(), JournalWriteError> {
    append_validated(journal_path, transaction)
}
