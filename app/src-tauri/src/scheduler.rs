//! Drives synced output. Runs on a dedicated OS thread because rodio's
//! `OutputStream` is `!Send` on Windows/macOS (audio APIs have thread
//! affinity), so we can't move the player across tokio tasks. The thread
//! owns the player for the duration of playback.
//!
//! Cancellation is a shared `AtomicBool` checked once per ~16 ms tick.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::alignment::Alignment;
use crate::audio::AudioPlayer;
use crate::printer;
use crate::transcript::{Speaker, Transcript};

#[derive(Debug, Clone, Serialize)]
struct PhraseStartEvent<'a> {
    index: usize,
    speaker: &'a Speaker,
    text: &'a str,
    /// Per-character reveal offsets in ms, measured from the moment this event
    /// fires (i.e. relative to phrase start). Length matches `text.chars()`.
    char_start_ms: &'a [u32],
    /// Total phrase duration in ms. The frontend uses it as a sanity cap on
    /// the last char's reveal time.
    duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct PhraseEndEvent {
    index: usize,
}

#[derive(Debug, Clone, Serialize)]
struct StatusEvent {
    position_ms: u64,
    total_ms: u64,
}

/// Spawn a background thread that loads the audio, opens a printer, and
/// drives playback. Returns immediately; progress is reported via Tauri
/// events emitted from inside the thread.
pub fn spawn(
    app: AppHandle,
    audio_path: std::path::PathBuf,
    transcript: Arc<Transcript>,
    alignment: Arc<Alignment>,
    cancel: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        if let Err(e) = run_blocking(app, &audio_path, transcript, alignment, cancel) {
            tracing::error!("scheduler thread exited with error: {e:#}");
        }
    });
}

fn run_blocking(
    app: AppHandle,
    audio_path: &Path,
    transcript: Arc<Transcript>,
    alignment: Arc<Alignment>,
    cancel: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let player = AudioPlayer::load(audio_path)?;
    let mut printer = printer::default_backend();

    let total_ms = player.duration().as_millis() as u64;
    player.play();

    let mut next_phrase_idx = 0usize;
    let mut current_phrase_idx: Option<usize> = None;

    let tick = Duration::from_millis(16);
    let mut last_status_emit = 0u64;

    loop {
        if cancel.load(Ordering::Relaxed) {
            tracing::info!("scheduler cancelled");
            break;
        }

        let pos = player.position_ms();

        while next_phrase_idx < alignment.phrases.len()
            && alignment.phrases[next_phrase_idx].start_ms <= pos
        {
            let aligned = &alignment.phrases[next_phrase_idx];
            let phrase = &transcript.phrases[aligned.phrase_index];

            if let Some(prev) = current_phrase_idx {
                let _ = app.emit("receipt://phrase-end", PhraseEndEvent { index: prev });
            }

            let _ = app.emit(
                "receipt://phrase-start",
                PhraseStartEvent {
                    index: phrase.index,
                    speaker: &phrase.speaker,
                    text: &phrase.text,
                    char_start_ms: &aligned.char_start_ms,
                    duration_ms: aligned.end_ms.saturating_sub(aligned.start_ms),
                },
            );

            if let Err(e) = printer.print_phrase(&phrase.text) {
                tracing::warn!("printer error: {e:#}");
            }

            current_phrase_idx = Some(phrase.index);
            next_phrase_idx += 1;
        }

        // Throttle status emits to ~10Hz so we don't drown the IPC channel.
        if pos.saturating_sub(last_status_emit) >= 100 {
            let _ = app.emit(
                "receipt://playback-status",
                StatusEvent { position_ms: pos, total_ms },
            );
            last_status_emit = pos;
        }

        if player.is_finished() || pos >= total_ms {
            break;
        }
        std::thread::sleep(tick);
    }

    if let Some(prev) = current_phrase_idx {
        let _ = app.emit("receipt://phrase-end", PhraseEndEvent { index: prev });
    }
    let _ = printer.cut();
    let _ = app.emit("receipt://state", "done");
    Ok(())
}
