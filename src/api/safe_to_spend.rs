use crate::config;
use axum::{response::Json, extract::State};
use serde::Serialize;
use std::sync::Arc;

/// Response for /api/safe-to-spend endpoint
/// Returns the current balance in the Unallocated budget account  
#[derive(Debug, Serialize)]
pub struct SafeToSpendResponse {
    /// Configured safe-to-spend account path
    pub account: String,
    /// Signed balance of the safe-to-spend (Unallocated) account
    pub unallocated: f64,
    /// Sum of negative balances among budget envelopes in the safe-to-spend header group (<= 0)
    pub overspent: f64,
    /// unallocated + overspent: the actual safe-to-spend amount
    pub balance: f64,
}

pub async fn get_safe_to_spend(
    State(config_ref): State<Arc<config::Config>>,
) -> Json<SafeToSpendResponse> {
    use crate::api::expense_totals::query_hledger_balances_batch;
    let journal_path = format!("{}/{}", config_ref.nas_path, config_ref.top_journal);

    // PERF: fetch all needed balances in ONE hledger invocation instead of N sequential calls
    let mut accounts = vec![config_ref.safe_to_spend_account.clone()];
    for entry in &config_ref.budget_entries {
        if entry.type_ == "budget" && entry.header == config_ref.safe_to_spend_header {
            accounts.push(entry.account.clone());
        }
    }
    let balances = query_hledger_balances_batch(&journal_path, &accounts);
    let unallocated = balances.get(&config_ref.safe_to_spend_account).copied().unwrap_or(0.0);

    // Total up underwater (negative-balance) budget envelopes in the header group.
    // Liabilities are excluded: they are debt, not envelope overspending.
    let mut overspent = 0.0;
    for entry in &config_ref.budget_entries {
        if entry.type_ == "budget" && entry.header == config_ref.safe_to_spend_header {
            if let Some(&b) = balances.get(&entry.account) {
                if b < 0.0 {
                    overspent += b;
                }
            }
        }
    }

    let unallocated = (unallocated * 100.0).round() / 100.0;
    let overspent = (overspent * 100.0).round() / 100.0;
    let balance = ((unallocated + overspent) * 100.0).round() / 100.0;
    Json(SafeToSpendResponse {
        account: config_ref.safe_to_spend_account.clone(),
        unallocated,
        overspent,
        balance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_safe_to_spend_response_serialize() {
        let response = SafeToSpendResponse {
            account: "assets:Bank:Checking:Budget:Unallocated".to_string(),
            unallocated: 66.54,
            overspent: -10.0,
            balance: 56.54,
        };
        
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("Unallocated"));
    }
}
