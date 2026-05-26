// Tracks the latest receipt://alignment event from the backend and exposes
// the current status as a label for the ready screen to render.

import { listen } from "@tauri-apps/api/event";

export type AlignmentStatus =
  | { kind: "idle" }
  | { kind: "started" }
  | { kind: "finished"; source: "openai_whisper" | "proportional" }
  | { kind: "failed"; message: string }
  | { kind: "skipped"; reason: string };

type Listener = (s: AlignmentStatus) => void;

let current: AlignmentStatus = { kind: "idle" };
const listeners: Listener[] = [];

function emit(next: AlignmentStatus) {
  current = next;
  for (const fn of listeners) fn(next);
}

export function getStatus(): AlignmentStatus {
  return current;
}

export function onStatus(fn: Listener): () => void {
  listeners.push(fn);
  fn(current);
  return () => {
    const i = listeners.indexOf(fn);
    if (i !== -1) listeners.splice(i, 1);
  };
}

export function reset(): void {
  emit({ kind: "idle" });
}

export function initAlignmentStatus(): void {
  listen<AlignmentStatus>("receipt://alignment", (e) => emit(e.payload));
}

export function describe(s: AlignmentStatus): string {
  switch (s.kind) {
    case "idle":
      return "アラインメント: 未実行";
    case "started":
      return "アラインメント: 実行中…";
    case "finished":
      return s.source === "openai_whisper"
        ? "アラインメント: 完了 (Whisper API)"
        : "アラインメント: 完了 (比例配分)";
    case "failed":
      return `アラインメント失敗: ${s.message}`;
    case "skipped":
      return `アラインメント未実行: ${s.reason}`;
  }
}
