use axum::http::StatusCode;
use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::file_handler::resolve_target_file;
use crate::journal_write::{append_validated, JournalWriteError};

/// Response for /api/transactions (validation result)
#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    pub valid: bool,
    pub message: String,
    pub transaction: Option<String>,
}

/// Request body for creating a transaction
#[derive(Debug, Deserialize)]
pub struct TransactionRequest {
    /// The transaction to add (ledger format)
    pub transaction: String,
    /// Target journal file (optional, auto-determined from config if not provided)
    pub target_file: Option<String>,
}

/// Handler for /api/transactions POST endpoint
pub async fn create_transaction(
    axum::extract::State(config): axum::extract::State<std::sync::Arc<Config>>,
    Json(req): Json<TransactionRequest>,
) -> impl IntoResponse {
    // A client-supplied target_file is resolved strictly under nas_path;
    // anything escaping it is rejected before any filesystem access.
    let journal_path = match determine_journal_path(&config, &req) {
        Ok(path) => path,
        Err(msg) => {
            return Json(TransactionResponse {
                valid: false,
                message: format!("Invalid target_file: {}", msg),
                transaction: None,
            })
            .into_response()
        }
    };

    // The staged hledger check inside append_validated gates the write:
    // only content that parses and balances against the existing journal
    // is appended. Anything else is a validation failure, not a write.
    match create_transaction_impl(&journal_path, &req.transaction) {
        Ok(()) => Json(TransactionResponse {
            valid: true,
            message: "Transaction added successfully".to_string(),
            transaction: Some(req.transaction),
        })
        .into_response(),
        Err(JournalWriteError::File(msg)) => {
            // Server-side log keeps the detail; the client gets a
            // generic message so filesystem paths never leak.
            tracing::error!("Failed to write transaction: {}", msg);
            (StatusCode::INTERNAL_SERVER_ERROR, "Journal file I/O error").into_response()
        }
        Err(e) => Json(TransactionResponse {
            valid: false,
            message: format!("Transaction validation failed: {}", e),
            transaction: None,
        })
        .into_response(),
    }
}


/// Determine which journal file to write to
/// Uses journal_routing to match accounts to specific journals
/// Falls back to top_journal if no match found
#[cfg_attr(test, visibility::make(pub))] pub fn determine_journal_path(config: &Config, req: &TransactionRequest) -> Result<String, String> {
    // If target_file is specified, resolve it strictly under nas_path:
    // absolute paths and `..` escapes are rejected (see resolve_target_file).
    if let Some(file) = &req.target_file {
        return resolve_target_file(&config.nas_path, file);
    }

    // Extract account names from the transaction
    let accounts = extract_accounts_from_transaction(&req.transaction);

    // Try to find a matching route
    for account in accounts {
        if let Some(journal_file) = find_journal_route(config, &account) {
            // journal_file already includes year path (e.g., "2026/example-card.journal")
            return Ok(format!("{}/{}", config.nas_path, journal_file));
        }
    }

    // Default to top_journal if no route match
    Ok(format!("{}/{}", config.nas_path, config.top_journal))
}


/// Extract account names from a transaction string
#[cfg_attr(test, visibility::make(pub))] pub fn extract_accounts_from_transaction(transaction: &str) -> Vec<String> {
    let mut accounts = Vec::new();
    
    for line in transaction.lines() {
        // Skip comment lines and date/pattern lines
        if line.starts_with(';') || line.trim().starts_with('*') || line.trim().starts_with('"') {
            continue;
        }
        
        // Check for account posting (starts with whitespace, contains account path)
        let trimmed = line.trim();
        
        // First try to extract full account paths (containing ':')
        // Accounts can have spaces (e.g., "expenses:Pet Supplies"), so find account until amount
        if trimmed.contains(':') && trimmed.split_whitespace().next().map_or(false, |w| !w.starts_with('$')) {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            let mut account_parts = Vec::new();
            let mut in_account = false;
            
            for part in parts {
                if part.contains(':') && !in_account {
                    in_account = true;
                    account_parts.push(part.to_string());
                } else if in_account && !part.starts_with('$') && !part.starts_with('-') {
                    account_parts.push(part.to_string());
                } else if in_account && (part.starts_with('$') || part.starts_with('-')) {
                    break; // Hit amount, stop
                }
            }
            
            if !account_parts.is_empty() {
                accounts.push(account_parts.join(" "));
            }
        } else if trimmed.split_whitespace().next().map_or(false, |w| !w.starts_with('$') && !w.starts_with('-')) {
            // If no colon, extract simple account names (like 'cash')
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            for part in parts {
                if !part.starts_with('$') && !part.starts_with('-') && !part.contains('/') && !part.contains('\'') && !part.contains('"') {
                    // Likely an account name - add it
                    accounts.push(part.to_string());
                    break;
                }
            }
        }
    }
    
    accounts
}

/// Find journal route for an account (longest prefix match)
#[cfg_attr(test, visibility::make(pub))] pub fn find_journal_route(config: &Config, account: &str) -> Option<String> {
    let mut best_match: Option<String> = None;
    let mut best_length = 0;
    
    for (payment_source_id, source) in &config.payment_sources {
        let prefix_len = payment_source_id.len();
        
        // Check if account starts with the payment source prefix
        if account.starts_with(payment_source_id) {
            if prefix_len > best_length {
                best_length = prefix_len;
                best_match = Some(source.journal.clone());
            }
        }
    }
    
    best_match
}

/// Implementation of transaction creation (separate to avoid async)
fn create_transaction_impl(journal_path: &str, transaction: &str) -> Result<(), JournalWriteError> {
    append_validated(journal_path, transaction)
}
