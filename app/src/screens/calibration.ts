// Calibration screen — drives test prints and captures the operator's
// Space-key press marking the moment a known line emerges from the tear-off.

export function wireCalibrationScreen(): void {
  document.getElementById("btn-calibration-start")?.addEventListener("click", () => {
    console.info("TODO: invoke('calibration_start')");
  });

  window.addEventListener("keydown", (e) => {
    if (e.code !== "Space") return;
    const app = document.getElementById("app");
    if (app?.dataset.state !== "calibration") return;
    e.preventDefault();
    console.info("TODO: invoke('calibration_mark')");
  });
}
