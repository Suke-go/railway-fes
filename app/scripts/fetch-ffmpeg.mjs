// Copies the ffmpeg binary provided by the `ffmpeg-static` npm package into
// `src-tauri/binaries/ffmpeg-<TARGET_TRIPLE>{.exe}`, matching Tauri 2's
// sidecar naming convention. The Tauri bundler then ships it inside the
// .app / .exe so end-users don't need ffmpeg on PATH.
//
// Run this once per development machine before `tauri dev` or `tauri build`.
// Each platform produces its own triple; for cross-platform release builds,
// run the script on each target platform's build agent.

import { copyFileSync, mkdirSync, chmodSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import ffmpegStatic from "ffmpeg-static";

const __dirname = dirname(fileURLToPath(import.meta.url));
const binDir = join(__dirname, "..", "src-tauri", "binaries");
mkdirSync(binDir, { recursive: true });

const triple = process.env.TARGET_TRIPLE ?? detectTriple();
const ext = triple.includes("windows") ? ".exe" : "";
const dest = join(binDir, `ffmpeg-${triple}${ext}`);

copyFileSync(ffmpegStatic, dest);
if (!ext) chmodSync(dest, 0o755);
console.log(`fetched ffmpeg → ${dest}`);

function detectTriple() {
  const { platform, arch } = process;
  const map = {
    "win32-x64": "x86_64-pc-windows-msvc",
    "darwin-x64": "x86_64-apple-darwin",
    "darwin-arm64": "aarch64-apple-darwin",
    "linux-x64": "x86_64-unknown-linux-gnu",
    "linux-arm64": "aarch64-unknown-linux-gnu",
  };
  const key = `${platform}-${arch}`;
  const triple = map[key];
  if (!triple) throw new Error(`unsupported host: ${key}`);
  return triple;
}
