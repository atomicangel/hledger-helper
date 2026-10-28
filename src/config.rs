use serde::{Deserialize, Serialize};
use std::fs;

/// Configuration for the hledger-helper backend
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    /// Path to the NAS mount containing hledger journals
    pub nas_path: String,
    /// Top-level journal file (entry point for hledger)
    pub top_journal: String,
    /// Current year for journal file routing
    pub current_year: u16,
    /// Budget account locations (prefixes for budget accounts)
    pub budget_locations: Vec<String>,
    /// Liability account locations (prefixes for liability accounts)
    pub liability_locations: Vec<String>,
    /// Credit account locations (prefixes for accounts you charge purchases
    /// to and later pay off: credit cards, lines of credit, BNPL/point-of-sale
    /// financing). A Quick Entry split whose source matches one of these
    /// prefixes shows the auto-cover UI. Paydown-only debts (IRS, student
    /// loans, mortgages) are NOT spending sources and must not be listed here.
    /// Missing key = no cover UI (fail-closed).
    #[serde(default)]
    pub credit_locations: Vec<String>,
    /// Expense account locations (prefixes for expense accounts)
    pub expense_locations: Vec<String>,
    /// Account whose balance is shown as "safe to spend" on Budget tab for Phase 1 /api/safe-to-spend endpoint
    #[serde(default = "default_safe_spend_account")]  
    pub safe_to_spend_account: String,
    /// Header group whose underwater (negative-balance) envelopes count against Safe-to-Spend
    #[serde(default = "default_safe_spend_header")]
    pub safe_to_spend_header: String,
    /// Static budget limits per account path (legacy flat-map YAML format)
    #[serde(default)]
    pub budget_limits: std::collections::HashMap<String, f64>,
    /// Budget entries from array config section
    #[serde(default = "Vec::new")]  
    pub budget_entries: Vec<BudgetEntry>,
    /// Payment sources for UI buttons and journal routing
    pub payment_sources: std::collections::HashMap<String, PaymentSource>,
    /// Transfer journal file for account transfers. Required: the
    /// server refuses to start if this key is missing or empty (there
    /// is no hardcoded fallback). A defined file is created at startup
    /// if it does not exist yet.
    #[serde(default)]
    pub transfer_journal: String,
    /// Server configuration
    pub server: ServerConfig,
}

fn default_safe_spend_account() -> String {
    // Return empty string if YAML key missing; user should add to config.yaml
    String::new()
}

fn default_safe_spend_header() -> String {
    "Checking".to_string()
}

/// Payment source configuration for UI buttons
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PaymentSource {
    pub label: String,
    pub journal: String,
}

/// Budget or liability entry from config.yaml (array format)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BudgetEntry {
    /// Full hledger account path
    pub account: String,
    /// User-friendly display name for UI cards
    #[serde(alias = "label")]
    pub label: String,
    /// Monthly budget limit or liability credit (positive for budgets, negative/debt for liabilities)
    pub amount: f64,
    /// Grouping header ("Checking", "Savings", etc.)
    pub header: String,
    /// Entry type: "budget" or "liability"  
    #[serde(alias = "type")]
    #[serde(default = "default_type")]
    pub type_: String,
}

fn default_type() -> String {
    "budget".to_string()
}

/// Budget category with full account path and display name (legacy Category)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Category {
    pub full_path: String,
    pub display_name: String,
    pub category_type: String,
}

/// Server configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    pub bind_address: String,    
    pub port: u16,
}

/// Display name for a budget-location account suffix: the full remainder
/// after the location prefix, so parents show ("Vehicle:Fuel"). A leading
/// "Budget" segment is the envelope container, not a name, so it is dropped.
/// Segment-exact: "RandomBudget" and "Budgeting" are untouched. The container
/// account itself ("Budget" alone) yields "".
fn budget_display_name(suffix: &str) -> String {
    let mut segments: Vec<&str> = suffix.split(':').filter(|s| !s.is_empty()).collect();
    if segments.first() == Some(&"Budget") {
        segments.remove(0);
    }
    segments.join(":")
}

impl Config {
    /// Load configuration from a YAML file
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)
            .map_err(|e| ConfigError::ReadFailed(path.to_string(), e))?;

        let config: Self = serde_yaml::from_str(&content)
            .map_err(|e| ConfigError::ParseFailed(path.to_string(), e))?;

        // transfer_journal is required: no hardcoded fallback exists, so
        // a missing or empty value is a startup error that forces the
        // operator to define it. (Creating a defined-but-missing file is
        // a startup step in main via ensure_transfer_journal, not part
        // of load, so loading a config never requires write access.)
        if config.transfer_journal.trim().is_empty() {
            return Err(ConfigError::TransferJournalMissing);
        }

        Ok(config)
    }

    /// Ensure the defined transfer journal file exists, creating parent
    /// directories and the file itself if needed. Called once at server
    /// startup (main), not from load, so loading a config -- including
    /// in tests with fake paths -- never requires write access.
    pub fn ensure_transfer_journal(&self) -> Result<(), ConfigError> {
        let journal_path = format!("{}/{}", self.nas_path, self.transfer_journal);
        if let Some(parent) = std::path::Path::new(&journal_path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ConfigError::TransferJournalCreateFailed(journal_path.clone(), e))?;
        }
        if !std::path::Path::new(&journal_path).exists() {
            fs::File::create(&journal_path)
                .map_err(|e| ConfigError::TransferJournalCreateFailed(journal_path.clone(), e))?;
        }
        Ok(())
    }

    /// Get all budget entries from config (array-only migration complete).
    pub fn get_all_budget_entries(&self) -> Vec<BudgetEntry> {
        let mut entries = self.budget_entries.clone();
        // Sort by header then label for consistent ordering
        entries.sort_by(|a, b| a.header.cmp(&b.header).then_with(|| a.label.cmp(&b.label)));  
        entries
    }



    /// Get the full path to a journal file
    #[allow(dead_code)] // used by integration tests via the lib crate
    pub fn journal_path(&self, file: &str) -> String {
        format!("{}/{}/{}", self.nas_path, self.current_year, file)
    }

    /// Get payment sources as a vector for API responses
    #[allow(dead_code)] // used by integration tests via the lib crate
    pub fn get_payment_sources(&self) -> Vec<PaymentSource> {
        self.payment_sources.values().cloned().collect()
    }

    /// Query hledger for budget accounts and extract categories
    pub fn get_categories_from_hledger(&self) -> Vec<Category> {
        use crate::hledger::Hledger;
        
        // Build the journal path
        let journal_path = format!("{}/{}", self.nas_path, self.top_journal);
        
        tracing::debug!("Journal path from config: {}", journal_path);
        tracing::debug!("NAS path: {}", self.nas_path);
        tracing::debug!("Top journal: {}", self.top_journal);
        
        // Create hledger instance and run accounts command with journal file
        let hledger = Hledger::new();
        
        match hledger.accounts(Some(&journal_path)) {
            Ok(accounts) => {
                tracing::debug!("Got {} accounts from hledger", accounts.len());
                for acc in &accounts {
                    tracing::trace!("Account: {}", acc);
                }
                let mut categories_vec: Vec<Category> = Vec::new();
                
                // Filter accounts matching budget locations and extract categories
                for account in &accounts {
                    let mut matched = false;
                    for prefix in &self.budget_locations {
                        let prefix_trimmed = prefix.trim_end_matches(':');
                        if account.starts_with(prefix_trimmed) {
                            tracing::debug!("Matched budget prefix '{}' for account: {}", prefix_trimmed, account);
                            // Get the part after the prefix
                            if let Some(suffix) = account.strip_prefix(prefix_trimmed) {
                                // Full remainder after the location prefix, so
                                // parents show (e.g. "Vehicle:Fuel"). A leading
                                // "Budget" segment is the envelope container,
                                // not a name, so it is dropped. Segment-exact:
                                // "RandomBudget" and "Budgeting" are untouched.
                                // The container account itself yields "" and
                                // is skipped (the dropdown filters empties too).
                                let display_name = budget_display_name(suffix);
                                if !display_name.is_empty() {
                                    tracing::debug!("Extracted category: {}", display_name);
                                    categories_vec.push(Category {
                                        display_name,
                                        full_path: account.clone(),
                                        category_type: "expense".to_string(),
                                    });
                                }
                            }
                            matched = true;
                            break;
                        }
                    }
                    
                    // Also check liability locations
                    if !matched {
                        for prefix in &self.liability_locations {
                            let prefix_trimmed = prefix.trim_end_matches(':');
                            if account.starts_with(prefix_trimmed) {
                                tracing::debug!("Matched liability prefix '{}' for account: {}", prefix_trimmed, account);
                                // Get the part after the prefix
                                if let Some(suffix) = account.strip_prefix(prefix_trimmed) {
                                    // For liabilities, the account itself is the category
                                    // If suffix is empty, use the full account name
                                    let segment = if suffix.is_empty() || suffix == ":" {
                                        // Extract last segment from full account path
                                        account.split(':')
                                            .filter(|s| !s.is_empty())
                                            .last()
                                            .map(|s| s.to_string())
                                            .unwrap_or_default()
                                    } else {
                                        // Get the last non-empty segment of the suffix
                                        suffix
                                            .split(':')
                                            .filter(|s| !s.is_empty())
                                            .last()
                                            .map(|s| s.to_string())
                                            .unwrap_or_default()
                                    };
                                    
                                    if !segment.is_empty() {
                                        tracing::debug!("Extracted liability: {}", segment);
                                        categories_vec.push(Category {
                                            display_name: segment,
                                            full_path: account.clone(),
                                            category_type: "liability".to_string(),
                                        });
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
                
                tracing::debug!("Found {} categories", categories_vec.len());
                categories_vec.sort_by(|a, b| a.display_name.cmp(&b.display_name));  
                categories_vec
            }
            Err(e) => {
                // Log error and return empty list
                tracing::error!("Failed to get accounts from hledger: {}", e);
                // Also log stderr if available
                tracing::debug!("Error details: {:?}", e);
                Vec::new()
            }
        }
    }

    /// Get budget categories from hledger
    pub fn get_categories(&self) -> Vec<Category> {
        self.get_categories_from_hledger()
    }

    /// Query hledger for expense accounts
    pub fn get_expenses_from_hledger(&self) -> Vec<Category> {
        use crate::hledger::Hledger;
        let journal_path = format!("{}/{}", self.nas_path, self.top_journal);
        let hledger = Hledger::new();
        match hledger.accounts(Some(&journal_path)) {
            Ok(accounts) => {
                let mut categories = Vec::new();
                for account in accounts {
                    for prefix in &self.expense_locations {
                        let prefix_trimmed = prefix.trim_end_matches(':');
                        if account.starts_with(prefix_trimmed) {
                            let mut display_name = account.strip_prefix(prefix_trimmed).unwrap_or(&account).to_string();
                            // Remove leading colon if present
                            if display_name.starts_with(':') {
                                display_name = display_name[1..].to_string();
                            }
                            categories.push(Category {
                                full_path: account.clone(),
                                display_name,
                                category_type: "expense".to_string(),
                            });
                            break;
                        }
                    }
                }
                categories.sort_by(|a, b| a.display_name.cmp(&b.display_name));  
                categories
            }
            Err(_) => Vec::new(),
        }
    }
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file '{0}': {1}")]
    ReadFailed(String, std::io::Error),
    #[error("Failed to parse config file '{0}': {1}")]
    ParseFailed(String, serde_yaml::Error),
    #[error("transfer_journal must be defined in config.yaml")]
    TransferJournalMissing,
    #[error("Failed to create transfer journal file '{0}': {1}")]
    TransferJournalCreateFailed(String, std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_display_name() {
        assert_eq!(super::budget_display_name(":Budget:Vehicle:Fuel"), "Vehicle:Fuel");
        assert_eq!(super::budget_display_name(":Budget:Home:Internet"), "Home:Internet");
        assert_eq!(super::budget_display_name(":Budget:Home:Maintenance"), "Home:Maintenance");
        assert_eq!(super::budget_display_name(":Budget:Budget"), "Budget");
        assert_eq!(super::budget_display_name(":Budget"), "");
        assert_eq!(super::budget_display_name(":House Reno"), "House Reno");
        assert_eq!(super::budget_display_name(":RandomBudget"), "RandomBudget");
        assert_eq!(super::budget_display_name(":Budgeting"), "Budgeting");
    }

    #[test]
    fn test_config_loads_with_budget_entries() {
        let config = Config::load("config.example.yaml").expect("Failed to load config");
        // Verify array-only budget entries are loaded (migration complete)
        assert!(!config.budget_entries.is_empty());  
    }

    #[test]
    fn test_get_all_budget_entries_merges() {
        let config = Config::load("config.example.yaml").expect("Failed to load config");
        let entries = config.get_all_budget_entries();
        assert!(!entries.is_empty());
        for entry in &entries {
            assert!(!entry.account.is_empty());
            assert!(!entry.label.is_empty());
            assert!(!entry.header.is_empty());
            // Allows zero/negative amounts for unallocated or liability entries
        }
    }

    #[test]
    fn test_entries_sorted_by_header_then_label() {
        let config = Config::load("config.example.yaml").expect("Failed to load config");
        let entries = config.get_all_budget_entries();
        for i in 0..entries.len().saturating_sub(1) {
            assert!(entries[i].header <= entries[i + 1].header);  
            if entries[i].header == entries[i + 1].header {
                assert!(entries[i].label <= entries[i + 1].label);
            }
        }
    }

    #[test]
    fn test_budget_entry_struct_accessible() {
        let config = Config::load("config.example.yaml").expect("Failed to load config");
        let entries = config.get_all_budget_entries();
        assert!(entries.len() > 0);  
    }
}

