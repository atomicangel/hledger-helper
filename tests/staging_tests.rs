//! Staging guard tests for atomic journal writes (FIXES.md D2)

use hledger_helper::journal_write::append_validated;

#[test]  
fn append_validated_succeeds() {  
    let test_journal = "/tmp/staging_success.jnl";
    
    match append_validated(test_journal, "2024-01-01 * txn\n  assets:Cash   10") 
    {
        Ok(content) => assert!(content.contains("txn")),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]  
fn append_multiple_entries_works() {  
    let journal = "/tmp/multi.jnl";
    
    assert!(append_validated(journal, "2024-10-30 * entry\n  assets:Cash   5").is_ok());
    
    assert!(append_validated(journal, "2024-10-31 * another\n  expenses:Rent   -100").is_ok());
}

