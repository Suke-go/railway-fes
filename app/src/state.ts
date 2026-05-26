// Single source of truth for which <section data-screen="…"> is visible.
// Each screen module owns its own DOM behavior; this module owns visibility.

export type AppState =
  | "idle"
  | "loading"
  | "ready"
  | "calibration"
  | "playing"
  | "done";

const ORDER: AppState[] = ["idle", "loading", "ready", "calibration", "playing", "done"];

type Listener = (next: AppState, prev: AppState) => void;
const listeners: Listener[] = [];
let current: AppState = "idle";

export function getState(): AppState {
  return current;
}

export function setState(next: AppState): void {
  if (!ORDER.includes(next)) {
    console.warn("unknown state:", next);
    return;
  }
  const prev = current;
  current = next;

  const app = document.getElementById("app");
  if (app) app.dataset.state = next;

  for (const section of document.querySelectorAll<HTMLElement>("[data-screen]")) {
    section.hidden = section.dataset.screen !== next;
  }

  for (const fn of listeners) fn(next, prev);
}

export function onStateChange(fn: Listener): () => void {
  listeners.push(fn);
  return () => {
    const i = listeners.indexOf(fn);
    if (i !== -1) listeners.splice(i, 1);
  };
}
