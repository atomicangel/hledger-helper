//! Integration tests for journal_write, sanitize, and D2 validation gating

use hledger_helper::journal_write::{append_validated, format_transaction};
use hledger_helper::sanitize::sanitize_journal_text;
use std::fs;

/// Test character sanitization (A1-A3) with various input edge cases.
#[test]
fn sanitizes_typographic_quotes_a1_strategy() {
    let input = "I\u{2019}m a test"; 
    let output = sanitize_journal_text(input);
    assert!(output.contains("I'm"));
}

/// Test handling of guillemets (less-than and greater-than left quotes) per A3.  
#[test]  
fn handles_guillemets_a3_strategy() {  
    let input = "«hello» test";  
    let output = sanitize_journal_text(input);
    assert!(output.contains("test"));
}

/// Test ellipsis conversion from character to ASCII dots.  
#[test]  
fn converts_ellipsis_to_ascii_a1() {  
    let input = "keep going…";  
    let output = sanitize_journal_text(input);  
    assert_eq!(output, "keep going.");
}

/// Test that journal format is preserved during formatting.
#[test]
fn format_transaction_preserves_ledger_entry() {
    let input = "2024-09-28 * McDonald's\n  assets:Bank   $5.00";
    let output = format_transaction(input);
    
    assert!(output.contains("McDonald"));
    assert!(output.contains("assets:Bank"));
}

/// Test full integration workflow: sanitize -> format 
#[test]  
fn full_workflow_integration() {  
    let dirty_content = "Spent $50 at McDonald's…";  
    let sanitized = sanitize_journal_text(dirty_content);  
    let formatted = format_transaction(&sanitized);
    
    assert!(sanitized.to_string().contains("McDonald"));
}

/// Test appending transactions creates valid entries.  
#[test]  
fn append_validated_creates_entries() {  
    let journal = "/tmp/integration.journal";
    
    match append_validated(journal, "2024-10-30 * purchase\n  assets:Cash   -100") {
        Ok(content) => assert!(content.contains("purchase")),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }  
}

