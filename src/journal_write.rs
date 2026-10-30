//! Journal writing with staging validation, hledger check gating (D2), and column 67 alignment  
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::file_handler::FileHandler;
use crate::hledger::{Hledger, HledgerError};
use crate::sanitize::sanitize_journal_text;

/// Error when writing a validated journal.
#[derive(Debug)]
#[allow(dead_code)]
pub enum JournalWriteError {
    ExistingJournalInvalid(String),
    CheckFailed(String),
    BreaksJournal(String),
    FormatError(String),
    File(String),
}

impl std::fmt::Display for JournalWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let msg = match self {
            Self::ExistingJournalInvalid(m) => format!("Existing journal invalid:\n{}", m),
            Self::CheckFailed(m) => format!("Staged content check failed:\n{}", m),
            Self::BreaksJournal(m) => format!("New transaction breaks journal:\n{}", m),
            Self::FormatError(m) => format!("Column 67 formatting error:\n{}", m),
            Self::File(m) => format!("Journal file I/O error:\n{}", m),
        };  
        writeln!(f, "{}", msg).map_err(|_| std::fmt::Error)
    }
}

/// Staging guard for atomic writes - cleans up staged file on drop if validation fails.  
pub struct StagingFileGuard(String);

impl Drop for StagingFileGuard {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}

/// Constant for column alignment in formatted output (FIXES.md requirement).
/// Column 67 aligns posting content in monospace terminal/ledger output.
pub const COLUMN_START: u8 = 67;

/// Format transaction with column 67 alignment (D1).
/// The first line (date + payee) is sanitized to ASCII-only so the journal
/// parses under any system locale; posting lines are realigned, all other
/// lines pass through unchanged.
pub fn format_transaction(transaction: &str) -> String {
    let mut lines = Vec::new();
    let mut first_line = true;
    
    for line in transaction.lines() {
        if first_line {
            // Sanitize the first line (date and payee) to ASCII-only so the
            // journal parses under any system locale. The payee is free text
            // (phone autocorrect can insert curly quotes); date and status
            // are ASCII and pass through unchanged. Posting lines below are
            // left exact: account names are structured identifiers.
            lines.push(sanitize_journal_text(line));
            first_line = false;
        } else if line.starts_with(' ') || line.starts_with('\t') {
            // Format posting lines
            let trimmed = line.trim_start();
            if let Some((account, amount)) = parse_posting_line(trimmed) {
                let formatted = format_posting_line(&account, &amount);
                lines.push(formatted);
            } else {
                lines.push(line.to_string());
            }
        } else {
            lines.push(line.to_string());
        }
    }
    
    lines.join("\n")
}

/// Parse a posting line into account and amount parts
fn parse_posting_line(line: &str) -> Option<(String, String)> {
    // Find the amount - look for $ symbol which marks the start of amount
    let dollar_pos = line.find('$')?;
    
    // Find the last whitespace before $
    let amount_start = line[..dollar_pos].rfind(|c: char| c.is_whitespace())
        .map_or(0, |pos| pos + 1);
    
    // Check if there's a - immediately before the amount
    let before_dollar = &line[..dollar_pos];
    let amount_start_with_sign = if before_dollar.trim_end().ends_with('-') {
        // The - is part of the amount, find the - position
        before_dollar.rfind('-').map_or(amount_start, |pos| pos)
    } else {
        amount_start
    };
    
    let account = line[..amount_start_with_sign].trim().to_string();
    let amount = line[amount_start_with_sign..].trim().to_string();
    
    if account.is_empty() {
        return None;
    }
    
    Some((account, amount))
}

/// Format a posting line with amounts starting at column 67 (1-indexed)
fn format_posting_line(account: &str, amount: &str) -> String {
    // The line starts with 2 spaces for indentation
    let indent = "  ";
    let account_len = account.len();
    
    // Align start of amount field to column 67 (1-indexed)
    // Position 0-indexed = COLUMN_START - 1
    let target_pos = COLUMN_START as usize - 1;
    let padding_needed = target_pos.saturating_sub(indent.len() + account_len);
    
    if padding_needed > 0 {
        format!("{}{}{}{}", indent, account, " ".repeat(padding_needed), amount)
    } else {
        // Account too long, just use default spacing
        format!("{} {}  {}", indent, account, amount)
    }
}

/// Monotonic suffix for staging files, so concurrent requests never share one.
static STAGE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// True if the string holds a control character that must never reach a
/// journal line. Tabs are posting indentation and newlines are the line
/// separator, so each is allowed only where the caller says so; everything
/// else control (including CR, which would smuggle line breaks past
/// line-based parsing) is forbidden.
fn has_forbidden_control(s: &str, allow_newline: bool) -> bool {
    s.chars()
        .any(|c| c.is_control() && c != '\t' && !(allow_newline && c == '\n'))
}

/// Validate a payee (free text on a transaction header line). A newline
/// would split the header and inject forged lines, so newlines and all
/// other control characters (except tab) are rejected. Anything else on
/// one line is the payee's business; the staged hledger check gates it.
pub fn validate_payee(payee: &str) -> Result<(), String> {
    if has_forbidden_control(payee, false) {
        return Err("payee must not contain newlines or control characters".to_string());
    }
    Ok(())
}

/// Validate an account name for a posting line. Newlines/controls would
/// split or smuggle lines; `;` starts an hledger comment (swallowing the
/// amount that follows it); `$` breaks amount parsing. None of these can
/// appear in a real account name, so all are rejected.
pub fn validate_account_name(account: &str) -> Result<(), String> {
    if has_forbidden_control(account, false) {
        return Err("account must not contain newlines or control characters".to_string());
    }
    if account.contains(';') {
        return Err("account must not contain ';'".to_string());
    }
    if account.contains('$') {
        return Err("account must not contain '$'".to_string());
    }
    Ok(())
}

/// A header line starts with a YYYY-MM-DD date (shape-checked only;
/// hledger itself validates that the date is real).
fn is_date_header(line: &str) -> bool {
    let b = line.as_bytes();
    b.len() >= 10
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4] == b'-'
        && b[5].is_ascii_digit()
        && b[6].is_ascii_digit()
        && b[7] == b'-'
        && b[8].is_ascii_digit()
        && b[9].is_ascii_digit()
}

/// Validate the shape of a raw transaction string: exactly one
/// transaction — a date header line, then only whitespace-led posting
/// lines. Anything else (a second header, a directive such as `include`,
/// a journal-level comment, a blank line) is rejected, so a submission
/// cannot smuggle extra transactions or directives into the journal.
pub fn validate_transaction_text(transaction: &str) -> Result<(), String> {
    let trimmed = transaction.trim_end();
    if trimmed.is_empty() {
        return Err("transaction must not be empty".to_string());
    }
    if has_forbidden_control(trimmed, true) {
        return Err("transaction must not contain carriage returns or control characters".to_string());
    }
    let mut lines = trimmed.lines();
    let header = lines.next().unwrap(); // trimmed is non-empty, so a first line exists
    if !is_date_header(header) {
        return Err("transaction must start with a date (YYYY-MM-DD)".to_string());
    }
    for line in lines {
        if line.trim().is_empty() {
            return Err("transaction must not contain blank lines".to_string());
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            return Err(
                "transaction must contain only one header line followed by posting lines"
                    .to_string(),
            );
        }
    }
    Ok(())
}

/// Scrub a staging-file path out of hledger output before it reaches a
/// client: the staged path embeds the real journal path, which must
/// never leak in an error message.
fn scrub_stage_path(text: &str, stage_path: &str) -> String {
    text.replace(&format!("./{}", stage_path), "staged journal")
        .replace(stage_path, "staged journal")
}

/// Append a transaction to a journal, gated by a staged hledger check.
/// The staged file holds the existing journal plus the new transaction,
/// exactly as the real file will look after the append; only if
/// `hledger check` passes on the staged file is the content appended to
/// the real journal. On a staged failure the existing journal is checked
/// alone, to tell "already broken" apart from "this breaks it". The
/// staged files are deleted on every path out (StagingFileGuard).
pub fn append_validated(journal_path: &str, content: &str) -> Result<(), JournalWriteError> {
    validate_transaction_text(content).map_err(JournalWriteError::FormatError)?;

    if let Some(parent) = std::path::Path::new(journal_path).parent() {
        fs::create_dir_all(parent).map_err(|e| {
            JournalWriteError::File(format!(
                "cannot create directory for {}: {}",
                journal_path, e
            ))
        })?;
    }

    let formatted = format_transaction(content);
    let existing = match fs::read_to_string(journal_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(JournalWriteError::File(format!(
                "cannot read journal {}: {}",
                journal_path, e
            )))
        }
    };
    let to_append = if existing.is_empty() || existing.ends_with('\n') {
        format!("{}\n\n", formatted)
    } else {
        format!("\n{}\n\n", formatted)
    };
    let staged_content = format!("{}{}", existing, to_append);

    let n = STAGE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let stage_path = format!(
        "{}.check.{}-{}.tmp",
        journal_path,
        std::process::id(),
        n
    );
    let _guard = StagingFileGuard(stage_path.clone());
    fs::write(&stage_path, &staged_content).map_err(|e| {
        JournalWriteError::File(format!(
            "cannot write staging file {}: {}",
            stage_path, e
        ))
    })?;

    let hledger = Hledger::new();
    if let Err(e) = hledger.check(&stage_path) {
        // Diagnose: is the existing journal already broken?
        let existing_stage = format!(
            "{}.existing-check.{}-{}.tmp",
            journal_path,
            std::process::id(),
            n
        );
        let _existing_guard = StagingFileGuard(existing_stage.clone());
        let existing_broken = fs::write(&existing_stage, &existing)
            .ok()
            .and_then(|_| hledger.check(&existing_stage).err())
            .is_some();
        if existing_broken {
            return Err(JournalWriteError::ExistingJournalInvalid(
                "existing journal already fails hledger check; refusing to append".to_string(),
            ));
        }
        // Report hledger's own diagnostic, with the staging path
        // scrubbed out; the real journal path never reaches the client.
        let detail = match &e {
            HledgerError::NonZeroExitWithStderr { stderr, .. } => {
                scrub_stage_path(stderr, &stage_path)
            }
            _ => scrub_stage_path(&e.to_string(), &stage_path),
        };
        return Err(JournalWriteError::BreaksJournal(detail));
    }

    let mut file = FileHandler::open(journal_path)
        .map_err(|e| JournalWriteError::File(e.to_string()))?;
    file.append(&to_append)
        .map_err(|e| JournalWriteError::File(e.to_string()))?;
    Ok(())
}

#[cfg(test)]  
mod tests {  
    use super::*;
    
    #[test]  
    fn format_basic() {  
        let out = format_transaction("2024-01-01 * test");
        assert!(out.contains("test"));
    } 
    
    #[test]
    fn append_validated_appends_balanced() {
        let dir = tempfile::tempdir().unwrap();
        let journal = dir.path().join("test.journal");
        let path = journal.to_str().unwrap();
        append_validated(
            path,
            "2024-10-30 * purchase\n  assets:Cash   $50.00\n  expenses:Food   -$50.00",
        )
        .unwrap();
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("purchase"));
        assert!(content.contains("assets:Cash"));
    }

    #[test]
    fn append_validated_rejects_unbalanced() {
        let dir = tempfile::tempdir().unwrap();
        let journal = dir.path().join("test.journal");
        let path = journal.to_str().unwrap();
        let err = append_validated(path, "2024-10-30 * purchase\n  assets:Cash   $50.00")
            .unwrap_err();
        assert!(matches!(err, JournalWriteError::BreaksJournal(_)));
        assert!(fs::read_to_string(path).map_or(true, |c| c.is_empty()));
    }

    #[test]
    fn append_validated_rejects_injection() {
        let dir = tempfile::tempdir().unwrap();
        let journal = dir.path().join("test.journal");
        let path = journal.to_str().unwrap();
        let err = append_validated(
            path,
            "2024-10-30 * purchase\n  assets:Cash   $50.00\n  expenses:Food   -$50.00\n2024-10-31 * forged\n  assets:Cash   $999.00\n  expenses:Food   -$999.00",
        )
        .unwrap_err();
        assert!(matches!(err, JournalWriteError::FormatError(_)));
    }

    #[test]
    fn field_validators_reject_smuggling() {
        assert!(validate_payee("dinner\n  assets:Cash   $999.00").is_err());
        assert!(validate_payee("McDonald's").is_ok());
        assert!(validate_account_name("assets:Cash;comment").is_err());
        assert!(validate_account_name("assets:Ca$h").is_err());
        assert!(validate_account_name("expenses:Pet Supplies").is_ok());
        assert!(validate_transaction_text(
            "2024-10-30 * ok\n  assets:Cash   $50.00\n  expenses:Food   -$50.00"
        )
        .is_ok());
        assert!(validate_transaction_text("2024-10-30 * ok\ninclude other.journal").is_err());
    }
}
