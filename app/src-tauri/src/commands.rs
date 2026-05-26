//! Tauri command handlers — the only entry points exposed to the frontend.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::alignment::{self, Alignment, AlignmentSource};
use crate::audio::AudioPlayer;
use crate::scheduler;
use crate::settings::Settings;
use crate::transcript::{self, Transcript};

#[derive(Default, Clone)]
pub struct SessionState(Arc<Mutex<Option<Session>>>);

struct Session {
    transcript: Arc<Transcript>,
    alignment: Arc<Alignment>,
    audio_path: PathBuf,
    cancel: Option<Arc<AtomicBool>>,
}

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub phrase_count: usize,
    pub audio_duration_ms: u64,
    pub transcript_path: String,
    pub audio_path: String,
    pub alignment_source: AlignmentSource,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AlignmentProgress {
    Started,
    Finished { source: AlignmentSource },
    Failed { message: String },
    Skipped { reason: String },
}

#[tauri::command]
pub fn ping() -> &'static str {
    "pong"
}

#[tauri::command]
pub async fn load_transcript(path: String) -> Result<Transcript, String> {
    let path = PathBuf::from(path);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let text = decode_text(&bytes);
    Ok(transcript::parse(&text))
}

/// Load transcript + audio, set up a session with a proportional alignment as
/// the initial timeline, and kick off Whisper alignment in the background.
/// Frontend can call `start_playback` immediately (with proportional sync) or
/// wait for the `receipt://alignment` event with `kind: "finished"` for
/// proper speech-locked timing.
#[tauri::command]
pub async fn load_session(
    app: AppHandle,
    transcript_path: String,
    audio_path: String,
    state: State<'_, SessionState>,
) -> Result<SessionSummary, String> {
    let transcript_pb = PathBuf::from(&transcript_path);
    let bytes = tokio::fs::read(&transcript_pb)
        .await
        .map_err(|e| format!("read transcript {}: {e}", transcript_pb.display()))?;
    let text = decode_text(&bytes);
    let transcript = Arc::new(transcript::parse(&text));

    let audio_pb = PathBuf::from(&audio_path);
    let probe_path = audio_pb.clone();
    let duration_ms = std::thread::spawn(move || -> Result<u64, String> {
        let player = AudioPlayer::load(&probe_path)
            .map_err(|e| format!("open audio {}: {e:#}", probe_path.display()))?;
        Ok(player.duration().as_millis() as u64)
    })
    .join()
    .map_err(|_| "audio probe thread panicked".to_string())??;

    let initial = Arc::new(alignment::proportional_alignment(&transcript, duration_ms));

    let summary = SessionSummary {
        phrase_count: transcript.phrases.len(),
        audio_duration_ms: duration_ms,
        transcript_path,
        audio_path,
        alignment_source: initial.source,
    };

    *state.0.lock() = Some(Session {
        transcript: Arc::clone(&transcript),
        alignment: initial,
        audio_path: audio_pb.clone(),
        cancel: None,
    });

    // Fire-and-forget the API alignment. The session keeps the proportional
    // alignment until this returns, so playback works in either case.
    spawn_background_alignment(app, state.0.clone(), transcript, audio_pb);

    Ok(summary)
}

fn spawn_background_alignment(
    app: AppHandle,
    session: Arc<Mutex<Option<Session>>>,
    transcript: Arc<Transcript>,
    audio_path: PathBuf,
) {
    tokio::spawn(async move {
        let api_key = match Settings::load().resolve_api_key() {
            Some(k) => k,
            None => {
                let _ = app.emit(
                    "receipt://alignment",
                    AlignmentProgress::Skipped {
                        reason: "OpenAI API key not configured — using proportional fallback".into(),
                    },
                );
                return;
            }
        };

        let _ = app.emit("receipt://alignment", AlignmentProgress::Started);

        match alignment::align_via_openai(&app, &api_key, &audio_path, &transcript).await {
            Ok(new_alignment) => {
                let source = new_alignment.source;
                {
                    let mut guard = session.lock();
                    if let Some(s) = guard.as_mut() {
                        s.alignment = Arc::new(new_alignment);
                    }
                }
                let _ = app.emit(
                    "receipt://alignment",
                    AlignmentProgress::Finished { source },
                );
            }
            Err(e) => {
                let _ = app.emit(
                    "receipt://alignment",
                    AlignmentProgress::Failed { message: format!("{e:#}") },
                );
            }
        }
    });
}

/// Manually re-trigger Whisper alignment (e.g. after the user sets the API
/// key without restarting the app).
#[tauri::command]
pub fn realign(app: AppHandle, state: State<'_, SessionState>) -> Result<(), String> {
    let (transcript, audio_path) = {
        let guard = state.0.lock();
        let session = guard.as_ref().ok_or("no session loaded")?;
        (Arc::clone(&session.transcript), session.audio_path.clone())
    };
    spawn_background_alignment(app, state.0.clone(), transcript, audio_path);
    Ok(())
}

#[tauri::command]
pub fn start_playback(
    app: AppHandle,
    state: State<'_, SessionState>,
) -> Result<(), String> {
    let mut guard = state.0.lock();
    let session = guard.as_mut().ok_or("no session loaded")?;
    if session.cancel.is_some() {
        return Err("playback already running".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    session.cancel = Some(cancel.clone());

    scheduler::spawn(
        app,
        session.audio_path.clone(),
        Arc::clone(&session.transcript),
        Arc::clone(&session.alignment),
        cancel,
    );

    Ok(())
}

#[tauri::command]
pub fn stop_playback(state: State<'_, SessionState>) -> Result<(), String> {
    let mut guard = state.0.lock();
    let session = guard.as_mut().ok_or("no session loaded")?;
    if let Some(c) = session.cancel.take() {
        c.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct ApiKeyStatus {
    pub configured: bool,
    pub source: &'static str, // "settings" | "env" | "none"
}

#[tauri::command]
pub fn get_api_key_status() -> ApiKeyStatus {
    let s = Settings::load();
    if s.openai_api_key.as_deref().is_some_and(|k| !k.is_empty()) {
        ApiKeyStatus { configured: true, source: "settings" }
    } else if std::env::var("OPENAI_API_KEY").ok().is_some_and(|k| !k.is_empty()) {
        ApiKeyStatus { configured: true, source: "env" }
    } else {
        ApiKeyStatus { configured: false, source: "none" }
    }
}

#[tauri::command]
pub fn set_api_key(key: String) -> Result<(), String> {
    let trimmed = key.trim().to_string();
    if trimmed.is_empty() {
        return Err("empty key".into());
    }
    let mut s = Settings::load();
    s.openai_api_key = Some(trimmed);
    s.save().map_err(|e| format!("save settings: {e:#}"))?;
    Ok(())
}

#[tauri::command]
pub fn clear_api_key() -> Result<(), String> {
    let mut s = Settings::load();
    s.openai_api_key = None;
    s.save().map_err(|e| format!("save settings: {e:#}"))?;
    Ok(())
}

/// Clear the active session entirely. Used by the frontend's "reset" button.
#[tauri::command]
pub fn clear_session(state: State<'_, SessionState>) -> Result<(), String> {
    let mut guard = state.0.lock();
    if let Some(mut session) = guard.take() {
        if let Some(c) = session.cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

/// Try UTF-8 first, fall back to Shift_JIS for older Japanese transcripts.
fn decode_text(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let (cow, _, had_errors) = encoding_rs::SHIFT_JIS.decode(bytes);
    if had_errors {
        tracing::warn!("transcript decoded with replacement chars");
    }
    cow.into_owned()
}
