// Idle screen — pick transcript + audio, then load the session and advance.
// Also hosts the OpenAI API key panel (persisted in backend settings.json).

import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { setState } from "../state";
import {
  setTranscriptPath,
  setAudioPath,
  setSummary,
  getTranscriptPath,
  getAudioPath,
  type SessionSummary,
} from "../session";

interface ApiKeyStatus {
  configured: boolean;
  source: "settings" | "env" | "none";
}

export function wireIdleScreen(): void {
  const pickTranscriptBtn = document.getElementById("btn-pick-transcript") as HTMLButtonElement | null;
  const pickAudioBtn = document.getElementById("btn-pick-audio") as HTMLButtonElement | null;
  const summaryEl = document.getElementById("loaded-summary");

  function refresh() {
    const t = getTranscriptPath();
    const a = getAudioPath();
    if (pickAudioBtn) pickAudioBtn.disabled = !t;
    if (summaryEl) {
      summaryEl.textContent = [
        t ? `txt: ${baseName(t)}` : "txt: —",
        a ? `音声: ${baseName(a)}` : "音声: —",
      ].join("  ");
    }
  }

  pickTranscriptBtn?.addEventListener("click", async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "transcript", extensions: ["txt"] }],
    });
    if (typeof picked !== "string") return;
    setTranscriptPath(picked);
    refresh();
  });

  pickAudioBtn?.addEventListener("click", async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "audio", extensions: ["wav", "mp3", "flac", "m4a", "ogg"] }],
    });
    if (typeof picked !== "string") return;
    setAudioPath(picked);
    refresh();

    const t = getTranscriptPath();
    if (!t) return;

    setState("loading");
    try {
      const summary = await invoke<SessionSummary>("load_session", {
        transcriptPath: t,
        audioPath: picked,
      });
      setSummary(summary);
      setState("ready");
    } catch (err) {
      console.error("load_session failed:", err);
      alert(`セッション読み込み失敗: ${err}`);
      setState("idle");
    }
  });

  wireApiKeyPanel();
  refresh();
}

function wireApiKeyPanel(): void {
  const statusEl = document.getElementById("api-key-status");
  const inputEl = document.getElementById("api-key-input") as HTMLInputElement | null;
  const setBtn = document.getElementById("btn-set-api-key");
  const clearBtn = document.getElementById("btn-clear-api-key");

  async function refresh() {
    if (!statusEl) return;
    try {
      const s = await invoke<ApiKeyStatus>("get_api_key_status");
      statusEl.textContent = describeKey(s);
    } catch (err) {
      statusEl.textContent = `API key 状態の取得失敗: ${err}`;
    }
  }

  setBtn?.addEventListener("click", async () => {
    const key = inputEl?.value.trim() ?? "";
    if (!key) {
      alert("API key を入力してください");
      return;
    }
    try {
      await invoke("set_api_key", { key });
      if (inputEl) inputEl.value = "";
      await refresh();
    } catch (err) {
      alert(`API key 保存失敗: ${err}`);
    }
  });

  clearBtn?.addEventListener("click", async () => {
    try {
      await invoke("clear_api_key");
      await refresh();
    } catch (err) {
      alert(`API key 削除失敗: ${err}`);
    }
  });

  refresh();
}

function describeKey(s: ApiKeyStatus): string {
  if (s.configured && s.source === "settings") return "API key: 設定済 (settings.json)";
  if (s.configured && s.source === "env") return "API key: 環境変数 OPENAI_API_KEY";
  return "API key: 未設定 (アラインメントは比例配分にフォールバック)";
}

function baseName(p: string): string {
  const m = p.split(/[\\/]/);
  return m[m.length - 1] ?? p;
}
