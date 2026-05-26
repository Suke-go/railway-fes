//! Persistent app settings (API key, calibration offset, etc.).
//!
//! Stored as JSON under the OS-appropriate config dir:
//! * macOS: `~/Library/Application Support/Receipt/settings.json`
//! * Windows: `%APPDATA%\Receipt\settings.json`
//! * Linux: `~/.local/share/Receipt/settings.json`
//!
//! The API key in this file takes precedence over the `OPENAI_API_KEY`
//! environment variable. Removing the key with `clear_api_key` falls back to
//! the env var if one is set.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// OpenAI API key. `None` means "use the OPENAI_API_KEY env var if any".
    #[serde(default)]
    pub openai_api_key: Option<String>,

    /// Printer calibration offset in ms (positive = send print earlier).
    /// Reserved for the calibration mode work.
    #[serde(default)]
    pub printer_offset_ms: Option<i64>,
}

impl Settings {
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("settings load failed, using defaults: {e:#}");
                Self::default()
            }
        }
    }

    fn try_load() -> Result<Self> {
        let path = settings_path()?;
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e).context(format!("read {}", path.display())),
        };
        Ok(serde_json::from_slice(&bytes).context("decode settings.json")?)
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("mkdir {}", parent.display()))?;
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    /// Effective API key: settings file value first, env var fallback.
    pub fn resolve_api_key(&self) -> Option<String> {
        if let Some(k) = self.openai_api_key.as_deref() {
            if !k.is_empty() {
                return Some(k.to_string());
            }
        }
        std::env::var("OPENAI_API_KEY").ok().filter(|s| !s.is_empty())
    }
}

fn settings_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("locating user config dir")?;
    Ok(base.join("Receipt").join("settings.json"))
}
