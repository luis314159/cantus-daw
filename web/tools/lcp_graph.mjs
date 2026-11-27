#!/usr/bin/env node
// Lighthouse's simulated LCP, node by node (round 41, the site budget). The mobile preset does
// not time the page: it replays what the browser asked for on a modelled slow 4G and a slowed
// CPU (Lantern), and the LCP is the end of the last request or task it counts — every request
// that ended before the picture painted in the real run (in the optimistic estimate, all but
// low-priority pictures), the first parse, layout and paint, and the scripts that ran before
// it. The report shows only "render delay"; this prints the two estimates' nodes with their
// simulated start and end, which is where the time goes.
//
//   node web/tools/lcp_graph.mjs [dist] [--runs 3] [--slowdown 4] [--page /] [--quiet]
//
// The same Lighthouse as the site budget (@lhci/cli 0.15.1, fetched by npx once), the same
// static server (Lighthouse CI's, compressed) and the CPU calibrated as lhci_calibrated.mjs
// does unless --slowdown is given. Needs Chrome or Edge (CHROME_PATH chooses). Compare two
// builds back to back: the numbers move in steps of one round trip (150 ms).

import { execSync, spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { findBrowser } from "./cdp.mjs";

const LHCI_VERSION = process.env.LHCI_VERSION || "0.15.1";
const REFERENCE = 3400; // lhci_calibrated.mjs
const args = process.argv.slice(2);
const opt = (n, d) => { const i = args.indexOf(n); return i >= 0 ? args[i + 1] : d; };
const VALUED = ["--runs", "--slowdown", "--page"];
const positional = args.filter((a, i) => !a.startsWith("--") && !VALUED.includes(args[i - 1]));
const dist = resolve(positional[0] ?? join(import.meta.dirname, "..", "dist"));
const runs = Number(opt("--runs", "3"));
const page = opt("--page", "/");
const quiet = args.includes("--quiet");

// Lighthouse CI as npx keeps it: its node_modules hold the Lighthouse it runs.
const win = process.platform === "win32";
function lhciModules() {
  const cache = execSync("npm config get cache", { shell: win }).toString().trim();
  const npx = join(cache, "_npx");
  for (const d of existsSync(npx) ? readdirSync(npx) : []) {
    const pkg = join(npx, d, "node_modules", "@lhci", "cli", "package.json");
    if (existsSync(pkg) && JSON.parse(readFileSync(pkg, "utf8")).version === LHCI_VERSION) return join(npx, d, "node_modules");
  }
  return null;
}
let nm = lhciModules();
if (!nm) {
  spawnSync(win ? "npx.cmd" : "npx", ["--yes", `@lhci/cli@${LHCI_VERSION}`, "--version"], { stdio: "inherit", shell: win });
  nm = lhciModules();
}
if (!nm) { console.error(`lcp_graph: no @lhci/cli@${LHCI_VERSION} in npm's npx cache`); process.exit(2); }
const require = createRequire(join(nm, "x.js"));
const imp = (p) => import(pathToFileURL(join(nm, p)).href);
const lighthouse = (await imp("lighthouse/core/index.js")).default;
const { LanternLargestContentfulPaint } = await imp("lighthouse/core/computed/metrics/lantern-largest-contentful-paint.js");
const chromeLauncher = await imp("chrome-launcher/dist/index.js");
const Server = require("@lhci/cli/src/collect/fallback-server.js");

const server = new Server(dist, false);
await server.listen();
const url = `http://localhost:${server.port}${page}`;
const chrome = await chromeLauncher.launch({ chromePath: findBrowser(), chromeFlags: ["--headless=new", "--no-sandbox"] });

const label = (n) => {
  if (n.type === "network") {
    const r = n.request || n.record;
    return `${r.priority} ${r.resourceType} ${r.transferSize} B ${r.url.replace(/^https?:\/\/[^/]+/, "").slice(0, 70)}`;
  }
  const t = {};
  for (const e of n.childEvents) t[e.name] = (t[e.name] || 0) + (e.dur || 0) / 1000;
  const top = Object.entries(t).sort((a, b) => b[1] - a[1]).slice(0, 3).map(([k, v]) => `${k} ${v.toFixed(1)}`);
  return `CPU task, ${(n.duration / 1000).toFixed(1)} ms observed: ${top.join(", ")}`;
};

let slowdown = opt("--slowdown") ? Number(opt("--slowdown")) : null;
const lcps = [];
try {
  const total = runs + (slowdown ? 0 : 1);
  for (let i = 0; i < total; i++) {
    const calibrating = slowdown === null;
    const throttling = { rttMs: 150, throughputKbps: 1638.4, requestLatencyMs: 562.5, downloadThroughputKbps: 1474.56, uploadThroughputKbps: 675, cpuSlowdownMultiplier: slowdown ?? 4 };
    const res = await lighthouse(url, { port: chrome.port, output: "json", logLevel: "error", onlyCategories: ["performance"], throttling });
    const { lhr, artifacts } = res;
    if (calibrating) {
      const index = lhr.environment.benchmarkIndex;
      slowdown = Math.round(Math.min(8, Math.max(1, (4 * index) / REFERENCE)) * 100) / 100;
      console.log(`lcp_graph: benchmark index ${index}: CPU slowdown ${slowdown}x`);
      continue;
    }
    const a = lhr.audits;
    const m = a.metrics.details.items[0];
    const lcp = a["largest-contentful-paint"].numericValue;
    lcps.push(lcp);
    console.log(`run ${lcps.length}: LCP ${Math.round(lcp)} ms, FCP ${Math.round(a["first-contentful-paint"].numericValue)} ms, CLS ${a["cumulative-layout-shift"].numericValue.toFixed(3)}; observed LCP ${m.observedLargestContentfulPaint} ms (benchmark ${lhr.environment.benchmarkIndex})`);
    if (quiet) continue;
    const data = { trace: artifacts.Trace, devtoolsLog: artifacts.DevtoolsLog, gatherContext: artifacts.GatherContext, settings: lhr.configSettings, URL: artifacts.URL, SourceMaps: artifacts.SourceMaps, simulator: null };
    const r = await LanternLargestContentfulPaint.request(data, { computedCache: new Map(), settings: lhr.configSettings });
    for (const [name, est] of [["optimistic", r.optimisticEstimate], ["pessimistic", r.pessimisticEstimate]]) {
      console.log(`  ${name} estimate ${Math.round(est.timeInMs)} ms (simulated start - end):`);
      const rows = [...est.nodeTimings.entries()].sort((x, y) => x[1].startTime - y[1].startTime);
      for (const [n, t] of rows) console.log(`    ${String(Math.round(t.startTime)).padStart(5)} - ${String(Math.round(t.endTime)).padStart(5)}  ${label(n)}`);
    }
  }
} finally {
  await chrome.kill();
  await server.close();
}
const sorted = [...lcps].sort((x, y) => x - y);
if (sorted.length) console.log(`lcp_graph: median LCP ${Math.round(sorted[Math.floor(sorted.length / 2)])} ms over ${sorted.length} runs at ${slowdown}x`);
