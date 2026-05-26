//! Forced alignment: produce per-phrase timestamps from (wav, transcript).
//!
//! Two paths, picked at runtime:
//!
//! * **OpenAI Whisper API** ([`align_via_openai`]) — POSTs the audio file to
//!   `/v1/audio/transcriptions` with `response_format=verbose_json` and
//!   `timestamp_granularities[]=word`, then greedily walks the returned word
//!   stream to assign timing to each of our pre-parsed phrases. Requires
//!   `OPENAI_API_KEY` in the environment. 25 MB upload limit.
//!
//! * **Proportional fallback** ([`proportional_alignment`]) — distributes the
//!   audio duration across phrases in proportion to their character count.
//!   No external dependency, no sync to actual speech, but useful when no API
//!   key is available or for quick iteration.
//!
//! The whisper-rs (local whisper.cpp) path stays as a stub for now — the API
//! path is faster to integrate and avoids the bundle-the-model question.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::audio_processing;
use crate::transcript::Transcript;
use crate::whisper_cache::{self, CachedWord, CachedWords};

/// Timing data for a single phrase, computed from alignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignedPhrase {
    pub phrase_index: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    /// Per-character start times in ms, relative to start_ms. Length matches
    /// the phrase's `text.chars().count()`. Used by the typewriter display.
    pub char_start_ms: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alignment {
    pub audio_duration_ms: u64,
    pub phrases: Vec<AlignedPhrase>,
    pub source: AlignmentSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentSource {
    Proportional,
    OpenaiWhisper,
}

/// Proportional fallback: distribute audio duration across phrases in
/// proportion to their character count.
pub fn proportional_alignment(transcript: &Transcript, audio_duration_ms: u64) -> Alignment {
    let total_chars: u64 = transcript
        .phrases
        .iter()
        .map(|p| p.text.chars().count().max(1) as u64)
        .sum::<u64>()
        .max(1);

    let mut t: u64 = 0;
    let phrases = transcript
        .phrases
        .iter()
        .map(|p| {
            let chars = p.text.chars().count().max(1) as u64;
            let dur = audio_duration_ms * chars / total_chars;
            let start = t;
            t = t.saturating_add(dur);
            AlignedPhrase {
                phrase_index: p.index,
                start_ms: start,
                end_ms: start + dur,
                char_start_ms: linear_char_timings(dur, p.text.chars().count()),
            }
        })
        .collect();

    Alignment {
        audio_duration_ms,
        phrases,
        source: AlignmentSource::Proportional,
    }
}

/// Align the transcript to the audio using OpenAI Whisper.
///
/// Flow:
/// 1. Hash the input audio. If a previous run cached the Whisper words for
///    this exact file, skip the API call entirely.
/// 2. If the file is too big for Whisper's 25 MB limit, shell out to ffmpeg
///    to re-encode to 32 kbps mono 16 kHz MP3 (transparent to caller).
/// 3. POST the (possibly compressed) file to `/v1/audio/transcriptions`.
/// 4. Cache the resulting words.
/// 5. Greedily walk the words to assign per-phrase timings.
pub async fn align_via_openai(
    app: &AppHandle,
    api_key: &str,
    audio_path: &Path,
    transcript: &Transcript,
) -> Result<Alignment> {
    // Cache lookup is keyed off the *original* file. If we re-compress
    // identical audio we'd get the same Whisper words back (modulo a
    // negligible decoder difference), so a hit on the original is safe.
    let hash = whisper_cache::hash_file(audio_path).await?;
    let cached = whisper_cache::lookup(&hash).await?;

    let cached_words = if let Some(c) = cached {
        tracing::info!(hash = %hash, "whisper cache hit");
        c
    } else {
        let prepared = audio_processing::prepare_for_whisper(app, audio_path).await?;
        let words = call_whisper_api(api_key, prepared.path()).await?;
        whisper_cache::store(&hash, &words).await?;
        words
    };

    let audio_duration_ms = (cached_words.duration * 1000.0) as u64;
    let phrases = greedy_align(transcript, &cached_words.words);

    Ok(Alignment {
        audio_duration_ms,
        phrases,
        source: AlignmentSource::OpenaiWhisper,
    })
}

async fn call_whisper_api(api_key: &str, audio_path: &Path) -> Result<CachedWords> {
    let bytes = tokio::fs::read(audio_path)
        .await
        .with_context(|| format!("read {}", audio_path.display()))?;

    let filename = audio_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".into());
    let mime = guess_mime(audio_path);

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename)
        .mime_str(mime)?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-1")
        .text("language", "ja")
        .text("response_format", "verbose_json")
        .text("timestamp_granularities[]", "word");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .context("POST /v1/audio/transcriptions")?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("whisper API {status}: {body}"));
    }

    let body: WhisperResponse = resp.json().await.context("decoding whisper response")?;
    let words = body
        .words
        .ok_or_else(|| anyhow!("whisper response missing `words` — check timestamp_granularities"))?;

    Ok(CachedWords { duration: body.duration, words })
}

#[derive(Debug, Deserialize)]
struct WhisperResponse {
    duration: f32,
    words: Option<Vec<CachedWord>>,
}

/// Greedy walk: for each phrase, consume whisper words from a moving cursor
/// until the consumed text has at least as many CJK-comparable characters as
/// our phrase. The window's first start_s / last end_s become the phrase's
/// timing. Falls back to interpolation when whisper words run out before we
/// finish.
///
/// Not optimal — a proper edit-distance DP would handle drops/insertions
/// better — but adequate when whisper roughly tracks the source transcript.
fn greedy_align(transcript: &Transcript, words: &[CachedWord]) -> Vec<AlignedPhrase> {
    let mut out: Vec<AlignedPhrase> = Vec::with_capacity(transcript.phrases.len());
    let mut word_cursor = 0usize;
    let audio_end_ms = words.last().map(|w| (w.end * 1000.0) as u64).unwrap_or(0);

    for phrase in &transcript.phrases {
        // Strip punctuation when measuring the target: whisper words usually
        // don't include 、/。 so counting them inflates the target.
        let target_chars = count_content_chars(&phrase.text);
        let start_word = word_cursor;
        let mut accumulated = 0usize;

        while word_cursor < words.len() && accumulated < target_chars {
            accumulated += count_content_chars(&words[word_cursor].word);
            word_cursor += 1;
        }

        let end_word = word_cursor;
        let (start_ms, end_ms) = if end_word > start_word {
            (
                (words[start_word].start * 1000.0) as u64,
                (words[end_word - 1].end * 1000.0) as u64,
            )
        } else if !out.is_empty() {
            // No words left — collapse to a zero-length tail at the previous
            // phrase's end so the scheduler still sees the phrase.
            let prev_end = out.last().unwrap().end_ms;
            (prev_end, prev_end)
        } else {
            (0, 0)
        };

        let dur = end_ms.saturating_sub(start_ms);
        out.push(AlignedPhrase {
            phrase_index: phrase.index,
            start_ms,
            end_ms,
            char_start_ms: linear_char_timings(dur, phrase.text.chars().count()),
        });
    }

    // Sanity: if the last aligned phrase ends before the audio does, leave
    // the trailing silence alone — the scheduler will just sit idle.
    let _ = audio_end_ms;
    out
}

/// Count characters that should affect the greedy match's "we've consumed
/// enough" decision. Whisper does not output punctuation as separate word
/// tokens (typically), and our transcripts contain Japanese punctuation
/// (、。) and full-width spaces (　) used for paragraph indentation. We also
/// drop ASCII whitespace and common interjections so length comparisons
/// between transcript phrases and whisper words stay meaningful.
fn count_content_chars(s: &str) -> usize {
    s.chars()
        .filter(|c| {
            !matches!(
                c,
                '、' | '。' | '・' | '「' | '」' | '『' | '』'
                    | '(' | ')' | '（' | '）'
                    | '　' | ' ' | '\t' | '\n' | '\r'
                    | '!' | '?' | '！' | '？'
                    | '.' | ','
            )
        })
        .count()
}

fn linear_char_timings(duration_ms: u64, nchars: usize) -> Vec<u32> {
    if nchars == 0 {
        return Vec::new();
    }
    (0..nchars)
        .map(|i| (duration_ms as f32 * i as f32 / nchars as f32) as u32)
        .collect()
}

fn guess_mime(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("ogg") | Some("oga") => "audio/ogg",
        Some("m4a") | Some("mp4") => "audio/mp4",
        Some("webm") => "audio/webm",
        _ => "application/octet-stream",
    }
}
