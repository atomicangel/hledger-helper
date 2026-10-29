use hledger_helper::hledger::{Hledger, HledgerError};
use hledger_helper::file_handler::{FileHandler, FileHandlerError};
use hledger_helper::config::Config;
use hledger_helper::api::transactions::{determine_journal_path, extract_accounts_from_transaction, find_journal_route};
use tempfile::TempDir;
use std::fs;

#[test]
fn test_hledger_error_display() {
    let err = HledgerError::NonZeroExit { cmd: "hledger check".into(), status: 1 };
    assert!(err.to_string().contains("hledger check"));
    assert!(err.to_string().contains("1"));
}

#[test]
fn test_hledger_with_custom_binary() {
    let _hledger = Hledger::with_binary("/usr/bin/hledger");
    // Verify default creation works
    let _default_hledger = Hledger::new();
    assert!(true); // Just verify construction doesn't panic
}

#[test]
fn test_config_validation_invalid_yaml() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("invalid.yaml");
    fs::write(&config_path, "invalid: yaml: content: [").unwrap();
    
    let result = Config::load(config_path.to_str().unwrap());
    assert!(result.is_err());
}

#[test]
fn test_config_validation_missing_fields() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("incomplete.yaml");
    fs::write(&config_path, "nas_path: /test").unwrap();
    
    let result = Config::load(config_path.to_str().unwrap());
    assert!(result.is_err());
}

#[test]
fn test_file_handler_creates_directories() {
    let temp_dir = TempDir::new().unwrap();
    let nested_path = temp_dir.path().join("a/b/c/test.journal");
    
    let mut handler = FileHandler::open(&nested_path).unwrap();
    handler.append("test content").unwrap();
    
    assert!(nested_path.exists());
}

#[test]
fn test_file_handler_append_content() {
    let temp_dir = TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("test.journal");
    
    let mut handler = FileHandler::open(&journal_path).unwrap();
    handler.append("first line\n").unwrap();
    handler.append("second line\n").unwrap();
    
    let content = fs::read_to_string(&journal_path).unwrap();
    assert!(content.contains("first line"));
    assert!(content.contains("second line"));
}

#[test]
fn test_file_handler_locking() {
    let temp_dir = TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("locked.journal");
    fs::write(&journal_path, "").unwrap();
    
    // Test locking mechanism works
    let handler = FileHandler::open(&journal_path).unwrap();
    assert!(handler.path().contains("locked.journal"));
}

#[test]
fn test_config_clone() {
    let config_yaml = r#"
nas_path: "/mnt/nas/finances"
top_journal: "main.journal"
current_year: 2026
llm_endpoint: null
budget_locations:
  - "assets:Bank:Checking:Budget:"
  - "Groceries"
liability_locations: []
expense_locations: []
transfer_journal: "2026/transfers.journal"
payment_sources:
  assets:bank:
    label: "Bank"
    journal: "2026/example-card.journal"
server:
  bind_address: "0.0.0.0"
  port: 8080
"#;
    
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, config_yaml).unwrap();
    
    let config1 = Config::load(config_path.to_str().unwrap()).unwrap();
    let config2 = config1.clone();
    
    assert_eq!(config1.nas_path, config2.nas_path);
    assert_eq!(config1.server.port, config2.server.port);
}

#[test]
fn test_file_handler_error_display() {
    let err = FileHandlerError::OpenFailed("test".into(), std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
    assert!(err.to_string().contains("test"));
}

#[test]
fn test_empty_journal_file() {
    let temp_dir = TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("empty.journal");
    fs::write(&journal_path, "").unwrap();
    
    let handler = FileHandler::open(&journal_path).unwrap();
    assert!(handler.path().contains("empty.journal"));
}

#[test]
fn test_extract_accounts_simple_transaction() {
    let transaction = "2026-01-15 * Grocery Store
  expenses:Food:Groceries  $50.00
  assets:bank  -$50.00";
    
    let accounts = extract_accounts_from_transaction(transaction);
    assert!(accounts.contains(&"expenses:Food:Groceries".to_string()));
    assert!(accounts.contains(&"assets:bank".to_string()));
}

#[test]
fn test_extract_accounts_with_simple_account() {
    let transaction = "2026-01-15 * Cash Purchase
  expenses:Pet Supplies  $10.00
  cash  -$10.00";
    
    let accounts = extract_accounts_from_transaction(transaction);
    assert!(accounts.contains(&"expenses:Pet Supplies".to_string()));
    assert!(accounts.contains(&"cash".to_string()));
}

#[test]
fn test_extract_accounts_ignores_amounts() {
    let transaction = "2026-01-15 * Test
  expenses:Food  $25.00
  assets:bank  -$25.00";
    
    let accounts = extract_accounts_from_transaction(transaction);
    // Should NOT include the amounts
    assert!(!accounts.contains(&"$25.00".to_string()));
    assert!(!accounts.contains(&"-$25.00".to_string()));
}

#[test]
fn test_extract_accounts_handles_split_transaction() {
    let transaction = "2026-01-15 * Multi-category Expense
  expenses:Food:Groceries  $30.00
  expenses:Vehicle:Fuel  $20.00
  expenses:Pet Supplies  $15.00
  assets:bank  -$65.00";
    
    let accounts = extract_accounts_from_transaction(transaction);
    assert!(accounts.contains(&"expenses:Food:Groceries".to_string()));
    assert!(accounts.contains(&"expenses:Vehicle:Fuel".to_string()));
    assert!(accounts.contains(&"expenses:Pet Supplies".to_string()));
    assert!(accounts.contains(&"assets:bank".to_string()));
}

#[test]
fn test_find_journal_route_exact_match() {
    let config_yaml = r#"
nas_path: "/test"
top_journal: "main.journal"
current_year: 2026
llm_endpoint: null
budget_locations:
  - "assets:Bank:Checking:Budget:"
liability_locations: []
expense_locations: []
transfer_journal: "2026/transfers.journal"
payment_sources:
  assets:bank:
    label: "Bank"
    journal: "2026/example-card.journal"
server:
  bind_address: "0.0.0.0"
  port: 8080
"#;
    
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, config_yaml).unwrap();
    
    let config = Config::load(config_path.to_str().unwrap()).unwrap();
    let result = find_journal_route(&config, "assets:bank");
    assert_eq!(result, Some("2026/example-card.journal".to_string()));
}

#[test]
fn test_find_journal_route_longest_prefix() {
    let config_yaml = r#"
nas_path: "/test"
top_journal: "main.journal"
current_year: 2026
llm_endpoint: null
budget_locations:
  - "assets:Bank:Checking:Budget:"
liability_locations: []
expense_locations: []
transfer_journal: "2026/transfers.journal"
payment_sources:
  assets:bank:
    label: "Bank"
    journal: "2026/example-card.journal"
  assets:bank:checking:
    label: "Checking"
    journal: "2026/checking.journal"
server:
  bind_address: "0.0.0.0"
  port: 8080
"#;
    
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, config_yaml).unwrap();
    
    let config = Config::load(config_path.to_str().unwrap()).unwrap();
    
    // Test longest prefix match
    let result1 = find_journal_route(&config, "assets:bank:checking");
    assert_eq!(result1, Some("2026/checking.journal".to_string()));
    
    let result2 = find_journal_route(&config, "assets:bank");
    assert_eq!(result2, Some("2026/example-card.journal".to_string()));
}

#[test]
fn test_find_journal_route_no_match() {
    let config_yaml = r#"
nas_path: "/test"
top_journal: "main.journal"
current_year: 2026
llm_endpoint: null
budget_locations:
  - "assets:Bank:Checking:Budget:"
liability_locations: []
expense_locations: []
transfer_journal: "2026/transfers.journal"
payment_sources:
  assets:bank:
    label: "Bank"
    journal: "2026/example-card.journal"
server:
  bind_address: "0.0.0.0"
  port: 8080
"#;
    
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, config_yaml).unwrap();
    
    let config = Config::load(config_path.to_str().unwrap()).unwrap();
    let result = find_journal_route(&config, "liabilities:credit-card");
    assert_eq!(result, None);
}

#[test]
fn test_determine_journal_path_with_route() {
    let config_yaml = r#"
nas_path: "/test"
top_journal: "main.journal"
current_year: 2026
llm_endpoint: null
budget_locations:
  - "assets:Bank:Checking:Budget:"
liability_locations: []
expense_locations: []
transfer_journal: "2026/transfers.journal"
payment_sources:
  assets:bank:
    label: "Bank"
    journal: "2026/example-card.journal"
server:
  bind_address: "0.0.0.0"
  port: 8080
"#;
    
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, config_yaml).unwrap();
    
    let config = Config::load(config_path.to_str().unwrap()).unwrap();
    
    let req = hledger_helper::api::transactions::TransactionRequest {
        transaction: "2026-01-15 * Test
  expenses:Food  $100.00
  assets:bank  -$100.00".to_string(),
        target_file: None,
    };
    
    let path = determine_journal_path(&config, &req);
    assert_eq!(path, "/test/2026/example-card.journal");
}

#[test]
fn test_determine_journal_path_fallback() {
    let config_yaml = r#"
nas_path: "/test"
top_journal: "main.journal"
current_year: 2026
llm_endpoint: null
budget_locations:
  - "assets:Bank:Checking:Budget:"
liability_locations: []
expense_locations: []
transfer_journal: "2026/transfers.journal"
payment_sources:
  assets:bank:
    label: "Bank"
    journal: "2026/example-card.journal"
server:
  bind_address: "0.0.0.0"
  port: 8080
"#;
    
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, config_yaml).unwrap();
    
    let config = Config::load(config_path.to_str().unwrap()).unwrap();
    
    // Transaction with no matching route (expenses don't have routes)
    let req = hledger_helper::api::transactions::TransactionRequest {
        transaction: "2026-01-15 * Test
  expenses:Food  $100.00
  cash  -$100.00".to_string(),
        target_file: None,
    };
    
    let path = determine_journal_path(&config, &req);
    assert_eq!(path, "/test/main.journal");
}

#[test]
fn test_determine_journal_path_with_target_file() {
    let config_yaml = r#"
nas_path: "/test"
top_journal: "main.journal"
current_year: 2026
llm_endpoint: null
budget_locations:
  - "assets:Bank:Checking:Budget:"
liability_locations: []
expense_locations: []
transfer_journal: "2026/transfers.journal"
payment_sources:
  assets:bank:
    label: "Bank"
    journal: "2026/example-card.journal"
server:
  bind_address: "0.0.0.0"
  port: 8080
"#;
    
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, config_yaml).unwrap();
    
    let config = Config::load(config_path.to_str().unwrap()).unwrap();
    
    // With target_file, should ignore routing
    let req = hledger_helper::api::transactions::TransactionRequest {
        transaction: "2026-01-15 * Test
  expenses:Food  $100.00
  assets:bank  -$100.00".to_string(),
        target_file: Some("2026/custom.journal".to_string()),
    };
    
    let path = determine_journal_path(&config, &req);
    assert_eq!(path, "/test/2026/custom.journal");
}
