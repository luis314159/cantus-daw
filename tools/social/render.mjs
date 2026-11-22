#!/usr/bin/env node
// Renders Cantus's social-account images (docs/social/accounts.md) with headless Chrome,
// through the site's own harness (web/tools/cdp.mjs), each at its platform's size:
//
//   node tools/social/render.mjs <out-dir> [--only <file-stem>]... [--no-sheet]
//
// Writes <out-dir>/<file> for every row of OUTPUTS below (PNG, or JPEG where a platform's
// file-size cap needs it), then <out-dir>/sheet.png: every image on one page, the avatars
// with their circle crop and a 98 px preview, the YouTube banner with its safe area and the
// TV / desktop / phone crops, the 3:1 headers with the avatar's corner. Look at the sheet
// before uploading anything. On the owner's PC: node tools/social/render.mjs F:/projects/video-work/social
// The pages (social.html, sheet.html) are served from the repository's root by a server of
// this script's own (only files under the root, plus the out folder at /out/).

import { createServer } from "node:http";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";
import { launch, openTab } from "../../web/tools/cdp.mjs";

const ROOT = resolve(import.meta.dirname, "..", "..");
const MIME = { ".html": "text/html; charset=utf-8", ".css": "text/css", ".js": "text/javascript", ".mjs": "text/javascript", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".png": "image/png", ".jpg": "image/jpeg", ".webp": "image/webp", ".json": "application/json" };

// file, asset, language, CSS size is the asset's own unless w/h are given, device scale, format.
// Sizes: docs/social/accounts.md says which are on the platform's own help page.
const OUTPUTS = [
  { file: "avatar-1080.png", asset: "avatar", w: 1080, use: "master; Instagram, TikTok" },
  { file: "avatar-1000.jpg", asset: "avatar", w: 1000, fmt: "jpeg", use: "Bluesky (1 MB cap)" },
  { file: "avatar-800.png", asset: "avatar", w: 800, use: "YouTube (shown at 98 px)" },
  { file: "avatar-400.png", asset: "avatar", w: 400, use: "X, Mastodon" },
  { file: "avatar-320.png", asset: "avatar", w: 320, use: "Instagram (shown at 110-320 px)" },
  { file: "youtube-watermark.png", asset: "watermark", use: "YouTube watermark" },
  { file: "youtube-banner.jpg", asset: "banner", fmt: "jpeg", use: "YouTube banner (6 MB cap)" },
  { file: "youtube-thumb-en.png", asset: "thumb", lang: "en", use: "launch video, English" },
  { file: "youtube-thumb-es.png", asset: "thumb", lang: "es", use: "launch video, Spanish" },
  { file: "youtube-thumb-en-4k.jpg", asset: "thumb", lang: "en", scale: 3, fmt: "jpeg", use: "the same at 3840 x 2160" },
  { file: "youtube-thumb-es-4k.jpg", asset: "thumb", lang: "es", scale: 3, fmt: "jpeg", use: "the same at 3840 x 2160" },
  { file: "header-en-1500x500.png", asset: "header", lang: "en", use: "X, Mastodon header (English)" },
  { file: "header-es-1500x500.png", asset: "header", lang: "es", use: "X, Mastodon header (Spanish)" },
  { file: "bluesky-banner-en.jpg", asset: "header", lang: "en", scale: 2, fmt: "jpeg", use: "Bluesky banner (1 MB cap)" },
  { file: "bluesky-banner-es.jpg", asset: "header", lang: "es", scale: 2, fmt: "jpeg", use: "Bluesky banner (Spanish)" },
];

const args = process.argv.slice(2);
const many = (n) => args.flatMap((a, i) => (a === n ? [args[i + 1]] : []));
const outDir = resolve(args[0] ?? "social-out");
const only = many("--only");
const sheet = !args.includes("--no-sheet");
mkdirSync(outDir, { recursive: true });

const server = createServer((req, res) => {
  const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
  const [base, rel] = path.startsWith("/out/") ? [outDir, path.slice(5)] : [ROOT, path];
  const file = normalize(join(base, rel));
  if (!file.startsWith(base) || !existsSync(file) || statSync(file).isDirectory()) { res.writeHead(404); return res.end(); }
  res.writeHead(200, { "content-type": MIME[extname(file).toLowerCase()] ?? "application/octet-stream", "cache-control": "no-store" });
  res.end(readFileSync(file));
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const origin = `http://127.0.0.1:${server.address().port}`;

const browser = await launch();
const tab = await openTab(browser);
const open = async (url, width, height, scale) => {
  await tab.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: scale, mobile: false });
  const loaded = tab.once("Page.loadEventFired", 60000);
  await tab.send("Page.navigate", { url });
  await loaded;
};
const shoot = async (file, fmt) => {
  await tab.eval(() => new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok))));
  const { data } = await tab.send("Page.captureScreenshot", fmt === "jpeg" ? { format: "jpeg", quality: 92, fromSurface: true } : { format: "png", fromSurface: true });
  writeFileSync(file, Buffer.from(data, "base64"));
};
try {
  await tab.send("Emulation.setDefaultBackgroundColorOverride", { color: { r: 0, g: 0, b: 0, a: 0 } });
  await tab.send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "dark" }] });
  const rows = OUTPUTS.filter((o) => !only.length || only.some((s) => o.file.startsWith(s)));
  for (const o of rows) {
    const scale = o.scale ?? 1;
    const url = `${origin}/tools/social/social.html?asset=${o.asset}&lang=${o.lang ?? "en"}${o.w ? `&w=${o.w}` : ""}`;
    await open(url, 100, 100, scale);
    const size = await tab.eval(async () => { await window.assetReady; return window.assetSize; });
    await tab.send("Emulation.setDeviceMetricsOverride", { width: size.w, height: size.h, deviceScaleFactor: scale, mobile: false });
    await shoot(join(outDir, o.file), o.fmt);
    const kb = Math.round(statSync(join(outDir, o.file)).size / 1024);
    console.log(o.file.padEnd(26), `${size.w * scale} x ${size.h * scale}`.padEnd(12), `${kb} KB`.padStart(8), " ", o.use);
  }
  writeFileSync(join(outDir, "outputs.json"), JSON.stringify(OUTPUTS, null, 1));
  if (sheet) {
    await open(`${origin}/tools/social/sheet.html`, 2400, 1000, 1);
    const h = await tab.eval(async () => { await window.sheetReady; return document.documentElement.scrollHeight; });
    await tab.send("Emulation.setDeviceMetricsOverride", { width: 2400, height: h, deviceScaleFactor: 1, mobile: false });
    await shoot(join(outDir, "sheet.png"), "png");
    console.log("sheet.png", `2400 x ${h}`);
  }
} finally {
  await browser.close();
  server.close();
}
