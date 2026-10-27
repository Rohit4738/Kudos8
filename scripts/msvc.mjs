// Shared helper: run a command with the MSVC (Visual Studio C++) environment loaded.
//
// Rust on Windows needs link.exe plus INCLUDE/LIB from the Visual Studio Build
// Tools. Rather than requiring every shell to be a "Developer PowerShell", the
// npm scripts route through this file, which asks `vswhere` for the install and
// imports `vcvars64.bat` once.
//
// OWNER: Session A. Non-Windows machines just run the command directly.

import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const VENDORED_SDK = path.join(REPO_ROOT, ".tools", "winsdk");
const VENDORED_SDK_TOOLS = path.join(REPO_ROOT, ".tools", "winsdk-buildtools", "bin");

const VSWHERE =
  "C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe";

function vswhereInstallPath() {
  if (!existsSync(VSWHERE)) return null;
  const result = spawnSync(
    VSWHERE,
    [
      "-latest",
      "-products",
      "*",
      "-requires",
      "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
      "-property",
      "installationPath",
    ],
    { encoding: "utf8" },
  );
  const found = (result.stdout || "").trim().split(/\r?\n/)[0];
  return found && existsSync(found) ? found : null;
}

function loadMsvcEnv() {
  if (process.platform !== "win32") return process.env;
  if (process.env.VSCMD_ARG_TGT_ARCH === "x64") return process.env; // already in a dev shell

  const candidates = [];
  const install = vswhereInstallPath();
  if (install) candidates.push(path.join(install, "VC", "Auxiliary", "Build", "vcvars64.bat"));

  // vswhere does not always see newer Visual Studio layouts (e.g. VS 18 BuildTools),
  // so also scan the known install roots.
  for (const root of [
    "C:\\Program Files\\Microsoft Visual Studio",
    "C:\\Program Files (x86)\\Microsoft Visual Studio",
  ]) {
    if (!existsSync(root)) continue;
    for (const version of readdirSync(root)) {
      for (const edition of ["BuildTools", "Community", "Professional", "Enterprise"]) {
        candidates.push(
          path.join(root, version, edition, "VC", "Auxiliary", "Build", "vcvars64.bat"),
        );
      }
    }
  }

  const vcvars = candidates.find((candidate) => existsSync(candidate));
  if (!vcvars) {
    console.warn(
      "[kudos] Visual Studio C++ build tools not found. Rust builds will fail until they are installed.",
    );
    return process.env;
  }

  // `vcvars64.bat` prints the environment it just configured; capture and merge it.
  // A temporary batch file is used because cmd.exe quoting through spawn is unreliable.
  const dir = mkdtempSync(path.join(os.tmpdir(), "kudos-msvc-"));
  const bat = path.join(dir, "env.bat");
  writeFileSync(bat, `@echo off\r\ncall "${vcvars}" >nul\r\nset\r\n`);
  const result = spawnSync(process.env.ComSpec || "cmd.exe", ["/d", "/c", bat], {
    encoding: "utf8",
  });
  if (result.status !== 0) {
    console.warn(`[kudos] failed to import MSVC environment from ${vcvars}`);
    return process.env;
  }

  const env = { ...process.env };
  for (const line of result.stdout.split(/\r?\n/)) {
    const at = line.indexOf("=");
    if (at > 0) setEnv(env, line.slice(0, at), line.slice(at + 1));
  }
  setEnv(env, "VSCMD_ARG_TGT_ARCH", "x64");

  ensureToolPaths(env);
  addVendoredWindowsSdk(env);
  return env;
}

/** Node and cargo may not be on PATH in a fresh shell; the npm scripts are. */
function ensureToolPaths(env) {
  const cargoBin = path.join(os.homedir(), ".cargo", "bin");
  const nodeBin = path.dirname(process.execPath);
  const dirs = [cargoBin, nodeBin].filter((dir) => existsSync(dir));
  if (dirs.length) {
    setEnv(env, "PATH", [...dirs, getEnv(env, "PATH")].filter(Boolean).join(";"));
  }
}

/** Windows env names are case-insensitive; JavaScript keys are not. */
function envKey(env, key) {
  const lower = key.toLowerCase();
  return Object.keys(env).find((candidate) => candidate.toLowerCase() === lower);
}

function getEnv(env, key) {
  const found = envKey(env, key);
  return found === undefined ? undefined : env[found];
}

function setEnv(env, key, value) {
  env[envKey(env, key) ?? key] = value;
}

/**
 * Some machines have the MSVC compiler but not the Windows SDK (no
 * `Kits\10\Lib`). `scripts/setup-windows-sdk.mjs` downloads the official
 * Microsoft SDK NuGet packages into `.tools/`; wire them in when present.
 */
function addVendoredWindowsSdk(env) {
  if (process.platform !== "win32") return;

  const um = path.join(VENDORED_SDK, "c", "um", "x64");
  const ucrt = path.join(VENDORED_SDK, "c", "ucrt", "x64");
  if (existsSync(um)) {
    setEnv(env, "LIB", [um, ucrt, getEnv(env, "LIB")].filter(Boolean).join(";"));
    setEnv(env, "LIBPATH", [um, ucrt, getEnv(env, "LIBPATH")].filter(Boolean).join(";"));
  } else if (!getEnv(env, "LIB")?.includes("kernel32")) {
    console.warn(
      "[kudos] Windows SDK import libraries not found. Run: npm run setup:windows-sdk",
    );
  }

  // embed-resource needs rc.exe to stamp the app icon/manifest.
  if (existsSync(VENDORED_SDK_TOOLS)) {
    for (const version of readdirSync(VENDORED_SDK_TOOLS)) {
      const bin = path.join(VENDORED_SDK_TOOLS, version, "x64");
      if (existsSync(path.join(bin, "rc.exe"))) {
        setEnv(env, "PATH", [bin, getEnv(env, "PATH")].filter(Boolean).join(";"));
        break;
      }
    }
  }

  // Headers live in `c/Include/<version>/{ucrt,shared,um,winrt,cppwinrt}`.
  const includeRoot = path.join(VENDORED_SDK, "c", "Include");
  if (existsSync(includeRoot)) {
    const version = readdirSync(includeRoot)[0];
    if (version) {
      const base = path.join(includeRoot, version);
      const dirs = ["ucrt", "shared", "um", "winrt", "cppwinrt"]
        .map((name) => path.join(base, name))
        .filter((dir) => existsSync(dir));
      setEnv(env, "INCLUDE", [...dirs, getEnv(env, "INCLUDE")].filter(Boolean).join(";"));
    }
  }
}

/**
 * Run `command args...` with the MSVC environment, inheriting stdio.
 * Exits the process with the child's exit code.
 */
export function runWithMsvc(command, args, options = {}) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    env: loadMsvcEnv(),
    shell: false,
    ...options,
  });
  if (result.error) {
    console.error(`[kudos] failed to run ${command}: ${result.error.message}`);
    process.exit(1);
  }
  process.exit(result.status ?? 1);
}

/** Resolve the cargo executable, even when ~/.cargo/bin is not on PATH yet. */
export function cargoCommand() {
  const exe = process.platform === "win32" ? "cargo.exe" : "cargo";
  const candidate = path.join(os.homedir(), ".cargo", "bin", exe);
  return existsSync(candidate) ? candidate : "cargo";
}
