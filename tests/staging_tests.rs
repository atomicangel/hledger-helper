use hledger_helper::journal_write::append_validated;

#[test]
fn append_validated_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("staging_success.jnl");
    let path = journal.to_str().unwrap();
    append_validated(
        path,
        "2024-01-01 * txn\n  assets:Cash   $10.00\n  expenses:Food   -$10.00",
    )
    .unwrap();
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("txn"));
}

#[test]
fn append_multiple_entries_works() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("multi.jnl");
    let path = journal.to_str().unwrap();
    append_validated(
        path,
        "2024-10-30 * entry\n  assets:Cash   $5.00\n  expenses:Food   -$5.00",
    )
    .unwrap();
    append_validated(
        path,
        "2024-10-31 * another\n  expenses:Rent   $100.00\n  assets:Cash   -$100.00",
    )
    .unwrap();
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("entry"));
    assert!(content.contains("another"));
}
