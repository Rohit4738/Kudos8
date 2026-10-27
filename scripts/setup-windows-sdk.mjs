// One-time setup for machines that have MSVC but no Windows SDK import libs.
//
// Downloads the official Microsoft Windows SDK NuGet packages into `.tools/`
// (git-ignored). `scripts/msvc.mjs` picks them up automatically.
//
// OWNER: Session A. Run with: `npm run setup:windows-sdk`

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const TOOLS = path.join(ROOT, ".tools");
const VERSION = "10.0.28000.2705";
const BUILD_TOOLS_VERSION = "10.0.28000.2705";

const PACKAGES = [
  {
    id: "microsoft.windows.sdk.cpp",
    version: VERSION,
    target: path.join(TOOLS, "winsdk"),
    marker: path.join(TOOLS, "winsdk", "c", "Include"),
  },
  {
    id: "microsoft.windows.sdk.cpp.x64",
    version: VERSION,
    target: path.join(TOOLS, "winsdk"),
    marker: path.join(TOOLS, "winsdk", "c", "um", "x64", "kernel32.lib"),
  },
  {
    id: "microsoft.windows.sdk.buildtools",
    version: BUILD_TOOLS_VERSION,
    target: path.join(TOOLS, "winsdk-buildtools"),
    marker: path.join(TOOLS, "winsdk-buildtools", "bin"),
  },
];

async function download(url, file) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${response.status} ${response.statusText} for ${url}`);
  const { writeFile } = await import("node:fs/promises");
  await writeFile(file, Buffer.from(await response.arrayBuffer()));
}

function unzip(zip, target) {
  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", `Expand-Archive -LiteralPath '${zip}' -DestinationPath '${target}' -Force`],
    { stdio: "inherit" },
  );
  if (result.status !== 0) throw new Error(`failed to extract ${zip}`);
}

for (const pkg of PACKAGES) {
  if (existsSync(pkg.marker)) {
    console.log(`[kudos] ${pkg.id} already present`);
    continue;
  }
  const url = `https://api.nuget.org/v3-flatcontainer/${pkg.id}/${pkg.version}/${pkg.id}.${pkg.version}.nupkg`;
  const zip = path.join(TOOLS, `${pkg.id}.zip`);
  mkdirSync(pkg.target, { recursive: true });
  console.log(`[kudos] downloading ${pkg.id} ${pkg.version}…`);
  await download(url, zip);
  unzip(zip, pkg.target);
  console.log(`[kudos] installed into ${path.relative(ROOT, pkg.target)}`);
}

console.log("[kudos] Windows SDK ready.");
