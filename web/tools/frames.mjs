#!/usr/bin/env node
// Frame times while the page scrolls (round fx, docs/round-fx/web.md): the home scrolled top
// to bottom 12 px a frame, CPU slowed 4x (Lighthouse's "mid-range phone" factor), frame
// intervals from requestAnimationFrame. A comparison tool, not a pass/fail one: run it on two
// builds back to back and compare.
//
//   node tools/frames.mjs [dist] [--page /] [--width 1440] [--cpu 4] [--runs 3] [--css "rules"]
//
// --css injects rules first (to find what a cost comes from: switch one effect off).

import { join, resolve } from "node:path";
import { launch, openTab, serve, visit } from "./cdp.mjs";

const args = process.argv.slice(2);
const opt = (n, d) => { const i = args.indexOf(n); return i >= 0 ? args[i + 1] : d; };
const VALUED = ["--page", "--width", "--cpu", "--runs", "--css"];
const positional = args.filter((a, i) => !a.startsWith("--") && !VALUED.includes(args[i - 1]));
const dist = resolve(positional[0] ?? join(import.meta.dirname, "..", "dist"));
const page = opt("--page", "/");
const width = Number(opt("--width", 1440));
const cpu = Number(opt("--cpu", 4));
const runs = Number(opt("--runs", 3));

const { server, origin } = await serve(dist);
const browser = await launch();
const tab = await openTab(browser);
const all = [];
try {
  for (let r = 0; r < runs; r++) {
    await tab.send("Emulation.setCPUThrottlingRate", { rate: 1 });
    await visit(tab, origin + page, { width, height: width < 600 ? 844 : 900, scheme: "dark", reducedMotion: false });
    if (opt("--css")) await tab.eval((css) => { const s = document.createElement("style"); s.textContent = css; document.head.append(s); }, opt("--css"));
    // lazy pictures in first, so decoding is not what is measured
    await tab.eval(async () => { for (let y = 0; y < document.scrollingElement.scrollHeight; y += innerHeight) { scrollTo(0, y); await new Promise((ok) => setTimeout(ok, 60)); } scrollTo(0, 0); await new Promise((ok) => setTimeout(ok, 300)); });
    await tab.send("Emulation.setCPUThrottlingRate", { rate: cpu });
    const times = await tab.eval(() => new Promise((ok) => {
      const out = [];
      let last = performance.now();
      const max = document.scrollingElement.scrollHeight - innerHeight;
      const step = () => {
        const now = performance.now();
        out.push(now - last);
        last = now;
        if (scrollY >= max - 1 || out.length > 4000) return ok(out.slice(2));
        window.scrollBy({ top: 12, behavior: "instant" });
        requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    }));
    all.push(...times);
  }
} finally {
  await browser.close();
  server.close();
}
all.sort((a, b) => a - b);
const q = (p) => all[Math.min(all.length - 1, Math.floor(all.length * p))];
const long = all.filter((t) => t > 25).length;
console.log(`frames ${all.length}  median ${q(0.5).toFixed(1)} ms  p95 ${q(0.95).toFixed(1)} ms  p99 ${q(0.99).toFixed(1)} ms  over 25 ms ${(100 * long / all.length).toFixed(1)} %  (cpu ${cpu}x, ${width} px, ${page})`);
