use std::process::Command;
use thiserror::Error;

/// Error types for hledger operations
#[derive(Debug, Error)]
pub enum HledgerError {
    #[error("Failed to execute hledger command '{cmd}': {source}")]
    CommandFailed {
        cmd: String,
        source: std::io::Error,
    },
    #[error("Hledger command '{cmd}' failed with exit status: {status}")]
    NonZeroExit {
        cmd: String,
        status: i32,
    },
    #[error("Hledger command '{cmd}' failed with exit status {status}: {stderr}")]
    NonZeroExitWithStderr {
        cmd: String,
        status: i32,
        stderr: String,
    },
}

/// Result type for hledger operations
pub type HledgerResult<T> = Result<T, HledgerError>;

/// Wrapper around hledger CLI commands
pub struct Hledger {
    binary: String,
}

impl Hledger {
    /// Create a new Hledger wrapper
    pub fn new() -> Self {
        Self {
            binary: "hledger".to_string(),
        }
    }

    /// Create a new Hledger wrapper with a custom binary path
    #[allow(dead_code)] // used by integration tests via the lib crate
    pub fn with_binary<P: Into<String>>(binary: P) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    /// Run a hledger command and return stdout
    fn run(&self, args: &[&str]) -> HledgerResult<String> {
        let output = Command::new(&self.binary)
            .args(args)
            .output()
            .map_err(|e| HledgerError::CommandFailed {
                cmd: format!("{} {}", self.binary, args.join(" ")),
                source: e,
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(HledgerError::NonZeroExitWithStderr {
                cmd: format!("{} {}", self.binary, args.join(" ")),
                status: output.status.code().unwrap_or(-1),
                stderr: stderr.to_string(),
            })
        }
    }

    /// Get list of accounts
    pub fn accounts(&self, journal_path: Option<&str>) -> HledgerResult<Vec<String>> {
        let args = vec!["accounts"];
        if let Some(jp) = journal_path {
            let output = Command::new(&self.binary)
                .env("LEDGER_FILE", jp)
                .args(&args)
                .output()
                .map_err(|e| HledgerError::CommandFailed {
                    cmd: format!("{} accounts (LEDGER_FILE={})", self.binary, jp),
                    source: e,
                })?;

            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).trim().lines().map(|s| s.to_string()).collect())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(HledgerError::NonZeroExitWithStderr {
                    cmd: format!("{} accounts (LEDGER_FILE={})", self.binary, jp),
                    status: output.status.code().unwrap_or(-1),
                    stderr: stderr.to_string(),
                })
            }
        } else {
            let output = self.run(&args)?;
            Ok(output.lines().map(|s| s.to_string()).collect())
        }
    }

    /// Get list of payees
    pub fn payees(&self) -> HledgerResult<Vec<String>> {
        let output = self.run(&["payees"])?;
        Ok(output.lines().map(|s| s.to_string()).collect())
    }

    /// Get balance for a specific account
    pub fn balance(&self, account: &str, period: Option<&str>, journal_path: Option<&str>) -> HledgerResult<String> {
        let mut args = vec!["balance", account];
        if let Some(p) = period {
            args.push("-p");
            args.push(p);
        }
        let mut cmd = Command::new(&self.binary);
        cmd.args(&args);
        if let Some(jp) = journal_path {
            cmd.env("LEDGER_FILE", jp);
        }
        let output = cmd.output()
            .map_err(|e| HledgerError::CommandFailed {
                cmd: format!("{} {}", self.binary, args.join(" ")),
                source: e,
            })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(HledgerError::NonZeroExitWithStderr {
                cmd: format!("{} {}", self.binary, args.join(" ")),
                status: output.status.code().unwrap_or(-1),
                stderr: stderr.to_string(),
            })
        }
    }

    /// Get balances for multiple accounts in a SINGLE hledger invocation.
    /// Uses --flat for machine-parseable one-line-per-account output.
    /// Non-existent accounts are simply omitted from the output (no error).
    pub fn balances_flat(&self, accounts: &[String], journal_path: Option<&str>) -> HledgerResult<String> {
        let mut args: Vec<&str> = vec!["balance", "--flat"];
        let acct_refs: Vec<&str> = accounts.iter().map(|s| s.as_str()).collect();
        args.extend(acct_refs);
        let mut cmd = Command::new(&self.binary);
        cmd.args(&args);
        if let Some(jp) = journal_path {
            cmd.env("LEDGER_FILE", jp);
        }
        let output = cmd.output()
            .map_err(|e| HledgerError::CommandFailed {
                cmd: format!("{} balance --flat ({} accounts)", self.binary, accounts.len()),
                source: e,
            })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(HledgerError::NonZeroExitWithStderr {
                cmd: format!("{} balance --flat ({} accounts)", self.binary, accounts.len()),
                status: output.status.code().unwrap_or(-1),
                stderr: stderr.to_string(),
            })
        }
    }

    /// Validate a journal file (hledger check)
    pub fn check(&self, journal_path: &str) -> HledgerResult<()> {
        let output = Command::new(&self.binary)
            .env("LEDGER_FILE", journal_path)
            .args(&["check"])
            .output()
            .map_err(|e| HledgerError::CommandFailed {
                cmd: format!("{} check (LEDGER_FILE={})", self.binary, journal_path),
                source: e,
            })?;

        if output.status.success() {
            Ok(())
        } else {
            let _stderr = String::from_utf8_lossy(&output.stderr);
            Err(HledgerError::NonZeroExit {
                cmd: format!("{} check (LEDGER_FILE={})", self.binary, journal_path),
                status: output.status.code().unwrap_or(-1),
            })
        }
    }
}

impl Default for Hledger {
    fn default() -> Self {
        Self::new()
    }
}
