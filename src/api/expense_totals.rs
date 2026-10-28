use axum::{response::Json, extract::State};
use serde::Serialize;
use std::sync::Arc;

/// Budget or liability spending total with computed balance data  
#[derive(Debug, Serialize)]
pub struct ExpenseTotal {
    /// Full account path from config
    pub account: String,     
    /// User-friendly display name from config label field
    pub display_name: String,     
    /// Grouping title for tab organization ("Checking", "Savings", etc.)  
    pub header: String,           
    /// Monthly budget limit or liability credit amount (positive)
    pub budget_limit: f64,        
    /// Spent total from hledger query (monthly sum, non-negative)
    pub spent_total: f64,
    /// Signed hledger balance (negative = underwater envelope; for liabilities this is negative owed)
    pub balance: f64,
    /// Remaining balance = budget_limit - spent_total
    pub remaining: f64,           
    /// Entry kind for progress bar color styling ("budget" or "liabilities")
    /// Canonical value is plural "liabilities", accepting singular input for compatibility
    pub kind: String,             
}

/// Response wrapper including per-header account balances
#[derive(Debug, Serialize)]
pub struct ExpenseTotalsResponse {
    pub entries: Vec<ExpenseTotal>,
    /// Map of header name -> hledger balance (e.g., "Checking" -> 1234.56)
    pub header_balances: std::collections::HashMap<String, f64>,
}

/// Parse balance string like "$123.45  path..." and extract f64 amount
fn parse_balance(s: &str) -> f64 {
    s.split_whitespace()
        .next()
        .map(|tok| {
            let cleaned = tok.trim_start_matches('$').replace(',', "");
            cleaned.parse::<f64>().unwrap_or(0.0)
        })
        .unwrap_or(0.0)
}

/// Query hledger for the SIGNED account balance by exact account path.
/// Negative = underwater envelope (overspent) or owed liability.
pub(crate) fn query_hledger_signed_balance(journal_path: &str, account: &str) -> f64 {
    match crate::hledger::Hledger::new().balance(account, None, Some(journal_path)) {
        Ok(balance_str) => parse_balance(&balance_str),
        Err(_) => 0.0,
    }
}

/// Query hledger for individual account balance by exact account path (Fix 3: direct query not prefix aggregation)
fn query_hledger_balance(journal_path: &str, account: &str) -> f64 {
    // For liabilities, use abs() so spent_total is non-negative (per struct doc)
    query_hledger_signed_balance(journal_path, account).abs()
}

/// Batch query hledger for MULTIPLE account balances in a SINGLE invocation.
/// Returns a HashMap from account path to signed balance.
/// This replaces N sequential `hledger balance <account>` calls with one
/// `hledger balance --flat <acct1> <acct2> ...` call. Accounts not found in
/// hledger output are absent from the map (callers should default to 0.0).
pub(crate) fn query_hledger_balances_batch(
    journal_path: &str,
    accounts: &[String],
) -> std::collections::HashMap<String, f64> {
    let mut map = std::collections::HashMap::new();
    if accounts.is_empty() {
        return map;
    }
    match crate::hledger::Hledger::new().balances_flat(accounts, Some(journal_path)) {
        Ok(output) => {
            for line in output.lines() {
                let line = line.trim();
                // Skip empty lines, separator rows (----), and the total row (amount only, no account)
                if line.is_empty() || line.starts_with('-') {
                    continue;
                }
                // Format: "$-7,050.00  liabilities:CC:Example Card"
                // Amount = first whitespace-separated token; account = remainder
                let mut parts = line.splitn(2, char::is_whitespace);
                match (parts.next(), parts.next()) {
                    (Some(_amt_tok), Some(acct)) => {
                        let acct = acct.trim();
                        if !acct.is_empty() {
                            map.insert(acct.to_string(), parse_balance(line));
                        }
                    }
                    _ => continue, // total row or malformed: skip
                }
            }
        }
        Err(_) => {} // on error return empty map; callers default missing accounts to 0.0
    }
    map
}

pub async fn get_expense_totals(
    State(config): State<Arc<crate::config::Config>>,
) -> Json<ExpenseTotalsResponse> { 
    let mut totals = Vec::new();
    let journal_path = format!("{}/{}", config.nas_path, config.top_journal);

    // PERF: fetch all balances in ONE hledger invocation instead of N sequential calls
    let mut accounts: Vec<String> = config.budget_entries.iter()
        .map(|e| e.account.clone())
        .collect();
    // Add header-level accounts (e.g., assets:Bank:Checking) for header balance display
    // Derived from budget_locations by stripping trailing ":"
    let header_accounts: Vec<(String, String)> = config.budget_locations.iter()
        .filter_map(|loc| {
            let acct = loc.trim_end_matches(':').to_string();
            // Header name is the last path component (e.g., "assets:Bank:Checking" -> "Checking")
            let header = acct.rsplit(':').next()?.to_string();
            Some((header, acct))
        })
        .collect();
    for (_, acct) in &header_accounts {
        accounts.push(acct.clone());
    }
    let balances = query_hledger_balances_batch(&journal_path, &accounts);
    
    for entry in &config.budget_entries {
        // Look up pre-fetched balance (0.0 if account not in hledger output)
        let balance = balances.get(&entry.account).copied().unwrap_or(0.0);
        let spent_total = balance.abs();

        totals.push(ExpenseTotal {
            account: entry.account.clone(),
            display_name: entry.label.clone(),
            header: entry.header.to_string(),
            budget_limit: ((entry.amount.abs() * 100.0).round() / 100.0),
            spent_total,
            balance,
            // Compute remaining as budget_limit - abs(spent_total) for both types
            // For budgets: remaining negative = overspent; positive = underbudget
            // For liabilities: remaining always positive (owed minus what's been "spent")
            remaining: ((entry.amount.abs() - spent_total) * 100.0).round() / 100.0,
            kind: entry.type_.clone(),          // field is `type_` not `kind`
        });
    }

    totals.sort_by(|a, b| a.header.cmp(&b.header).then_with(|| a.display_name.cmp(&b.display_name)));

    // Build header balances map
    let mut header_balances = std::collections::HashMap::new();
    for (header, acct) in &header_accounts {
        let prefix = format!("{}:", acct);
        let mut bal = balances.get(acct).copied().unwrap_or(0.0);
        for (acc_name, acc_bal) in balances.iter() {
            if acc_name.starts_with(&prefix) {
                bal += acc_bal;
            }
        }
        header_balances.insert(header.clone(), (bal * 100.0).round() / 100.0);
    }

    Json(ExpenseTotalsResponse { entries: totals, header_balances })
}