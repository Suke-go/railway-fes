//! Audio playback + position clock. Acts as the master clock for sync.
//!
//! `rodio::Sink::get_pos()` gives us the actual audio playback position
//! (sample-accurate, not wall clock), which is what the scheduler polls.
//! Supports WAV, MP3, FLAC via symphonia (see Cargo.toml features).

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use rodio::source::Source;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

/// Wraps an output stream + sink. Dropping this stops audio.
pub struct AudioPlayer {
    // Stream must outlive the sink; we keep both alive together.
    _stream: OutputStream,
    _handle: OutputStreamHandle,
    sink: Sink,
    duration: Duration,
}

impl AudioPlayer {
    /// Decode the audio file but don't start playing yet. Returns the total
    /// duration so the caller can build an alignment timeline.
    pub fn load(path: &Path) -> Result<Self> {
        let (stream, handle) =
            OutputStream::try_default().context("opening default audio output")?;
        let sink = Sink::try_new(&handle).context("creating audio sink")?;
        sink.pause();

        let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let decoder = Decoder::new(BufReader::new(file))
            .with_context(|| format!("decoding {}", path.display()))?;

        // total_duration() is best-effort: WAV always, MP3 only if the file
        // has accurate frame info (CBR is fine; VBR may require a scan).
        let duration = decoder
            .total_duration()
            .ok_or_else(|| anyhow::anyhow!("could not determine audio duration"))?;

        sink.append(decoder);
        Ok(Self { _stream: stream, _handle: handle, sink, duration })
    }

    pub fn play(&self) {
        self.sink.play();
    }

    pub fn stop(&self) {
        self.sink.stop();
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Current playback position in ms, sample-accurate.
    pub fn position_ms(&self) -> u64 {
        self.sink.get_pos().as_millis() as u64
    }

    pub fn is_finished(&self) -> bool {
        self.sink.empty()
    }
}
