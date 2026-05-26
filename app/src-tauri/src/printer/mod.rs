//! Thermal printer abstraction.
//!
//! Receipt drives an EPSON TM-T90II (80mm, ESC/POS, 576px wide) directly over
//! USB to avoid the OS print-spooler buffering that would destroy our
//! sync timing. See `reference-bakebake-printer` memory for the prior Python
//! implementation we are NOT reusing for transport (only for command set).

mod escpos;

#[cfg(not(target_os = "windows"))]
mod rusb_backend;

mod stub_backend;

use anyhow::Result;

/// All printer backends speak this trait. The scheduler holds a `Box<dyn
/// PrinterBackend + Send>` so it can be swapped per-platform or replaced with
/// `StubBackend` for development without hardware.
pub trait PrinterBackend: Send {
    /// Print one phrase. The backend is responsible for any line wrapping,
    /// encoding (Shift_JIS), and feeding required after the phrase.
    fn print_phrase(&mut self, text: &str) -> Result<()>;

    /// Feed `n` blank lines (used between paragraphs, at end of session).
    fn feed(&mut self, n: u8) -> Result<()>;

    /// Cut the paper (full cut) — only called at end of session.
    fn cut(&mut self) -> Result<()>;
}

/// Returns the platform's default backend for production use, or the stub if
/// no hardware backend is compiled for this platform.
pub fn default_backend() -> Box<dyn PrinterBackend> {
    #[cfg(not(target_os = "windows"))]
    {
        match rusb_backend::RusbBackend::open_default() {
            Ok(b) => return Box::new(b),
            Err(e) => {
                tracing::warn!("rusb backend unavailable, using stub: {e:#}");
            }
        }
    }
    Box::new(stub_backend::StubBackend::default())
}
