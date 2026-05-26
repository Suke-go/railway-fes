//! ESC/POS command byte builders for EPSON TM-T90II.
//!
//! Command set verified against the working `bakebake-generator` Python
//! implementation (see `reference-bakebake-printer` memory). We initialize for
//! Japanese (Shift_JIS / Kanji mode) at session start.

#![allow(dead_code)]

pub const INIT: &[u8] = &[0x1B, 0x40];
pub const KANJI_ON: &[u8] = &[0x1C, 0x26];
pub const KANJI_SHIFT_JIS: &[u8] = &[0x1C, 0x43, 0x01];
pub const CUT_FULL: &[u8] = &[0x1D, 0x56, 0x00];

/// Feed `n` lines. ESC d n.
pub fn feed_lines(n: u8) -> [u8; 3] {
    [0x1B, 0x64, n]
}

/// Feed `n` dot-rows for finer-grained paper advance. ESC J n.
pub fn feed_dots(n: u8) -> [u8; 3] {
    [0x1B, 0x4A, n]
}

/// Session init: reset + kanji + Shift_JIS.
pub fn session_open() -> Vec<u8> {
    let mut v = Vec::with_capacity(8);
    v.extend_from_slice(INIT);
    v.extend_from_slice(KANJI_ON);
    v.extend_from_slice(KANJI_SHIFT_JIS);
    v
}

/// Encode Japanese text as Shift_JIS for the printer.
pub fn encode_sjis(text: &str) -> Vec<u8> {
    let (cow, _, _) = encoding_rs::SHIFT_JIS.encode(text);
    cow.into_owned()
}
