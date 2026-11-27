#!/usr/bin/env node
// Two builds of the site, looking the same (round 41, the site budget): for a change meant to
// be invisible (how the page loads, not what it shows). Page by page, it compares
//   1. the style as the browser parsed it: every rule of every sheet, its text and its order
//      (@font-face rules as a set: a face may be declared later, as prerender.rs does);
//   2. full-page screenshots after the app has taken the page over, light and dark, 390 and
//      1440 px — and for the two homes also without script, the page as pre-rendered.
// A page that does not draw the same twice (pictures decoded in a different order) is retried
// up to three times; run it with the same build on both sides to see which pages do.
//
//   node web/tools/same_look.mjs <distA> <distB> [--only /,/es/,/download/]
//
// Needs Node 22+ and Chrome or Edge (CHROME_PATH). Exit 1 when anything differs.

import { resolve } from "node:path";
import { decodePng, launch, openTab, serve, visit } from "./cdp.mjs";

const args = process.argv.slice(2);
const opt = (n) => { const i = args.indexOf(n); return i >= 0 ? args[i + 1] : undefined; };
const positional = args.filter((a, i) => !a.startsWith("--") && args[i - 1] !== "--only");
if (positional.length < 2) { console.error("usage: same_look.mjs <distA> <distB> [--only /,/es/]"); process.exit(2); }
const [A, B] = positional.map((p) => resolve(p));
const pages = (opt("--only") ?? "/,/es/,/download/,/instruments/,/effects/comprimo/,/measured/,/studio/,/learn/").split(",");
const sa = await serve(A), sb = await serve(B);
const browser = await launch();
const tab = await openTab(browser);

function rules() {
  const out = [];
  const walk = (rs, pre) => {
    for (const r of rs) {
      if (r.cssRules && !(r instanceof CSSKeyframesRule)) walk(r.cssRules, pre + (r.conditionText ?? r.name ?? "") + " > ");
      else out.push(pre + r.cssText);
    }
  };
  for (const s of document.styleSheets) walk(s.cssRules, "");
  return out;
}
// down the page and back, so every lazy picture has loaded, then the whole page
async function withApp(origin, page, width, height, scheme) {
  await visit(tab, origin + page, { width, height, scheme });
  const h = await tab.eval(async () => {
    for (let y = 0; y < document.scrollingElement.scrollHeight; y += innerHeight) {
      window.scrollTo({ top: y, behavior: "instant" });
      await new Promise((r) => setTimeout(r, 100));
    }
    window.scrollTo({ top: 0, behavior: "instant" });
    await Promise.all([...document.images].map((i) => (i.complete ? null : new Promise((r) => { i.onload = i.onerror = r; }))));
    await document.fonts.ready;
    await new Promise((r) => setTimeout(r, 300));
    return document.scrollingElement.scrollHeight;
  });
  return shot(width, h);
}
// without script: nothing can be evaluated in the page, so a fixed wait
async function withoutScript(origin, page, width, height, scheme) {
  await tab.send("Emulation.setScriptExecutionDisabled", { value: true });
  await tab.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: width < 600 });
  await tab.send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }, { name: "prefers-reduced-motion", value: "reduce" }] });
  const loaded = tab.once("Page.loadEventFired", 60000);
  await tab.send("Page.navigate", { url: origin + page });
  await loaded;
  await new Promise((r) => setTimeout(r, 1500));
  const m = await tab.send("Page.getLayoutMetrics");
  const img = await shot(width, Math.ceil(m.cssContentSize.height));
  await tab.send("Emulation.setScriptExecutionDisabled", { value: false });
  return img;
}
async function shot(width, h) {
  const r = await tab.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: true, clip: { x: 0, y: 0, width, height: Math.min(h, 16000), scale: 1 } });
  return decodePng(Buffer.from(r.data, "base64"));
}
function diff(x, y) {
  if (x.width !== y.width || x.height !== y.height) return `size ${x.width}x${x.height} against ${y.width}x${y.height}`;
  const row = x.data.length / x.height, ch = row / x.width;
  let n = 0, x0 = 1e9, x1 = -1, y0 = 1e9, y1 = -1;
  for (let i = 0; i < x.data.length; i++) {
    if (x.data[i] === y.data[i]) continue;
    n++;
    const py = Math.floor(i / row), px = Math.floor((i % row) / ch);
    x0 = Math.min(x0, px); x1 = Math.max(x1, px); y0 = Math.min(y0, py); y1 = Math.max(y1, py);
  }
  return n ? `${n} bytes differ in x ${x0}-${x1}, y ${y0}-${y1}` : null;
}

let differ = 0;
try {
  for (const page of pages) {
    await visit(tab, sa.origin + page, { width: 1440, height: 900, scheme: "dark" });
    const ra = await tab.eval(rules);
    await visit(tab, sb.origin + page, { width: 1440, height: 900, scheme: "dark" });
    const rb = await tab.eval(rules);
    const face = (r) => r.startsWith("@font-face");
    const same = JSON.stringify(ra.filter((r) => !face(r))) === JSON.stringify(rb.filter((r) => !face(r)))
      && JSON.stringify(ra.filter(face).sort()) === JSON.stringify(rb.filter(face).sort());
    console.log(`${page} style: ${ra.length} and ${rb.length} rules, ${same ? "the same" : "DIFFERENT"}`);
    if (!same) {
      differ++;
      const inB = new Set(rb), inA = new Set(ra);
      for (const r of ra.filter((r) => !inB.has(r)).slice(0, 5)) console.log(`  only in A: ${r.slice(0, 200)}`);
      for (const r of rb.filter((r) => !inA.has(r)).slice(0, 5)) console.log(`  only in B: ${r.slice(0, 200)}`);
    }
    for (const scheme of ["dark", "light"]) {
      for (const width of [390, 1440]) {
        const height = width < 600 ? 844 : 900;
        const ways = page === "/" || page === "/es/" ? [withApp, withoutScript] : [withApp];
        for (const way of ways) {
          let d, tries = 0;
          do {
            d = diff(await way(sa.origin, page, width, height, scheme), await way(sb.origin, page, width, height, scheme));
          } while (d && ++tries < 4);
          if (d) differ++;
          console.log(`  ${scheme} ${width} ${way === withApp ? "with the app" : "without script"}: ${d ?? "identical"}${tries && !d ? ` (on try ${tries + 1})` : ""}`);
        }
      }
    }
  }
} finally {
  await browser.close();
  sa.server.close();
  sb.server.close();
}
console.log(differ ? `same_look: ${differ} differences` : "same_look: the same");
process.exit(differ ? 1 : 0);
