# oralhistory/

Drop your transcript (`.txt`) and audio (`.wav`/`.mp3`/`.flac`/`.m4a`/`.ogg`) files here.

Both single-speaker and multi-speaker formats are recognized — see the parser in [`../app/src-tauri/src/transcript.rs`](../app/src-tauri/src/transcript.rs) for the details:

- **Single speaker**: paragraphs separated by blank lines. Interviewer turns prefixed with `──`.
- **Multi speaker**: speaker labels like `名前：` (also bare `名前さん` lines) introduce paragraphs.

Personal interview material is not redistributed with this repository — see the root `.gitignore`.
