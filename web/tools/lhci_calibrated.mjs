#!/usr/bin/env node
// Lighthouse CI with the CPU calibrated to the machine that measures (docs/RELEASING.md L3:
// a timing gate compares with a reference, never a fixed number on a machine of unknown speed).
//
//   node web/tools/lhci_calibrated.mjs <lighthouserc.json> [extra lhci autorun flags…]
//
// Lighthouse's mobile preset emulates a mid-range phone by slowing the host's CPU 4×. On a
// slower host the "phone" is slower too: the site budget's LCP read 1 884 ms on a GitHub runner
// whose benchmark index was 3 466 and 2 106 ms — over the 2 000 ms budget — on one at 2 786,
// for the same page (cantus-daw runs 44820803288 and 44982501210, 2026-11-20/22). So:
//
//   1. one calibration run (`lhci collect -n 1`) reads the host's `benchmarkIndex`;
//   2. the slowdown becomes 4 × index / REFERENCE (clamped to 1…8): the same phone on any host;
//   3. `lhci autorun` measures with it, and every report is checked to have used it.
//
// REFERENCE is the benchmark index of the runners the 2 s budget was set and held on (round 39
// to beta.16: 3 100–3 500). Run from the folder Lighthouse CI should keep `.lighthouseci` in.
// Exit code: autorun's, or 2 when the calibration or the check fails.

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";

const REFERENCE = 3400;
const BASE = 4;
const LHCI = `@lhci/cli@${process.env.LHCI_VERSION || "0.15.1"}`;

const [rc, ...extra] = process.argv.slice(2);
if (!rc) {
  console.error("usage: lhci_calibrated.mjs <lighthouserc.json> [extra lhci flags…]");
  process.exit(2);
}
const config = `--config=${resolve(rc)}`;
const npx = process.platform === "win32" ? "npx.cmd" : "npx";
const run = (args) =>
  spawnSync(npx, ["--yes", LHCI, ...args], { stdio: "inherit", shell: process.platform === "win32" });

// The flags autorun takes that collect takes too (the dist folder, Chrome's flags).
const collectFlags = extra.filter((a) => a.startsWith("--collect."));

// 1. Calibration.
const work = resolve(".lighthouseci");
rmSync(work, { recursive: true, force: true });
const cal = run(["collect", config, "--numberOfRuns=1", ...collectFlags.map((a) => a.replace("--collect.", "--"))]);
if (cal.status !== 0) {
  console.error("lhci_calibrated: the calibration run failed");
  process.exit(2);
}
const lhrs = existsSync(work) ? readdirSync(work).filter((f) => /^lhr-.*\.json$/.test(f)) : [];
if (lhrs.length === 0) {
  console.error("lhci_calibrated: the calibration run left no report");
  process.exit(2);
}
const index = JSON.parse(readFileSync(join(work, lhrs[0]), "utf8")).environment?.benchmarkIndex;
if (!(index > 0)) {
  console.error("lhci_calibrated: no benchmarkIndex in the calibration report");
  process.exit(2);
}
const slowdown = Math.round(Math.min(8, Math.max(1, (BASE * index) / REFERENCE)) * 100) / 100;
console.log(
  `lhci_calibrated: host benchmark index ${index}, reference ${REFERENCE}: CPU slowdown ${slowdown}× (${BASE}× on the reference host)`
);
rmSync(work, { recursive: true, force: true });

// 2. The measurement.
const flag = `--collect.settings.throttling.cpuSlowdownMultiplier=${slowdown}`;
const res = run(["autorun", config, flag, ...extra]);

// 3. Every report used the calibrated slowdown (else the gate measured something else).
const used = existsSync(work)
  ? readdirSync(work)
      .filter((f) => /^lhr-.*\.json$/.test(f))
      .map((f) => JSON.parse(readFileSync(join(work, f), "utf8")).configSettings?.throttling?.cpuSlowdownMultiplier)
  : [];
if (used.length === 0 || used.some((m) => Math.abs(m - slowdown) > 1e-9)) {
  console.error(`lhci_calibrated: the reports used ${JSON.stringify(used)}, not ${slowdown}`);
  process.exit(2);
}
console.log(`lhci_calibrated: ${used.length} runs at ${slowdown}×`);
process.exit(res.status ?? 2);
