#!/usr/bin/env node
// The site's motion, held to its rules (round fx, docs/round-fx/web.md):
//   1. every @keyframes moves only transform and opacity (no layout, no paint-heavy property),
//      but for one named exception measured cheap (the icons' strokes);
//   2. with `prefers-reduced-motion: reduce`, nothing is left animating once the page settles
//      — scrolled from top to bottom — and nothing is left hidden or displaced by a reveal;
//   3. without it, the motion is really there (so rule 2 is not passing on an empty page).
//
//   node tools/motion.mjs [dist] [--only /,/download/]
//
// Needs Node 22+ and Chrome or Edge (CHROME_PATH to choose one). Exit 1 on any breach.

import { join, resolve } from "node:path";
import { launch, openTab, serve, visit } from "./cdp.mjs";

const args = process.argv.slice(2);
const opt = (n) => { const i = args.indexOf(n); return i >= 0 ? args[i + 1] : undefined; };
const positional = args.filter((a, i) => !a.startsWith("--") && args[i - 1] !== "--only");
const dist = resolve(positional[0] ?? join(import.meta.dirname, "..", "dist"));
const pages = (opt("--only") ?? "/,/es/download/,/instruments/,/effects/comprimo/,/measured/,/studio/,/learn/").split(",");

// In the page: the properties every @keyframes animates, by rule name.
function keyframeProps() {
  const out = {};
  const walk = (rules) => {
    for (const r of rules) {
      if (r instanceof CSSKeyframesRule) {
        const props = new Set();
        for (const k of r.cssRules) for (let i = 0; i < k.style.length; i++) props.add(k.style[i]);
        out[r.name] = [...props];
      } else if (r.cssRules) walk(r.cssRules);
    }
  };
  for (const s of document.styleSheets) { try { walk(s.cssRules); } catch { /* another origin */ } }
  return out;
}

// In the page: scroll top to bottom, then report what still animates and what is not at rest.
async function settle() {
  const step = innerHeight * 0.8;
  for (let y = 0; y < document.scrollingElement.scrollHeight; y += step) {
    window.scrollTo({ top: y, behavior: "instant" });
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  }
  await new Promise((r) => setTimeout(r, 300));
  const running = document.getAnimations()
    .filter((a) => a.playState === "running")
    .map((a) => `${a.animationName ?? a.transitionProperty ?? "?"} on ${a.effect?.target?.className || a.effect?.target?.localName}`);
  const scrollDriven = document.getAnimations().filter((a) => a.timeline && a.timeline !== document.timeline).length;
  // a revealed element at rest: opaque and in its place
  const off = [...document.querySelectorAll(".uc-li, .m-card, .steps li, .sys-card, .stat, .songs, .song, .song-bar, .fx-list, .fx-list li, .fx-item, .studio-card, .facts li, .promises li, .flist li, .flist .ico, .foot-cols > *, .foot-bar, .foot-brand .brand img, .backdrop, .spot, main h2")]
    .filter((el) => { const cs = getComputedStyle(el); return cs.opacity !== "1" || !["none", "matrix(1, 0, 0, 1, 0, 0)"].includes(cs.transform); })
    .slice(0, 5).map((el) => `${el.localName}.${el.className}`);
  return { running, scrollDriven, off };
}

const { server, origin } = await serve(dist);
const browser = await launch();
const tab = await openTab(browser);
const breaches = [];
try {
  await visit(tab, `${origin}/`, { width: 1440, height: 900, scheme: "dark", reducedMotion: false });
  const kf = await tab.eval(keyframeProps);
  const allowed = new Set(["transform", "opacity", "animation-timing-function"]);
  // One exception, measured cheap with tools/frames.mjs (docs/round-fx/web.md §3d): the icons'
  // strokes draw on 24-px SVGs.
  const measured = { stroke: ["stroke-dashoffset"] };
  for (const [name, props] of Object.entries(kf)) {
    const bad = props.filter((p) => !allowed.has(p) && !(measured[name] ?? []).includes(p));
    if (bad.length) breaches.push(`@keyframes ${name} animates ${bad.join(", ")}`);
  }
  console.log(`keyframes: ${Object.keys(kf).length} rules, ${Object.keys(kf).join(" ")}`);
  for (const page of pages) {
    for (const [reduced, width] of [[true, 1440], [false, 1440], [true, 390], [false, 390]]) {
      await visit(tab, origin + page, { width, height: width < 600 ? 844 : 900, scheme: "light", reducedMotion: reduced });
      const r = await tab.eval(settle);
      if (reduced) {
        if (r.running.length) breaches.push(`${page} ${width} (reduced motion): still animating — ${r.running.slice(0, 4).join("; ")}`);
        if (r.scrollDriven) breaches.push(`${page} ${width} (reduced motion): ${r.scrollDriven} scroll-driven animations`);
        if (r.off.length) breaches.push(`${page} ${width} (reduced motion): not at rest — ${r.off.join("; ")}`);
      } else if (r.off.length) {
        // with motion, once the whole page has been scrolled past, every reveal has finished
        breaches.push(`${page} ${width} (motion): left half-shown — ${r.off.join("; ")}`);
      } else if (r.scrollDriven === 0 && r.running.length === 0) {
        breaches.push(`${page} (motion allowed): no animation at all — is motion.css loaded?`);
      }
      console.log(`${page.padEnd(16)} ${String(width).padEnd(5)}${reduced ? "reduce   " : "motion ok"}  running ${r.running.length}  scroll-driven ${r.scrollDriven}  not at rest ${r.off.length}${process.env.MOTION_DEBUG ? ` ${r.off.join(" | ")}` : ""}`);
    }
  }
} finally {
  await browser.close();
  server.close();
}
for (const b of breaches) console.log(`BREACH ${b}`);
console.log(breaches.length ? `motion: ${breaches.length} breaches` : "motion: transform and opacity only; reduced motion leaves the page still and at rest");
process.exit(breaches.length ? 1 : 0);
