//! Quick CLI: `cargo run --example dump_transcript -- ../../oralhistory/yoshida.txt`
//! Prints a summary of phrases the parser produced, useful for eyeballing
//! the real transcripts.

use std::env;
use std::fs;

use app_lib::transcript;

fn main() {
    let path = env::args().nth(1).expect("usage: dump_transcript <path>");
    let bytes = fs::read(&path).expect("read transcript");
    let text = std::str::from_utf8(&bytes).expect("utf-8 transcript");
    let t = transcript::parse(text);

    println!("file: {path}");
    println!("phrases: {}", t.phrases.len());
    println!("---");
    for p in t.phrases.iter().take(20) {
        let star = if p.paragraph_start { "¶" } else { " " };
        println!(
            "{star} [{:>3}] {:?} {}",
            p.index,
            p.speaker,
            truncate(&p.text, 60)
        );
    }
    if t.phrases.len() > 20 {
        println!("... ({} more)", t.phrases.len() - 20);
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect();
    if s.chars().count() > max_chars {
        out.push('…');
    }
    out
}
