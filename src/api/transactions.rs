use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::file_handler::FileHandlerError;

use crate::{hledger::Hledger, file_handler::FileHandler};
use crate::config::Config;

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
    let hledger = Hledger::new();
    
    // Determine the journal file path
    let journal_path = determine_journal_path(&config, &req);
    
    // Validate the transaction first
    match hledger.check(&journal_path) {
        Ok(()) => {
            // Transaction is valid, write to file
            match create_transaction_impl(&journal_path, &req.transaction) {
                Ok(()) => {
                    Json(TransactionResponse {
                        valid: true,
                        message: "Transaction added successfully".to_string(),
                        transaction: Some(req.transaction),
                    }).into_response()
                }
                Err(e) => {
                    tracing::error!("Failed to write transaction: {}", e);
                    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Err(e) => {
            Json(TransactionResponse {
                valid: false,
                message: format!("Transaction validation failed: {}", e),
                transaction: None,
            }).into_response()
        }
    }
}

/// Determine which journal file to write to
/// Uses journal_routing to match accounts to specific journals
/// Falls back to top_journal if no match found
#[cfg_attr(test, visibility::make(pub))] pub fn determine_journal_path(config: &Config, req: &TransactionRequest) -> String {
    // If target_file is specified, use it directly
    if let Some(file) = &req.target_file {
        return format!("{}/{}", config.nas_path, file);
    }
    
    // Extract account names from the transaction
    let accounts = extract_accounts_from_transaction(&req.transaction);
    
    // Try to find a matching route
    for account in accounts {
        if let Some(journal_file) = find_journal_route(config, &account) {
            // journal_file already includes year path (e.g., "2026/example-card.journal")
            return format!("{}/{}", config.nas_path, journal_file);
        }
    }
    
    // Default to top_journal if no route match
    format!("{}/{}", config.nas_path, config.top_journal)
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
fn create_transaction_impl(journal_path: &str, transaction: &str) -> Result<(), FileHandlerError> {
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
            // Keep the first line (date and payee) as-is
            lines.push(line.to_string());
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
    const COLUMN_START: usize = 67;
    
    // The line starts with 2 spaces for indentation
    let indent = "  ";
    let account_len = account.len();
    
    // Align start of amount field to column 62 (1-indexed)
    // Position 0-indexed = COLUMN_START - 1
    let target_pos = COLUMN_START - 1;
    let padding_needed = target_pos.saturating_sub(indent.len() + account_len);
    
    if padding_needed > 0 {
        format!("{}{}{}{}", indent, account, " ".repeat(padding_needed), amount)
    } else {
        // Account too long, just use default spacing
        format!("{} {}  {}", indent, account, amount)
    }
}
