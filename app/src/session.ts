// In-memory mirror of the backend session. Screens read from here to render
// summaries; the backend remains the source of truth.

export interface SessionSummary {
  phrase_count: number;
  audio_duration_ms: number;
  transcript_path: string;
  audio_path: string;
}

let transcriptPath: string | null = null;
let audioPath: string | null = null;
let summary: SessionSummary | null = null;

export function setTranscriptPath(p: string | null): void {
  transcriptPath = p;
}
export function getTranscriptPath(): string | null {
  return transcriptPath;
}

export function setAudioPath(p: string | null): void {
  audioPath = p;
}
export function getAudioPath(): string | null {
  return audioPath;
}

export function setSummary(s: SessionSummary | null): void {
  summary = s;
}
export function getSummary(): SessionSummary | null {
  return summary;
}

export function reset(): void {
  transcriptPath = null;
  audioPath = null;
  summary = null;
}
