use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::file_handler::FileHandlerError;

use crate::{hledger::Hledger, file_handler::FileHandler};
use crate::config::Config;
use crate::sanitize::sanitize_journal_text;

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
    let hledger = Hledger::new();
    
    // Determine the journal file path (defaults to transfer_journal from config)
    let journal_path = if let Some(file) = &req.target_file {
        format!("{}/{}", config.nas_path, file)
    } else {
        format!("{}/{}", config.nas_path, config.transfer_journal)
    };
    
    // Build the transfer transaction (use local timezone instead of UTC)
    let date = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
    let payee = &req.payee;
    let amount = req.amount;
    
    let transaction = format!(
        "{} ! {}\n  {}  ${:.2}\n  {}  -${:.2}\n",
        date, payee, req.dest_account, amount, req.source_account, amount
    );
    
    // Validate the transaction first
    tracing::info!("Validating transaction: {}", transaction);
    tracing::info!("Journal path: {}", journal_path);
    match hledger.check(&journal_path) {
        Ok(()) => {
            // Transaction is valid, write to file
            match create_transfer_impl(&journal_path, &transaction) {
                Ok(()) => {
                    Json(TransferResponse {
                        valid: true,
                        message: "Transfer added successfully".to_string(),
                        transaction: Some(transaction),
                    }).into_response()
                }
                Err(e) => {
                    tracing::error!("Failed to write transfer: {}", e);
                    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Err(e) => {
            Json(TransferResponse {
                valid: false,
                message: format!("Transfer validation failed: {}", e),
                transaction: None,
            }).into_response()
        }
    }
}

/// Implementation of transfer creation (separate to avoid async)
fn create_transfer_impl(journal_path: &str, transaction: &str) -> Result<(), FileHandlerError> {
    // Create directory if needed
    if let Some(parent) = std::path::Path::new(journal_path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| FileHandlerError::CreateDirFailed(journal_path.to_string(), e))?;
    }

    // Format transaction with aligned columns
    let formatted_transaction = format_transaction(transaction);
    
    // Open file and append transaction
    let mut file = FileHandler::open(journal_path)?;
    file.append(&format!("{}\n\n", formatted_transaction))
}

/// Format transaction with aligned columns (62 character width)
fn format_transaction(transaction: &str) -> String {
    let mut lines = Vec::new();
    let mut first_line = true;
    
    for line in transaction.lines() {
        if first_line {
            // Sanitize the first line (date and payee) to ASCII-only so the
            // journal parses under any system locale. The payee is free text
            // (phone autocorrect can insert curly quotes); date and status
            // are ASCII and pass through unchanged. Posting lines below are
            // left exact: account names are structured identifiers.
            lines.push(sanitize_journal_text(line));
            first_line = false;
        } else if line.starts_with(' ') || line.starts_with('\t') {
            // Format posting lines
            let trimmed = line.trim_start();
            if let Some((account, amount)) = parse_posting_line(trimmed) {
                let formatted = format_posting_line(&account, &amount);
                lines.push(formatted);
            } else {
                lines.push(line.to_string());
            }
        } else {
            lines.push(line.to_string());
        }
    }
    
    lines.join("\n")
}

/// Parse a posting line into account and amount parts
fn parse_posting_line(line: &str) -> Option<(String, String)> {
    // Find the amount - look for $ symbol which marks the start of amount
    let dollar_pos = line.find('$')?;
    
    // Find the last whitespace before $
    let amount_start = line[..dollar_pos].rfind(|c: char| c.is_whitespace())
        .map_or(0, |pos| pos + 1);
    
    // Check if there's a - immediately before the amount
    let before_dollar = &line[..dollar_pos];
    let amount_start_with_sign = if before_dollar.trim_end().ends_with('-') {
        // The - is part of the amount, find the - position
        before_dollar.rfind('-').map_or(amount_start, |pos| pos)
    } else {
        amount_start
    };
    
    let account = line[..amount_start_with_sign].trim().to_string();
    let amount = line[amount_start_with_sign..].trim().to_string();
    
    if account.is_empty() {
        return None;
    }
    
    Some((account, amount))
}

/// Format a posting line with 62-character alignment
fn format_posting_line(account: &str, amount: &str) -> String {
    const COLUMN_START: usize = 62;
    
    // The line starts with 2 spaces for indentation
    let indent = "  ";
    let account_len = account.len();
    
    // Check if amount is negative (starts with -)
    let is_negative = amount.starts_with('-');
    let amount_without_sign = if is_negative { &amount[1..] } else { amount };
    
    // Calculate padding to align $ sign at column 62
    // Column 62 means position 61 (0-indexed)
    let padding_needed = COLUMN_START - 1 - indent.len() - account_len - if is_negative { 1 } else { 0 };
    
    if padding_needed > 0 {
        let sign = if is_negative { "-" } else { "" };
        format!("{}{}{}{}{}", indent, account, " ".repeat(padding_needed), sign, amount_without_sign)
    } else {
        // Account too long, just use default spacing
        format!("{} {}  {}", indent, account, amount)
    }
}
