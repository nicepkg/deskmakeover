/**
 * Keeps public/engine/dm_icon_wasm.wasm — the playground's engine module —
 * current, mirroring the font-subset pattern:
 *
 * - With a fresh cargo artifact on this machine (dev): copies
 *   target/wasm32-unknown-unknown/release/dm_icon_wasm.wasm into public/engine/.
 * - Without one (CI has no Rust toolchain): verifies the committed copy exists
 *   and fails the build loudly if it is missing.
 *
 * Rebuild the artifact: cargo build -p dm-icon-wasm --target wasm32-unknown-unknown --release
 */
import { access, copyFile, mkdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(here, "..");
const artifact = path.resolve(websiteRoot, "../target/wasm32-unknown-unknown/release/dm_icon_wasm.wasm");
const served = path.join(websiteRoot, "public/engine/dm_icon_wasm.wasm");

async function exists(p) {
  try {
    await access(p);
    return true;
  } catch {
    return false;
  }
}

if (await exists(artifact)) {
  const [a, s] = [await stat(artifact), (await exists(served)) ? await stat(served) : null];
  if (!s || a.mtimeMs > s.mtimeMs || a.size !== s.size) {
    await mkdir(path.dirname(served), { recursive: true });
    await copyFile(artifact, served);
    console.log(`engine wasm: copied ${(a.size / 1024).toFixed(0)} KB -> public/engine/dm_icon_wasm.wasm`);
  } else {
    console.log("engine wasm: committed copy is current.");
  }
} else if (await exists(served)) {
  console.log("engine wasm: no cargo artifact here; using the committed copy.");
} else {
  console.error("Missing public/engine/dm_icon_wasm.wasm and no cargo artifact to copy.");
  console.error("Run: cargo build -p dm-icon-wasm --target wasm32-unknown-unknown --release");
  process.exit(1);
}
