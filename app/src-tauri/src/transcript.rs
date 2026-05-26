//! Parses oral history transcripts into a flat sequence of `Phrase`s that drive
//! both the on-screen display and the thermal printer.
//!
//! Two on-disk formats are supported (both share UTF-8 encoding):
//!
//! 1. **Single-speaker** (e.g. `yoshida.txt`)
//!    - Paragraphs separated by blank lines.
//!    - Interviewer turns are prefixed with `──`.
//!    - No explicit speaker labels.
//!
//! 2. **Multi-speaker** (e.g. `otsuka.txt`)
//!    - Same paragraph/blank-line structure.
//!    - A line containing only `名前：` (a name followed by `：` or `:`) labels
//!      the speaker for subsequent paragraphs until the next label.
//!    - Interviewer turns are still prefixed with `──`.
//!
//! Each non-empty paragraph is then split into phrases at `、` and `。`. Phrases
//! are the atomic sync unit (see [[project-receipt-design-decisions]] memory).

use serde::Serialize;

/// One paragraph from the transcript, before phrase splitting.
#[derive(Debug, Clone)]
struct Paragraph {
    speaker: Speaker,
    text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Speaker {
    Interviewer,
    /// Struct variant (not tuple) so it works with internally-tagged
    /// serialization. Frontend sees `{"kind": "named", "name": "大塚さん"}`.
    Named { name: String },
    /// Single-speaker transcripts where no name labels appear.
    Anonymous,
}

/// A unit of synchronized output. One `Phrase` is one print burst on the
/// receipt printer and one screen of typewriter text.
#[derive(Debug, Clone, Serialize)]
pub struct Phrase {
    /// 0-based index across the whole transcript.
    pub index: usize,
    /// Speaker carried over from the enclosing paragraph.
    pub speaker: Speaker,
    /// Text content. Trailing punctuation is preserved (e.g. `、`/`。`).
    pub text: String,
    /// True for the first phrase of its paragraph — the display can use this
    /// to add a leading line break / speaker label on the printout.
    pub paragraph_start: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Transcript {
    pub phrases: Vec<Phrase>,
}

const INTERVIEWER_PREFIX: &str = "──";

pub fn parse(input: &str) -> Transcript {
    let paragraphs = collect_paragraphs(input);
    let phrases = split_phrases(&paragraphs);
    Transcript { phrases }
}

fn collect_paragraphs(input: &str) -> Vec<Paragraph> {
    let mut out = Vec::new();
    let mut current_speaker = Speaker::Anonymous;
    let mut buf = String::new();

    for raw_line in input.lines() {
        let line = raw_line.trim();

        if line.is_empty() {
            flush(&mut buf, &current_speaker, &mut out);
            continue;
        }

        // Speaker label line: only a name followed by ：/:, no other content.
        if let Some(name) = parse_speaker_label(line) {
            flush(&mut buf, &current_speaker, &mut out);
            current_speaker = Speaker::Named { name };
            continue;
        }

        // Interviewer turn — emit immediately as its own paragraph so it does
        // not get bundled with whatever speaker label was last active.
        if let Some(stripped) = line.strip_prefix(INTERVIEWER_PREFIX) {
            flush(&mut buf, &current_speaker, &mut out);
            out.push(Paragraph {
                speaker: Speaker::Interviewer,
                text: stripped.trim_start_matches('　').trim().to_string(),
            });
            continue;
        }

        // Otherwise: regular content line. Append (with a space if we're
        // continuing a multi-line paragraph). Leading `　` (full-width space)
        // is a common indent in these transcripts — drop it.
        let normalized = line.trim_start_matches('　');
        if !buf.is_empty() {
            buf.push(' ');
        }
        buf.push_str(normalized);
    }
    flush(&mut buf, &current_speaker, &mut out);
    out
}

fn flush(buf: &mut String, speaker: &Speaker, out: &mut Vec<Paragraph>) {
    if buf.trim().is_empty() {
        buf.clear();
        return;
    }
    out.push(Paragraph {
        speaker: speaker.clone(),
        text: std::mem::take(buf),
    });
}

/// Returns `Some(name)` if `line` is *only* a speaker label like `大塚さん：`,
/// `大塚さん:`, or just `大塚さん` (when the trailing colon was dropped by the
/// transcriber, which happens in otsuka.txt line 9).
///
/// Heuristic for the colon-less form: short line ending in a Japanese honorific
/// (さん / 君 / 氏 / 先生). This is intentionally narrow to avoid mistaking a
/// content line that happens to end with the honorific for a label — content
/// lines are almost always longer than a bare name and usually end with
/// punctuation (。/、/!).
fn parse_speaker_label(line: &str) -> Option<String> {
    if let Some((name, rest)) = split_once_on_colon(line) {
        if rest.trim().is_empty() {
            let name = name.trim();
            if !name.is_empty() && name.chars().count() <= 20 {
                return Some(name.to_string());
            }
        }
    }

    // Colon-less form: short, no terminal punctuation, ends with honorific.
    let trimmed = line.trim();
    if trimmed.chars().count() > 15 {
        return None;
    }
    if trimmed.ends_with('。') || trimmed.ends_with('、') || trimmed.ends_with('！') {
        return None;
    }
    for honorific in ["さん", "君", "氏", "先生"] {
        if trimmed.ends_with(honorific) {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn split_once_on_colon(s: &str) -> Option<(&str, &str)> {
    // Prefer full-width 「：」 as it's the dominant convention in JP transcripts.
    if let Some(idx) = s.find('：') {
        return Some((&s[..idx], &s[idx + '：'.len_utf8()..]));
    }
    s.split_once(':')
}

fn split_phrases(paragraphs: &[Paragraph]) -> Vec<Phrase> {
    let mut out = Vec::new();
    for para in paragraphs {
        let pieces = split_paragraph_into_phrases(&para.text);
        for (i, text) in pieces.into_iter().enumerate() {
            out.push(Phrase {
                index: out.len(),
                speaker: para.speaker.clone(),
                text,
                paragraph_start: i == 0,
            });
        }
    }
    out
}

/// Splits a paragraph at `、` and `。`, keeping the punctuation attached to the
/// preceding phrase. Empty fragments are dropped. ASCII `.` / `,` are NOT
/// treated as splitters because the transcripts are Japanese and might contain
/// them inside proper nouns or dates.
fn split_paragraph_into_phrases(text: &str) -> Vec<String> {
    let mut phrases = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if ch == '、' || ch == '。' {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                phrases.push(trimmed.to_string());
            }
            current.clear();
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        phrases.push(tail.to_string());
    }
    phrases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_speaker_with_interviewer_marker() {
        let input = "──質問ですか。\n\n答えです、はい。\n";
        let t = parse(input);
        assert_eq!(t.phrases.len(), 3);
        assert_eq!(t.phrases[0].speaker, Speaker::Interviewer);
        assert_eq!(t.phrases[0].text, "質問ですか。");
        assert_eq!(t.phrases[1].speaker, Speaker::Anonymous);
        assert_eq!(t.phrases[1].text, "答えです、");
        assert!(t.phrases[1].paragraph_start);
        assert_eq!(t.phrases[2].text, "はい。");
        assert!(!t.phrases[2].paragraph_start);
    }

    #[test]
    fn parses_multi_speaker_labels() {
        let input = "大塚さん：\nあのね、そうだよ。\n\n岩田さん：\n違うと思う。\n";
        let t = parse(input);
        assert_eq!(t.phrases.len(), 3);
        assert_eq!(t.phrases[0].speaker, Speaker::Named { name: "大塚さん".into() });
        assert_eq!(t.phrases[0].text, "あのね、");
        assert_eq!(t.phrases[1].text, "そうだよ。");
        assert_eq!(t.phrases[2].speaker, Speaker::Named { name: "岩田さん".into() });
        assert_eq!(t.phrases[2].text, "違うと思う。");
    }

    #[test]
    fn ignores_blank_lines_and_full_width_indents() {
        let input = "　最初の段落です。\n\n　\n　次の段落、続きです。\n";
        let t = parse(input);
        assert_eq!(t.phrases.len(), 3);
        assert_eq!(t.phrases[0].text, "最初の段落です。");
        assert_eq!(t.phrases[1].text, "次の段落、");
        assert_eq!(t.phrases[2].text, "続きです。");
    }

    #[test]
    fn standalone_name_without_colon_is_a_speaker_label() {
        // otsuka.txt line 9 has `大塚さん` (no colon) on its own line,
        // mid-paragraph between blank lines. Must still be detected as a label.
        let input = "大塚さん：\nまず最初の発言です。\n\n大塚さん\nいや、次の発言です。\n";
        let t = parse(input);
        assert_eq!(t.phrases.len(), 3);
        assert_eq!(t.phrases[0].speaker, Speaker::Named { name: "大塚さん".into() });
        assert_eq!(t.phrases[0].text, "まず最初の発言です。");
        assert_eq!(t.phrases[1].speaker, Speaker::Named { name: "大塚さん".into() });
        assert_eq!(t.phrases[1].text, "いや、");
        assert!(t.phrases[1].paragraph_start);
        assert_eq!(t.phrases[2].text, "次の発言です。");
        assert!(!t.phrases[2].paragraph_start);
    }

    #[test]
    fn colon_in_content_is_not_a_speaker_label() {
        // A line with content after the colon is regular text, not a label.
        let input = "時刻は10:30でした。\n";
        let t = parse(input);
        assert_eq!(t.phrases.len(), 1);
        assert_eq!(t.phrases[0].speaker, Speaker::Anonymous);
        assert_eq!(t.phrases[0].text, "時刻は10:30でした。");
    }
}
