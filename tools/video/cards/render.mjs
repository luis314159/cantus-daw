#!/usr/bin/env node
// Renders the launch video's cards to transparent PNG frames with headless Chrome, through
// the site's own harness (web/tools/cdp.mjs), one frame per 1/60 s of the card's time:
//
//   node tools/video/cards/render.mjs <out-dir> [--lang en|es] [--fps 60] [--scale 2]
//        [--card <name>]... [--dur <name>=<s>]... [--still <s>] [--os preview|all]
//
// Writes <out-dir>/<lang>/<card>/00000.png … at 1920 x 1080 CSS px times --scale (3840 x
// 2160 by default), alpha kept, and <out-dir>/<lang>/<card>.json with the frame count. With
// --still, one PNG per card at that time instead (<card>-still.png), for looking at.
// The page is served from the repository's root by a server of this script's own (only
// files under the root; a missing file is a 404, never a crash).

import { createServer } from "node:http";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";
import { launch, openTab } from "../../../web/tools/cdp.mjs";

const ROOT = resolve(import.meta.dirname, "..", "..", "..");
const ALL = ["title", "count-instruments", "count-effects", "lower-record", "lower-session", "lower-plugins", "free", "end-a", "end-b"];
const MIME = { ".html": "text/html; charset=utf-8", ".css": "text/css", ".js": "text/javascript", ".mjs": "text/javascript", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".png": "image/png" };

const args = process.argv.slice(2);
const many = (n) => args.flatMap((a, i) => (a === n ? [args[i + 1]] : []));
const opt = (n, d) => { const i = args.indexOf(n); return i >= 0 ? args[i + 1] : d; };
const outDir = resolve(args[0] ?? "cards-out");
const lang = opt("--lang", "en");
const fps = Number(opt("--fps", 60));
const scale = Number(opt("--scale", 2));
const still = opt("--still", null);
const os = opt("--os", "preview"); // the end card's platforms: macOS as a preview (the owner), or all three alike
const cards = many("--card").length ? many("--card") : ALL;
const durs = Object.fromEntries(many("--dur").map((kv) => kv.split("=")).map(([k, v]) => [k, Number(v)]));

const server = createServer((req, res) => {
  const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
  const file = normalize(join(ROOT, path));
  if (!file.startsWith(ROOT) || !existsSync(file) || statSync(file).isDirectory()) { res.writeHead(404); return res.end(); }
  res.writeHead(200, { "content-type": MIME[extname(file).toLowerCase()] ?? "application/octet-stream" });
  res.end(readFileSync(file));
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const origin = `http://127.0.0.1:${server.address().port}`;

const browser = await launch();
const tab = await openTab(browser);
try {
  await tab.send("Emulation.setDeviceMetricsOverride", { width: 1920, height: 1080, deviceScaleFactor: scale, mobile: false });
  await tab.send("Emulation.setDefaultBackgroundColorOverride", { color: { r: 0, g: 0, b: 0, a: 0 } });
  await tab.send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "dark" }] });
  for (const card of cards) {
    const dur = durs[card];
    const url = `${origin}/tools/video/cards/cards.html?card=${card}&lang=${lang}&os=${os}${dur ? `&dur=${dur}` : ""}`;
    const loaded = tab.once("Page.loadEventFired", 60000);
    await tab.send("Page.navigate", { url });
    await loaded;
    const total = await tab.eval(async () => { await window.cardReady; return window.cardDuration; });
    const dir = join(outDir, lang);
    mkdirSync(dir, { recursive: true });
    const shot = async (t, file) => {
      // Two animation frames after the change, so the layout and the paint are this time's.
      await tab.eval((t) => new Promise((ok) => { window.cardSetTime(t); requestAnimationFrame(() => requestAnimationFrame(ok)); }), t);
      const { data } = await tab.send("Page.captureScreenshot", { format: "png", fromSurface: true });
      writeFileSync(file, Buffer.from(data, "base64"));
    };
    if (still !== null) {
      await shot(Number(still), join(dir, `${card}-still.png`));
      console.log(card, "still at", still);
      continue;
    }
    const frames = Math.round(total * fps);
    const cardDir = join(dir, card);
    mkdirSync(cardDir, { recursive: true });
    for (let f = 0; f < frames; f++) {
      await shot(f / fps, join(cardDir, `${String(f).padStart(5, "0")}.png`));
    }
    writeFileSync(join(dir, `${card}.json`), JSON.stringify({ card, lang, fps, frames, seconds: total, width: 1920 * scale, height: 1080 * scale }, null, 1));
    console.log(card, frames, "frames");
  }
} finally {
  await browser.close();
  server.close();
}
