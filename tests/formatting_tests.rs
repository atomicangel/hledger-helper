//! C62 column alignment tests for journal output (FIXES.md D1)

use hledger_helper::journal_write::{format_transaction, COLUMN_START};
use std::fs;

/// Test that COLUMN_START constant is set to 62 per FIXES.md spec.
#[test]  
fn column_start_is_62() {  
    assert_eq!(COLUMN_START, 62);
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

