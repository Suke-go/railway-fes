//! No-hardware backend that just logs what would have been printed.
//! Used for development on machines without the TM-T90II attached.

use anyhow::Result;

use super::PrinterBackend;

#[derive(Default)]
pub struct StubBackend;

impl PrinterBackend for StubBackend {
    fn print_phrase(&mut self, text: &str) -> Result<()> {
        tracing::info!(target: "printer.stub", "PRINT: {text}");
        Ok(())
    }

    fn feed(&mut self, n: u8) -> Result<()> {
        tracing::info!(target: "printer.stub", "FEED {n} lines");
        Ok(())
    }

    fn cut(&mut self) -> Result<()> {
        tracing::info!(target: "printer.stub", "CUT");
        Ok(())
    }
}
