import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { setState, type AppState } from "./state";
import { wireIdleScreen } from "./screens/idle";
import { wireReadyScreen } from "./screens/ready";
import { wireCalibrationScreen } from "./screens/calibration";
import { wirePlayingScreen } from "./screens/playing";
import { wireDoneScreen } from "./screens/done";
import { initAlignmentStatus } from "./alignment-status";

window.addEventListener("DOMContentLoaded", async () => {
  // Sanity-check the Rust backend is reachable before showing anything.
  try {
    const pong = await invoke<string>("ping");
    console.info("backend ping:", pong);
  } catch (err) {
    console.error("backend unreachable:", err);
  }

  initAlignmentStatus();
  wireIdleScreen();
  wireReadyScreen();
  wireCalibrationScreen();
  wirePlayingScreen();
  wireDoneScreen();

  // Backend can drive state transitions by emitting `receipt://state` events.
  listen<AppState>("receipt://state", (e) => setState(e.payload));

  setState("idle");
});
