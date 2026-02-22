use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use memmap2::Mmap;

pub const MMAP_THRESHOLD: u64 = 64 * 1024;
pub const BINARY_CHECK_SIZE: usize = 8192;

pub fn read_file(path: &Path) -> Result<Option<String>, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() == 0 {
        return Ok(None);
    }
    if metadata.len() >= MMAP_THRESHOLD {
        read_file_mmap(path)
    } else {
        read_file_buffered(path)
    }
}

fn read_file_mmap(path: &Path) -> Result<Option<String>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mmap = unsafe { Mmap::map(&file) }.map_err(|e| e.to_string())?;
    let data = &mmap[..];
    if is_binary(data) {
        return Ok(None);
    }
    let s = std::str::from_utf8(data).map_err(|_| "Not valid UTF-8".to_string())?;
    Ok(Some(s.to_owned()))
}

fn read_file_buffered(path: &Path) -> Result<Option<String>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut check_buf = [0u8; BINARY_CHECK_SIZE];
    let n = reader.read(&mut check_buf).map_err(|e| e.to_string())?;
    if is_binary(&check_buf[..n]) {
        return Ok(None);
    }
    let mut all = Vec::from(&check_buf[..n]);
    reader.read_to_end(&mut all).map_err(|e| e.to_string())?;
    let s = std::str::from_utf8(&all).map_err(|_| "Not valid UTF-8".to_string())?;
    Ok(Some(s.to_owned()))
}

pub fn is_binary(data: &[u8]) -> bool {
    let check_len = data.len().min(BINARY_CHECK_SIZE);
    data[..check_len].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_detects_null_bytes() {
        assert!(is_binary(&[0x48, 0x65, 0x00, 0x6c]));
    }

    #[test]
    fn binary_clean_text() {
        assert!(!is_binary(b"Hello, world!"));
    }

    #[test]
    fn binary_empty() {
        assert!(!is_binary(&[]));
    }
}
