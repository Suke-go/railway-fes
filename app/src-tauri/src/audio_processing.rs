//! Helpers for getting an audio file into a shape the Whisper API will
//! accept. The API caps uploads at 25 MB, which a 60-minute oral history
//! comfortably exceeds. We re-encode large files to 32 kbps mono 16 kHz MP3
//! (Whisper's internal pipeline operates at 16 kHz mono anyway, so the
//! recognition side sees no further degradation).
//!
//! ffmpeg is resolved in this order:
//!   1. The bundled sidecar binary at `binaries/ffmpeg-<triple>{.exe}` via
//!      `tauri-plugin-shell`. Production builds always include this.
//!   2. The `ffmpeg` command on the user's PATH. Lets a developer skip the
//!      sidecar fetch step and just rely on a system install.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{anyhow, Context, Result};
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;
use tempfile::NamedTempFile;

const SIZE_LIMIT_BYTES: u64 = 25 * 1024 * 1024;
const COMPRESS_ABOVE_BYTES: u64 = 24 * 1024 * 1024;

pub enum Prepared {
    AsIs(PathBuf),
    Compressed {
        path: PathBuf,
        _temp: NamedTempFile,
    },
}

impl Prepared {
    pub fn path(&self) -> &Path {
        match self {
            Prepared::AsIs(p) => p,
            Prepared::Compressed { path, .. } => path,
        }
    }
}

pub async fn prepare_for_whisper(app: &AppHandle, input: &Path) -> Result<Prepared> {
    let meta = tokio::fs::metadata(input)
        .await
        .with_context(|| format!("stat {}", input.display()))?;

    if meta.len() <= COMPRESS_ABOVE_BYTES {
        return Ok(Prepared::AsIs(input.to_path_buf()));
    }

    let temp = tempfile::Builder::new()
        .prefix("receipt-whisper-")
        .suffix(".mp3")
        .tempfile()
        .context("creating temp file for compressed audio")?;
    let out_path = temp.path().to_path_buf();

    tracing::info!(
        input = %input.display(),
        size_mb = format!("{:.1}", meta.len() as f64 / 1024.0 / 1024.0),
        "compressing audio for Whisper API",
    );

    run_ffmpeg(app, input, &out_path).await?;

    let out_meta = tokio::fs::metadata(&out_path)
        .await
        .context("stat compressed audio")?;
    if out_meta.len() > SIZE_LIMIT_BYTES {
        return Err(anyhow!(
            "compressed audio is still {:.1} MB — source is too long; trim it or lower bitrate further",
            out_meta.len() as f64 / 1024.0 / 1024.0
        ));
    }

    tracing::info!(
        out_mb = format!("{:.1}", out_meta.len() as f64 / 1024.0 / 1024.0),
        "audio compressed"
    );

    Ok(Prepared::Compressed { path: out_path, _temp: temp })
}

async fn run_ffmpeg(app: &AppHandle, input: &Path, output: &Path) -> Result<()> {
    let args = ffmpeg_args(input, output);

    // 1) Prefer the bundled sidecar.
    match app.shell().sidecar("ffmpeg") {
        Ok(sidecar) => {
            let output_result = sidecar
                .args(&args)
                .output()
                .await
                .context("spawning bundled ffmpeg sidecar")?;
            if !output_result.status.success() {
                let stderr = String::from_utf8_lossy(&output_result.stderr);
                return Err(anyhow!(
                    "bundled ffmpeg exited with {:?}: {stderr}",
                    output_result.status
                ));
            }
            return Ok(());
        }
        Err(e) => {
            tracing::warn!("bundled ffmpeg sidecar unavailable ({e:#}); falling back to PATH ffmpeg");
        }
    }

    // 2) Fall back to system PATH ffmpeg.
    let status = tokio::process::Command::new("ffmpeg")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .await
        .context(
            "spawning ffmpeg from PATH (bundled sidecar missing — run `npm run fetch-ffmpeg`)",
        )?;
    if !status.success() {
        return Err(anyhow!("ffmpeg exited with {status}"));
    }
    Ok(())
}

fn ffmpeg_args(input: &Path, output: &Path) -> Vec<String> {
    vec![
        "-hide_banner".into(),
        "-loglevel".into(), "error".into(),
        "-y".into(),
        "-i".into(), input.display().to_string(),
        "-vn".into(),
        "-c:a".into(), "libmp3lame".into(),
        "-b:a".into(), "32k".into(),
        "-ac".into(), "1".into(),
        "-ar".into(), "16000".into(),
        output.display().to_string(),
    ]
}
