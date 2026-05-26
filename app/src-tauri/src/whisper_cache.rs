//! Cache for OpenAI Whisper API responses, keyed by audio file content hash.
//!
//! Avoids paying the API cost twice for the same audio. Stored as JSON in
//! `<data_dir>/Receipt/whisper_cache/<sha256>.json`. The cached data is the
//! raw `words + duration` payload; phrase-level alignment (the DP/greedy
//! matching) runs fresh every load since the transcript can change.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedWords {
    pub duration: f32,
    pub words: Vec<CachedWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedWord {
    pub word: String,
    pub start: f32,
    pub end: f32,
}

/// Compute the sha256 of a file. Hashing a long audio file is fast (~hundreds
/// of MB/s) compared with the Whisper API round-trip, so we don't bother with
/// a cheaper fingerprint.
pub async fn hash_file(path: &Path) -> Result<String> {
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("read {} for hashing", path.display()))?;
    let digest = Sha256::digest(&bytes);
    Ok(hex::encode_short(&digest))
}

pub async fn lookup(hash: &str) -> Result<Option<CachedWords>> {
    let path = cache_path(hash)?;
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let v: CachedWords = serde_json::from_slice(&bytes)
                .with_context(|| format!("decode cache {}", path.display()))?;
            Ok(Some(v))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("read cache file"),
    }
}

pub async fn store(hash: &str, entry: &CachedWords) -> Result<()> {
    let path = cache_path(hash)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("mkdir {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(entry)?;
    tokio::fs::write(&path, bytes)
        .await
        .with_context(|| format!("write cache {}", path.display()))?;
    Ok(())
}

fn cache_path(hash: &str) -> Result<PathBuf> {
    let base = dirs::data_dir().context("locating user data dir")?;
    Ok(base.join("Receipt").join("whisper_cache").join(format!("{hash}.json")))
}

/// Minimal hex encoder — pulling in the `hex` crate just for this would be
/// wasteful. Lives in a submodule so the import path reads naturally.
mod hex {
    pub fn encode_short(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0xF) as usize] as char);
        }
        out
    }
}
