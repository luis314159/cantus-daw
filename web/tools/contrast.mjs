#!/usr/bin/env node
// The site's text contrast, measured on the rendered pages (round fx, docs/round-fx/web.md).
//
//   node tools/contrast.mjs [dist] [--quick] [--only /download/,/es/] [--json out.json] [--jobs 3]
//
// Serves dist/ (default web/dist), opens every pre-rendered page in headless Chrome or Edge in
// the light and the dark colour scheme, 1440 and 390 px wide, scrolls it a screen at a time
// and, for every visible text node, compares its colour with the pixels behind it: the screen
// is captured once with the glyphs made transparent (-webkit-text-fill-color, which leaves
// currentColor, borders and icons as they are), and the text's colour, alpha and opacity are
// composited over each pixel sampled under its line boxes. A line passes when 90 % of the
// samples reach WCAG AA: 4.5:1, or 3:1 for large text (24 px, or 18.66 px at weight 700).
// Text under aria-hidden or in a disabled control is decorative and reported apart; text
// covered by something else (the fixed bar) is skipped. Measured with reduced motion, so
// nothing is mid-animation. Exit 1 when any line fails. Needs Node 22+ and Chrome or Edge
// (CHROME_PATH to choose one); no npm package.

import { readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { decodePng, launch, openTab, serve, visit } from "./cdp.mjs";

const args = process.argv.slice(2);
const flag = (n) => args.includes(n);
const opt = (n) => { const i = args.indexOf(n); return i >= 0 ? args[i + 1] : undefined; };
const VALUED = ["--only", "--json", "--schemes", "--widths", "--jobs"];
const positional = args.filter((a, i) => !a.startsWith("--") && !VALUED.includes(args[i - 1]));
const dist = resolve(positional[0] ?? join(import.meta.dirname, "..", "dist"));

/** Every pre-rendered page (`index.html` under dist), the manual, the press kit and the 404. */
function allPages() {
  const out = [];
  const walk = (dir, url) => {
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      if (e.isDirectory()) walk(join(dir, e.name), `${url}${e.name}/`);
      else if (e.name === "index.html") out.push(url);
      else if (/^(en|es)\.html$/.test(e.name) && url.endsWith("/manual/")) out.push(`${url}${e.name}`);
    }
  };
  walk(dist, "/");
  out.push("/404.html");
  return out.sort();
}

/** One page of each kind, both languages: what CI measures. */
function quickPages(all) {
  const kinds = new Map();
  for (const p of all) {
    const tail = p.replace(/^\/es\//, "/");
    const kind = tail.replace(/^\/(instruments|effects)\/[^/]+\/$/, "/$1/*/");
    const key = `${p.startsWith("/es/") ? "es" : "en"}:${kind}`;
    if (!kinds.has(key)) kinds.set(key, p);
  }
  return [...kinds.values()].sort();
}

const all = allPages();
let pages = flag("--quick") ? quickPages(all) : all;
if (opt("--only")) pages = opt("--only").split(",");
const schemes = (opt("--schemes") ?? "light,dark").split(",");
const widths = (opt("--widths") ?? "1440,390").split(",").map(Number);
const height = (w) => (w < 600 ? 844 : 900);

// In the page: every visible text node in the viewport, grouped by its element.
function collect() {
  const ids = (window.__cid ??= { map: new WeakMap(), n: 0 });
  const parse = (c) => {
    const m = c.match(/rgba?\(([^)]+)\)/);
    if (!m) return null;
    const v = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
    return [v[0], v[1], v[2], v.length > 3 ? v[3] : 1];
  };
  const path = (el) => {
    const parts = [];
    for (let e = el; e && e !== document.body && parts.length < 4; e = e.parentElement) {
      const cls = [...e.classList].slice(0, 2).map((c) => `.${c}`).join("");
      parts.unshift(`${e.localName}${e.id ? `#${e.id}` : ""}${cls}`);
    }
    return parts.join(" > ");
  };
  const W = innerWidth, H = innerHeight;
  const ops = new Map();
  const opacity = (e) => {
    if (!e) return 1;
    if (!ops.has(e)) ops.set(e, parseFloat(getComputedStyle(e).opacity) * opacity(e.parentElement));
    return ops.get(e);
  };
  const byEl = new Map();
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let t = walker.nextNode(); t; t = walker.nextNode()) {
    if (!t.data.trim()) continue;
    const el = t.parentElement;
    if (!el) continue;
    const box = el.getBoundingClientRect();
    if (box.bottom <= 0 || box.top >= H || box.width === 0) continue;
    if (el.closest("script, style, noscript, template, title")) continue;
    const cs = getComputedStyle(el);
    if (cs.visibility !== "visible" || cs.display === "none") continue;
    const op = opacity(el);
    if (op < 0.05) continue;
    const range = document.createRange();
    range.selectNodeContents(t);
    const rects = [...range.getClientRects()].filter((r) =>
      r.width >= 2 && r.height >= 6 && r.top >= 0 && r.bottom <= H && r.left >= 0 && r.right <= W);
    // Wholly visible: the middle and the four ends of the line box all hit this element (not
    // the sticky bar over it, a dialog, or a scroller's edge).
    const mine = (x, y) => { const hit = document.elementFromPoint(x, y); return hit && (hit === el || el.contains(hit) || hit.contains(el)); };
    const seen = rects.filter((r) => {
      const [cx, cy, x0, x1, y0, y1] = [r.left + r.width / 2, r.top + r.height / 2, r.left + 1, r.right - 1, r.top + 1, r.bottom - 1];
      return mine(cx, cy) && mine(x0, y0) && mine(x1, y0) && mine(x0, y1) && mine(x1, y1);
    });
    if (!seen.length) continue;
    const color = parse(cs.color);
    if (!color) continue;
    let id = ids.map.get(el);
    if (id === undefined) { id = ids.n++; ids.map.set(el, id); }
    const size = parseFloat(cs.fontSize), weight = parseInt(cs.fontWeight, 10);
    const item = byEl.get(el) ?? {
      id, sel: path(el), text: "", color, op, size, weight,
      large: size >= 24 || (size >= 18.66 && weight >= 700),
      deco: !!el.closest('[aria-hidden="true"], :disabled, [aria-disabled="true"]'),
      rects: [],
    };
    item.text = `${item.text} ${t.data.trim()}`.trim().slice(0, 70);
    for (const r of seen) item.rects.push([r.left, r.top, r.width, r.height]);
    byEl.set(el, item);
  }
  return [...byEl.values()];
}

const HIDE = `*, *::before, *::after, ::marker, ::placeholder { -webkit-text-fill-color: transparent !important;
  text-shadow: none !important; text-decoration-color: transparent !important; -webkit-text-stroke: 0 !important; }`;

const lum = (r, g, b) => {
  const f = (v) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
};
const ratio = (a, b) => { const [x, y] = [lum(...a), lum(...b)]; return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
const hex = (c) => `#${c.slice(0, 3).map((v) => Math.round(v).toString(16).padStart(2, "0")).join("")}`;

/** The item's contrast against the captured background: the 10th percentile of its samples. */
function measure(item, img) {
  const [r, g, b, a] = item.color;
  const alpha = a * item.op;
  const got = [];
  let bgSum = [0, 0, 0];
  for (const [x, y, w, h] of item.rects) {
    for (let i = 0; i < 9; i++) {
      for (let j = 0; j < 3; j++) {
        const px = Math.min(img.width - 1, Math.floor(x + 1 + ((w - 2) * (i + 0.5)) / 9));
        const py = Math.min(img.height - 1, Math.floor(y + 2 + ((h - 4) * (j + 0.5)) / 3));
        const o = (py * img.width + px) * 4;
        const bg = [img.data[o], img.data[o + 1], img.data[o + 2]];
        const fg = [r * alpha + bg[0] * (1 - alpha), g * alpha + bg[1] * (1 - alpha), b * alpha + bg[2] * (1 - alpha)];
        got.push({ v: ratio(fg, bg), bg });
        bgSum = bgSum.map((s, k) => s + bg[k]);
      }
    }
  }
  got.sort((p, q) => p.v - q.v);
  const pick = got[Math.floor(got.length * 0.1)];
  return { ratio: pick.v, bg: hex(pick.bg), fg: hex([r, g, b]) + (alpha < 1 ? `@${alpha.toFixed(2)}` : "") };
}

async function measurePage(tab, origin, page, scheme, width) {
  await visit(tab, origin + page, { width, height: height(width), scheme });
  const total = await tab.eval(() => document.scrollingElement.scrollHeight);
  const vh = height(width);
  const worst = new Map();
  for (let y = 0; y < total; y += vh - 160) {
    await tab.eval((top) => new Promise((ok) => {
      window.scrollTo({ top, behavior: "instant" });
      requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(ok, 80)));
    }), y);
    const items = await tab.eval(collect);
    await tab.eval((css) => {
      const s = document.createElement("style");
      s.id = "__hide"; s.textContent = css; document.head.append(s);
      return new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
    }, HIDE);
    const shot = await tab.send("Page.captureScreenshot", { format: "png" });
    await tab.eval(() => document.getElementById("__hide").remove());
    const img = decodePng(Buffer.from(shot.data, "base64"));
    for (const it of items) {
      const m = measure(it, img);
      const prev = worst.get(it.id);
      if (!prev || m.ratio < prev.ratio) worst.set(it.id, { ...m, sel: it.sel, text: it.text, large: it.large, deco: it.deco, size: it.size });
    }
    if (y + vh >= total) break;
  }
  const items = [...worst.values()];
  const need = (it) => (it.large ? 3 : 4.5);
  const real = items.filter((it) => !it.deco);
  return {
    page, scheme, width, measured: real.length,
    min: real.reduce((m, it) => Math.min(m, it.ratio), Infinity),
    fails: real.filter((it) => it.ratio < need(it)).sort((p, q) => p.ratio - q.ratio),
    decorative: items.filter((it) => it.deco && it.ratio < need(it)).length,
  };
}

const { server, origin } = await serve(dist);
const browser = await launch();
const views = pages.flatMap((page) => schemes.flatMap((scheme) => widths.map((width) => ({ page, scheme, width }))));
const results = new Array(views.length);
const started = Date.now();
const report = (r) => {
  const tag = `${r.page} ${r.scheme} ${r.width}`.padEnd(46);
  const lines = [`${tag} ${String(r.measured).padStart(4)} texts  min ${r.min === Infinity ? "-" : r.min.toFixed(2)}${r.fails.length ? `  FAIL ${r.fails.length}` : ""}`];
  for (const f of r.fails.slice(0, 12)) {
    lines.push(`    ${f.ratio.toFixed(2)} < ${f.large ? 3 : 4.5}  ${f.fg} on ${f.bg}  ${f.size}px  ${f.sel}  "${f.text}"`);
  }
  if (r.fails.length > 12) lines.push(`    … and ${r.fails.length - 12} more`);
  console.log(lines.join("\n"));
};
try {
  // A few tabs at once (--jobs, default 3), each taking the next view; reported in order of finishing.
  let next = 0;
  const worker = async () => {
    const tab = await openTab(browser);
    while (next < views.length) {
      const i = next++;
      const v = views[i];
      results[i] = await measurePage(tab, origin, v.page, v.scheme, v.width);
      report(results[i]);
    }
  };
  await Promise.all(Array.from({ length: Number(opt("--jobs") ?? 3) }, worker));
} finally {
  await browser.close();
  server.close();
}
const failed = results.reduce((n, r) => n + r.fails.length, 0);
if (opt("--json")) writeFileSync(opt("--json"), JSON.stringify(results, null, 1));
const secs = ((Date.now() - started) / 1000).toFixed(0);
console.log(`contrast: ${results.length} page views in ${secs} s; ${failed ? `${failed} texts below AA` : "every text at AA or better"}`);
process.exit(failed ? 1 : 0);
