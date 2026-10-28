#!/usr/bin/env node
/**
 * Local mirror of .github/workflows/ci.yml.
 *
 * Runs the same gates, in the same order, on the current checkout and prints a
 * summary table. This file is standalone on purpose: package.json is shared, so
 * there is no `npm run verify` alias. Invoke it directly:
 *
 *   node scripts/ci/verify-local.mjs
 *   node scripts/ci/verify-local.mjs --quick
 *   node scripts/ci/verify-local.mjs --strict
 *   node scripts/ci/verify-local.mjs --fail-fast
 *
 * OWNER: Session J (repo automation). See CONTRIBUTING.md.
 */

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, "..", "..");
const IS_WINDOWS = process.platform === "win32";
const NPM = IS_WINDOWS ? "npm.cmd" : "npm";

const argv = process.argv.slice(2);
const flags = new Set(argv);
const knownFlags = new Set(["--help", "-h", "--quick", "--skip-package", "--strict", "--fail-fast"]);

const help = flags.has("--help") || flags.has("-h");
const quick = flags.has("--quick") || flags.has("--skip-package");
const strict = flags.has("--strict");
const failFast = flags.has("--fail-fast");

function usage() {
  console.log(`Kudos local verification

Usage:
  node scripts/ci/verify-local.mjs [options]

Options:
  --quick, --skip-package  Skip the slow \`npm run tauri build\` and artifact check
  --strict                 Treat advisory rustfmt/clippy failures as fatal
  --fail-fast              Stop at the first hard-gate failure
  --help, -h               Show this help

The default run mirrors CI: svelte-check, cargo check, rustfmt (advisory),
clippy (advisory), cargo test, frontend build, Tauri build and artifact check.
`);
}

if (help) {
  usage();
  process.exit(0);
}

const unknown = [...flags].filter((flag) => !knownFlags.has(flag));
if (unknown.length > 0) {
  console.error(`[verify] unknown option(s): ${unknown.join(", ")}`);
  usage();
  process.exit(2);
}

if (!existsSync(path.join(ROOT, "node_modules"))) {
  console.error("[verify] node_modules is missing. Run `npm install` first.");
  process.exit(1);
}

const pkg = JSON.parse(readFileSync(path.join(ROOT, "package.json"), "utf8"));
for (const script of ["check", "rust:check", "rust:test", "build", "tauri"]) {
  if (!pkg.scripts?.[script]) {
    console.error(`[verify] package.json is missing the "${script}" script.`);
    process.exit(1);
  }
}

const nodeMajor = Number(process.versions.node.split(".")[0]);
if (nodeMajor < 22) {
  console.warn(`[verify] Node ${process.versions.node} is old; CI uses Node 24.`);
}

/** Run npm with args, using cmd.exe on Windows so .cmd shims work without a shell warning. */
function spawnNpm(args) {
  const options = {
    cwd: ROOT,
    stdio: "inherit",
    env: process.env,
  };
  if (IS_WINDOWS) {
    const command = [NPM, ...args].join(" ");
    return spawnSync(process.env.ComSpec || "cmd.exe", ["/d", "/s", "/c", command], options);
  }
  return spawnSync(NPM, args, options);
}

const results = [];

function runGate(gate) {
  if (gate.skip) {
    results.push({
      label: gate.label,
      display: gate.display,
      outcome: "SKIP",
      seconds: 0,
      advisory: gate.advisory,
      note: gate.skipReason,
    });
    return true;
  }

  console.log(`\n=== ${gate.label} ===`);
  console.log(`$ ${gate.display}`);

  const started = Date.now();
  const result = spawnNpm(gate.args);
  const seconds = (Date.now() - started) / 1000;
  const code = result.error ? null : result.status;
  const ok = code === 0;
  const advisory = Boolean(gate.advisory);
  const outcome = ok ? "PASS" : advisory && !strict ? "WARN" : "FAIL";

  results.push({
    label: gate.label,
    display: gate.display,
    outcome,
    seconds,
    advisory,
    note: result.error?.message,
  });

  if (!ok && advisory && !strict) {
    console.log(`[verify] advisory gate failed (${gate.label}); continuing.`);
  }
  return ok;
}

function artifactGate() {
  const label = "Package artifacts";
  const exe = path.join(ROOT, "src-tauri", "target", "release", "kudos.exe");
  const bundleDir = path.join(ROOT, "src-tauri", "target", "release", "bundle", "nsis");
  const installers = existsSync(bundleDir)
    ? readdirSync(bundleDir).filter((name) => /^Kudos_.*-setup\.exe$/i.test(name))
    : [];

  console.log(`\n=== ${label} ===`);
  if (!existsSync(exe)) {
    console.error(`[verify] portable exe missing: ${path.relative(ROOT, exe)}`);
    results.push({ label, display: "check exe + NSIS installer", outcome: "FAIL", seconds: 0 });
    return false;
  }
  if (installers.length === 0) {
    console.error(`[verify] no Kudos_*-setup.exe in ${path.relative(ROOT, bundleDir)}`);
    results.push({ label, display: "check exe + NSIS installer", outcome: "FAIL", seconds: 0 });
    return false;
  }

  const mib = (bytes) => (bytes / (1024 * 1024)).toFixed(2);
  console.log(`[verify] ${path.relative(ROOT, exe)} (${mib(statSync(exe).size)} MiB)`);
  for (const name of installers) {
    const file = path.join(bundleDir, name);
    console.log(`[verify] ${path.relative(ROOT, file)} (${mib(statSync(file).size)} MiB)`);
  }
  results.push({ label, display: "check exe + NSIS installer", outcome: "PASS", seconds: 0 });
  return true;
}

function printSummary() {
  if (results.length === 0) return;

  const header = ["Gate", "Command", "Result", "Time"];
  const rows = results.map((result) => [
    result.label,
    result.display,
    result.outcome,
    result.seconds > 0 ? `${result.seconds.toFixed(1)}s` : "-",
  ]);
  const widths = header.map((title, index) =>
    Math.max(title.length, ...rows.map((row) => row[index].length)),
  );

  const line = (cells) =>
    cells
      .map((cell, index) => (index === cells.length - 1 ? cell : cell.padEnd(widths[index])))
      .join("  ")
      .trimEnd();

  console.log("\n" + "=".repeat(widths.reduce((sum, width) => sum + width + 2, 0)));
  console.log("Kudos local verification summary");
  console.log("=".repeat(widths.reduce((sum, width) => sum + width + 2, 0)));
  console.log(line(header));
  console.log(line(widths.map((width) => "-".repeat(width))));
  for (const row of rows) console.log(line(row));
  console.log("");

  const failed = results.filter((result) => result.outcome === "FAIL");
  const warned = results.filter((result) => result.outcome === "WARN");
  const skipped = results.filter((result) => result.outcome === "SKIP");
  const passed = results.filter((result) => result.outcome === "PASS");

  console.log(`Passed: ${passed.length}   Failed: ${failed.length}   Advisory warnings: ${warned.length}   Skipped: ${skipped.length}`);
  if (warned.length > 0) {
    console.log("Advisory gates (rustfmt/clippy) are reported but not blocking. Use --strict to make them fatal.");
  }
  if (skipped.length > 0) {
    console.log("Skipped: " + skipped.map((result) => `${result.label}${result.note ? ` (${result.note})` : ""}`).join(", "));
  }
}

const gates = [
  {
    label: "Svelte + TypeScript",
    display: "npm run check",
    args: ["run", "check"],
  },
  {
    label: "Cargo check",
    display: "npm run rust:check",
    args: ["run", "rust:check"],
  },
  {
    label: "Rust formatting",
    display: "npm run rust -- fmt --all -- --check",
    args: ["run", "rust", "--", "fmt", "--all", "--", "--check"],
    advisory: true,
  },
  {
    label: "Clippy",
    display: "npm run rust -- clippy --all-targets",
    args: ["run", "rust", "--", "clippy", "--all-targets"],
    advisory: true,
  },
  {
    label: "Rust tests",
    display: "npm run rust:test",
    args: ["run", "rust:test"],
  },
  {
    label: "Frontend build",
    display: "npm run build",
    args: ["run", "build"],
  },
  {
    label: "Tauri package",
    display: "npm run tauri build",
    args: ["run", "tauri", "build"],
    skip: quick,
    skipReason: "quick mode",
  },
];

console.log("Kudos local verification");
console.log(`Repo:     ${ROOT}`);
console.log(`Version:  ${pkg.version}`);
console.log(`Node:     ${process.versions.node}`);
console.log(`Platform: ${process.platform} ${process.arch}`);
console.log(`Mode:     ${quick ? "quick (no packaging)" : "full"}${strict ? ", strict" : ""}`);

let hardFailure = false;
for (const gate of gates) {
  const ok = runGate(gate);
  if (!ok && !gate.advisory) {
    hardFailure = true;
    if (failFast) break;
  }
}

const tauriGate = results.find((result) => result.label === "Tauri package");
if (quick) {
  results.push({
    label: "Package artifacts",
    display: "check exe + NSIS installer",
    outcome: "SKIP",
    seconds: 0,
    note: "quick mode",
  });
} else if (tauriGate?.outcome === "FAIL" || hardFailure) {
  results.push({
    label: "Package artifacts",
    display: "check exe + NSIS installer",
    outcome: "SKIP",
    seconds: 0,
    note: "Tauri build did not succeed",
  });
} else {
  artifactGate();
}

printSummary();

const failed = results.filter((result) => result.outcome === "FAIL");
if (failed.length > 0) {
  console.error(`\n[verify] ${failed.length} gate(s) failed.`);
  process.exit(1);
}

console.log("\n[verify] all hard gates passed.");
