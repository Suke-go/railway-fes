// Playing screen — renders the current phrase character-by-character per the
// backend's `char_start_ms` schedule, then vanishes instantly when the next
// phrase begins. The vanish-without-fade is intentional: the volatility of
// spoken words is the whole point of the screen.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { setState } from "../state";

interface PhraseStart {
  index: number;
  speaker:
    | { kind: "interviewer" }
    | { kind: "anonymous" }
    | { kind: "named"; name: string };
  text: string;
  char_start_ms: number[];
  duration_ms: number;
}

interface PlaybackStatus {
  position_ms: number;
  total_ms: number;
}

// Pending reveal timers for the currently-typewriting phrase. Cleared on
// next phrase-start or phrase-end so a slow phrase doesn't bleed into a
// faster successor.
let pendingTimers: number[] = [];
let currentPhraseIndex = -1;

function clearPending() {
  for (const id of pendingTimers) window.clearTimeout(id);
  pendingTimers = [];
}

function renderPhrase(textEl: HTMLElement, payload: PhraseStart) {
  clearPending();
  textEl.replaceChildren();

  const chars = [...payload.text];
  const spans: HTMLSpanElement[] = chars.map((ch) => {
    const span = document.createElement("span");
    span.className = "char";
    span.textContent = ch;
    return span;
  });
  textEl.append(...spans);

  for (let i = 0; i < spans.length; i++) {
    const delay = Math.max(0, payload.char_start_ms[i] ?? 0);
    const id = window.setTimeout(() => spans[i].classList.add("shown"), delay);
    pendingTimers.push(id);
  }
}

export function wirePlayingScreen(): void {
  const speakerEl = document.getElementById("phrase-speaker");
  const textEl = document.getElementById("phrase-text");
  const statusEl = document.getElementById("playback-status");

  listen<PhraseStart>("receipt://phrase-start", (e) => {
    currentPhraseIndex = e.payload.index;
    if (speakerEl) speakerEl.textContent = speakerLabel(e.payload.speaker);
    if (textEl) renderPhrase(textEl, e.payload);
  });

  listen<{ index: number }>("receipt://phrase-end", (e) => {
    if (e.payload.index !== currentPhraseIndex) return;
    clearPending();
    if (textEl) textEl.replaceChildren();
    if (speakerEl) speakerEl.textContent = "";
  });

  listen<PlaybackStatus>("receipt://playback-status", (e) => {
    if (!statusEl) return;
    statusEl.textContent = `${formatMs(e.payload.position_ms)} / ${formatMs(e.payload.total_ms)}`;
  });

  document.getElementById("btn-stop")?.addEventListener("click", async () => {
    try {
      await invoke("stop_playback");
    } catch (err) {
      console.warn("stop_playback failed:", err);
    }
    clearPending();
    if (textEl) textEl.replaceChildren();
    if (speakerEl) speakerEl.textContent = "";
    setState("ready");
  });
}

function speakerLabel(s: PhraseStart["speaker"]): string {
  if (s.kind === "interviewer") return "（聞き手）";
  if (s.kind === "named") return s.name;
  return "";
}

function formatMs(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  return `${m}:${String(s % 60).padStart(2, "0")}`;
}
