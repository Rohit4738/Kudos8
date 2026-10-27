// `npm run tauri <args>` -> Tauri CLI with the MSVC build environment loaded.
// OWNER: Session A.

import path from "node:path";
import { fileURLToPath } from "node:url";
import { runWithMsvc } from "./msvc.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cli = path.join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");

runWithMsvc(process.execPath, [cli, ...process.argv.slice(2)]);
