use axum::{response::Json, extract::State};
use std::sync::Arc;

use crate::config::Config;

/// Transfer account for UI dropdowns
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct TransferAccountResponse {
    pub id: String,
    pub label: String,
    pub journal: String,
}

/// Get available transfer accounts for UI dropdowns
pub async fn get_transfer_accounts(
    State(config): State<Arc<Config>>,
) -> Json<Vec<TransferAccountResponse>> {
    // Transfer accounts are those under:
    // - assets:Bank:Checking:Budget:*
    // - assets:Bank:Savings:Budget:*
    // - liabilities:CC:*
    
    let mut accounts = Vec::new();
    
    // Build the journal path
    let journal_path = format!("{}/{}", config.nas_path, config.top_journal);
    
    // Create hledger instance and run accounts command
    let hledger = crate::hledger::Hledger::new();
    
    match hledger.accounts(Some(&journal_path)) {
        Ok(all_accounts) => {
            // Filter for transfer accounts
            for account in &all_accounts {
                // Check if it's under Checking:Budget, Savings:Budget, or CC
                if account.starts_with("assets:Bank:Checking:Budget:") 
                    || account.starts_with("assets:Bank:Savings:Budget:")
                    || account.starts_with("liabilities:CC:") {
                    
                    // Extract display name (last segment)
                    let parts: Vec<&str> = account.split(':').collect();
                    if let Some(last_part) = parts.last() {
                        // Map account to journal based on payment sources config
                        let journal = find_journal_for_account(&config, account);
                        
                        accounts.push(TransferAccountResponse {
                            id: account.clone(),
                            label: last_part.to_string(),
                            journal,
                        });
                    }
                }
            }
            // Sort accounts alphabetically by label
            accounts.sort_by(|a, b| a.label.cmp(&b.label));
        }
        Err(e) => {
            tracing::error!("Failed to get accounts from hledger: {}", e);
        }
    }
    
    Json(accounts)
}

/// Find the journal file for a transfer account
fn find_journal_for_account(config: &Config, account: &str) -> String {
    // Check against payment sources config
    for (payment_source_id, source) in &config.payment_sources {
        if account.starts_with(payment_source_id) {
            return source.journal.clone();
        }
    }
    
    // Fall back to the configured transfer journal for transfer accounts
    config.transfer_journal.clone()
}
