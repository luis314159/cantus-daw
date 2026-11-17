#!/usr/bin/env node
// Pictures of the built site (round fx): each page's first screen and the whole page, light
// and dark, 1440 and 390 px wide, from headless Chrome or Edge.
//
//   node tools/shoot.mjs <out-dir> [dist] [--only /,/download/,/es/,/es/download/] [--full]
//        [--motion] [--frames /,12,90]
//
// Names are <page>-<scheme>-<width>.png (the first screen) and …-full.png with --full.
// --motion lets the page animate (reduced motion is the default, so pictures are stable).
// --el <selector> writes only that element (scrolled into view, 24 px around it): <name>-el.png.
// --frames <page>,<count>,<ms> writes a sequence of the first screen, one every <ms>, to show
// the animations, scrolling a little on each frame after the first third.

import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { launch, openTab, serve, visit } from "./cdp.mjs";

const args = process.argv.slice(2);
const opt = (n) => { const i = args.indexOf(n); return i >= 0 ? args[i + 1] : undefined; };
const VALUED = ["--only", "--frames", "--schemes", "--widths", "--el"];
const positional = args.filter((a, i) => !a.startsWith("--") && !VALUED.includes(args[i - 1]));
const out = resolve(positional[0] ?? "shots");
const dist = resolve(positional[1] ?? join(import.meta.dirname, "..", "dist"));
const pages = (opt("--only") ?? "/,/download/,/es/,/es/download/").split(",");
const schemes = (opt("--schemes") ?? "light,dark").split(",");
const widths = (opt("--widths") ?? "1440,390").split(",").map(Number);
const height = (w) => (w < 600 ? 844 : 900);
const name = (p) => (p === "/" ? "home" : p.replace(/^\/|\/$/g, "").replace(/\//g, "-").replace(/^es$/, "es-home"));
mkdirSync(out, { recursive: true });

const { server, origin } = await serve(dist);
const browser = await launch();
const tab = await openTab(browser);
const png = (r) => Buffer.from(r.data, "base64");
try {
  const frames = opt("--frames");
  if (frames) {
    const [page, count, ms] = frames.split(",");
    for (const scheme of schemes) {
      for (const width of widths) {
        await visit(tab, origin + page, { width, height: height(width), scheme, reducedMotion: false });
        const n = Number(count);
        for (let i = 0; i < n; i++) {
          if (i > n / 3) await tab.eval((dy) => window.scrollBy({ top: dy, behavior: "instant" }), Math.round(height(width) / 10));
          await tab.eval((t) => new Promise((ok) => setTimeout(ok, t)), Number(ms));
          const shot = await tab.send("Page.captureScreenshot", { format: "png" });
          writeFileSync(join(out, `${name(page)}-${scheme}-${width}-f${String(i).padStart(3, "0")}.png`), png(shot));
        }
      }
    }
  } else {
    for (const page of pages) {
      for (const scheme of schemes) {
        for (const width of widths) {
          await visit(tab, origin + page, { width, height: height(width), scheme, reducedMotion: !args.includes("--motion") });
          const base = join(out, `${name(page)}-${scheme}-${width}`);
          if (opt("--el")) {
            const box = await tab.eval((sel) => new Promise((ok) => {
              const el = document.querySelector(sel);
              if (!el) return ok(null);
              el.scrollIntoView({ block: "center", behavior: "instant" });
              setTimeout(() => { const r = el.getBoundingClientRect(); ok({ x: r.left + scrollX, y: r.top + scrollY, w: r.width, h: r.height }); }, 300);
            }), opt("--el"));
            if (!box) { console.log(`${base}: no ${opt("--el")}`); continue; }
            const clip = { x: Math.max(0, box.x - 24), y: Math.max(0, box.y - 24), width: Math.min(width, box.w + 48), height: box.h + 48, scale: 1 };
            writeFileSync(`${base}-el.png`, png(await tab.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: true, clip })));
            console.log(`${base}-el`);
            continue;
          }
          writeFileSync(`${base}.png`, png(await tab.send("Page.captureScreenshot", { format: "png" })));
          if (args.includes("--full")) {
            // down the page and back, so every lazy picture has loaded
            const h = await tab.eval(async () => {
              for (let y = 0; y < document.scrollingElement.scrollHeight; y += innerHeight) {
                window.scrollTo({ top: y, behavior: "instant" });
                await new Promise((r) => setTimeout(r, 120));
              }
              window.scrollTo({ top: 0, behavior: "instant" });
              await Promise.all([...document.images].map((i) => (i.complete ? null : new Promise((r) => { i.onload = i.onerror = r; }))));
              await new Promise((r) => setTimeout(r, 200));
              return document.scrollingElement.scrollHeight;
            });
            const shot = await tab.send("Page.captureScreenshot", {
              format: "png", captureBeyondViewport: true, clip: { x: 0, y: 0, width, height: Math.min(h, 16000), scale: 1 },
            });
            writeFileSync(`${base}-full.png`, png(shot));
          }
          console.log(base);
        }
      }
    }
  }
} finally {
  await browser.close();
  server.close();
}
