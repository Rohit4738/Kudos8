// `npm run rust -- <cargo args>` -> cargo with the MSVC build environment loaded.
// Runs inside src-tauri so `cargo check` / `cargo test` work from the repo root.
// OWNER: Session A.

import path from "node:path";
import { fileURLToPath } from "node:url";
import { cargoCommand, runWithMsvc } from "./msvc.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

runWithMsvc(cargoCommand(), process.argv.slice(2), {
  cwd: path.join(root, "src-tauri"),
});
