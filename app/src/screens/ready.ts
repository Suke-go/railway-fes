// Ready screen — session is loaded. Show summary + current alignment status,
// let user start playback, re-trigger alignment, or reset back to file pickers.

import { invoke } from "@tauri-apps/api/core";
import { setState, onStateChange } from "../state";
import { getSummary, reset as resetSession } from "../session";
import { describe, getStatus, onStatus, reset as resetAlignment } from "../alignment-status";

export function wireReadyScreen(): void {
  const summaryEl = document.getElementById("ready-summary");
  const alignmentEl = document.getElementById("alignment-status");

  function renderSummary() {
    if (!summaryEl) return;
    const s = getSummary();
    if (!s) {
      summaryEl.textContent = "";
      return;
    }
    const mins = Math.floor(s.audio_duration_ms / 60000);
    const secs = Math.floor((s.audio_duration_ms % 60000) / 1000);
    summaryEl.textContent = `${s.phrase_count} 句 / 音声長 ${mins}:${String(secs).padStart(2, "0")}`;
  }

  function renderAlignment() {
    if (alignmentEl) alignmentEl.textContent = describe(getStatus());
  }

  onStateChange((next) => {
    if (next !== "ready") return;
    renderSummary();
    renderAlignment();
  });

  onStatus(() => renderAlignment());

  document
    .getElementById("btn-calibrate")
    ?.addEventListener("click", () => setState("calibration"));

  document.getElementById("btn-start")?.addEventListener("click", async () => {
    try {
      await invoke("start_playback");
      setState("playing");
    } catch (err) {
      console.error("start_playback failed:", err);
      alert(`再生開始失敗: ${err}`);
    }
  });

  document.getElementById("btn-realign")?.addEventListener("click", async () => {
    try {
      await invoke("realign");
    } catch (err) {
      console.error("realign failed:", err);
      alert(`再アラインメント失敗: ${err}`);
    }
  });

  document.getElementById("btn-reset-from-ready")?.addEventListener("click", async () => {
    try {
      await invoke("clear_session");
    } catch (err) {
      console.warn("clear_session failed:", err);
    }
    resetSession();
    resetAlignment();
    setState("idle");
  });
}
