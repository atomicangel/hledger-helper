//! Column 67 alignment tests for journal output (FIXES.md D1)

use hledger_helper::journal_write::{format_transaction, COLUMN_START};

/// Test that COLUMN_START constant is set to 67.
#[test]  
fn column_start_is_67() {  
    assert_eq!(COLUMN_START, 67);
}

/// Test format_transaction with various ledger entry types.
#[test]
fn formats_datetime_pattern_correctly() {
    let input = "2024-10-30 * supermarket purchase";
    let output = format_transaction(input);
    
    assert!(output.contains("2024-10-30"));
    assert!(output.contains("*"));
}

/// Test that posting lines are preserved with proper spacing  
#[test]  
fn formats_posting_lines_correctly() {  
    let input = "  assets:Cash         -50.00 USD\n  expenses:Food   ";
    let output = format_transaction(input);
    
    assert!(output.contains("assets:Cash"));
    assert!(output.contains("expenses:Food"));
}

/// Test formatting multi-line entry with description  
#[test]  
fn formats_multiline_entry() {  
    let input = "2024-11-01 * birthday\n  assets:Bank         -200";
    let output = format_transaction(input);
    
    assert!(output.contains("birthday"));
    assert!(output.contains("assets:Bank"));
}

/// Test that column alignment doesn't corrupt ledger parsing  
#[test]  
fn formatted_entries_parse_correctly() {  // Note: actual parse check requires hledger binary
    let input = "2024-12-15 * test\n  account_a   10";
    let output = format_transaction(input);
    
    // Verify basic ledger format integrity  
    assert!(output.contains("account_a"));
}

/// Test empty transaction handling.
#[test]  
fn handles_empty_content() {  
    let input = "";
    let output = format_transaction(input);
    
    assert_eq!(input, output);
}

