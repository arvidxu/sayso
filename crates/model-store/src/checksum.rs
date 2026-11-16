use std::fs::File;
use std::io::Read;
use std::path::Path;

use dictation_types::{AppError, AppResult, CancelToken};
use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256 of the file, read `buffer_bytes` at a time; stops when `cancel` is used.
pub fn sha256_of_file(path: &Path, buffer_bytes: usize, cancel: &CancelToken) -> AppResult<String> {
    let mut file = File::open(path).map_err(|err| {
        AppError::storage(format!(
            "failed to open '{}' to check it: {err}",
            path.display()
        ))
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; buffer_bytes.max(1)];
    loop {
        if cancel.is_cancelled() {
            return Err(AppError::cancelled("checking the file was cancelled"));
        }
        let read = file.read(&mut buffer).map_err(|err| {
            AppError::storage(format!("failed to read '{}': {err}", path.display()))
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(to_hex(&hasher.finalize()))
}

pub fn same_checksum(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected.trim())
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
