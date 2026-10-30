use std::fs::{self, File, OpenOptions};
use std::io::{Write};
use std::path::Path;
use fs2::FileExt;
use tracing::info;

/// Error types for file operations
#[derive(Debug, thiserror::Error)]
pub enum FileHandlerError {
    #[error("Failed to open file '{0}': {1}")]
    OpenFailed(String, std::io::Error),
    #[error("Failed to lock file '{0}': {1}")]
    LockFailed(String, std::io::Error),
    #[error("Failed to write to file '{0}': {1}")]
    WriteFailed(String, std::io::Error),
    #[error("Failed to create directory for '{0}': {1}")]
    CreateDirFailed(String, std::io::Error),
}

/// Result type for file operations
pub type FileHandlerResult<T> = Result<T, FileHandlerError>;

/// File handler with advisory locking for journal files
pub struct FileHandler {
    path: String,
    file: Option<File>,
}

impl FileHandler {
    /// Open a file with advisory locking
    pub fn open<P: AsRef<Path>>(path: P) -> FileHandlerResult<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)
                .map_err(|e| FileHandlerError::CreateDirFailed(path_str.clone(), e))?;
        }

        // Open the file
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(path.as_ref())
            .map_err(|e| FileHandlerError::OpenFailed(path_str.clone(), e))?;

        // Acquire exclusive lock (blocking)
        file.lock_exclusive()
            .map_err(|e| FileHandlerError::LockFailed(path_str.clone(), e))?;

        info!("Locked file: {}", path_str);

        Ok(Self {
            path: path_str,
            file: Some(file),
        })
    }

    /// Write content to the file (append mode)
    #[allow(dead_code)] // used by integration tests via the lib crate
    pub fn append(&mut self, content: &str) -> FileHandlerResult<()> {
        let file = self.file.as_mut()
            .ok_or_else(|| FileHandlerError::OpenFailed(self.path.clone(), std::io::Error::new(std::io::ErrorKind::NotConnected, "File closed")))?;

        use std::io::Seek;
        file.seek(std::io::SeekFrom::End(0))
            .map_err(|e| FileHandlerError::WriteFailed(self.path.clone(), e))?;

        file.write_all(content.as_bytes())
            .map_err(|e| FileHandlerError::WriteFailed(self.path.clone(), e))?;

        // Flush to ensure data is written
        file.flush()
            .map_err(|e| FileHandlerError::WriteFailed(self.path.clone(), e))?;

        // Best-effort durability: a completed write should survive a
        // crash, so sync it to storage. A sync failure is logged, not
        // reported: the bytes are already in the file, and reporting
        // failure for a landed entry invites a double-submit.
        if let Err(e) = file.sync_all() {
            tracing::warn!("Failed to sync journal file after append: {}", e);
        }

        Ok(())
    }

    /// Get the file path
    #[allow(dead_code)] // used by integration tests via the lib crate
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl std::fmt::Debug for FileHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileHandler")
            .field("path", &self.path)
            .finish()
    }
}

/// Drop implementation to release the lock
impl Drop for FileHandler {
    fn drop(&mut self) {
        if let Some(file) = &mut self.file {
            file.unlock().ok();
            tracing::info!("Unlocked file: {}", self.path);
        }
    }
}

/// Resolve a client-supplied target_file against the journal root.
/// The result is always strictly under nas_path: absolute paths and any
/// `..` component are rejected, so a request can never address a file
/// outside the journal tree (no traversal escape). Note: this is a
/// lexical check. A symlink planted inside nas_path pointing outside
/// would still escape it; nas_path is operator-owned, so planting one
/// already requires local filesystem access.
pub fn resolve_target_file(nas_path: &str, target_file: &str) -> Result<String, String> {
    if target_file.is_empty() {
        return Err("target_file must not be empty".to_string());
    }
    if target_file.contains('\0') {
        return Err("target_file must not contain NUL".to_string());
    }
    let rel = std::path::Path::new(target_file);
    if rel.is_absolute() {
        return Err("target_file must be a relative path".to_string());
    }
    if rel
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("target_file must not contain '..'".to_string());
    }
    Ok(format!("{}/{}", nas_path, target_file))
}
