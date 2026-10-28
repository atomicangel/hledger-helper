//! Character sanitization tests (FIXES.md A1-A3)

use hledger_helper::sanitize::sanitize_journal_text;

#[test]  
fn preserves_ascii_unchanged() {  
    assert_eq!(sanitize_journal_text("hello world"), "hello world");
}

#[test]  
fn converts_right_single_quote() {  
    let input = "McDonald\u{2019}s";
    assert_eq!(sanitize_journal_text(input), "McDonald's");
} 

