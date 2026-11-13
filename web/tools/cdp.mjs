// A small headless-browser harness for the site's own checks (contrast.mjs, shoot.mjs): a
// static server for web/dist, Chrome or Edge launched headless, and the DevTools protocol
// spoken over Node's built-in WebSocket — no npm package (Node 22 or newer).

import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { extname, join, normalize, resolve, sep } from "node:path";
import { inflateSync } from "node:zlib";

const MIME = {
  ".html": "text/html; charset=utf-8", ".css": "text/css", ".js": "text/javascript",
  ".mjs": "text/javascript", ".wasm": "application/wasm", ".json": "application/json",
  ".svg": "image/svg+xml", ".png": "image/png", ".webp": "image/webp", ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg", ".gif": "image/gif", ".woff2": "font/woff2", ".mp3": "audio/mpeg",
  ".xml": "application/xml", ".txt": "text/plain; charset=utf-8", ".ico": "image/x-icon",
};

/** Serves `dist` as the host does: `/x/` is `/x/index.html`, anything else missing is the 404 page. */
export function serve(dist) {
  const root = resolve(dist);
  const server = createServer((req, res) => {
    let path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    let file = normalize(join(root, path));
    if (!file.startsWith(root)) { res.writeHead(403); return res.end(); }
    if (existsSync(file) && statSync(file).isDirectory()) file = join(file, "index.html");
    let status = 200;
    if (!existsSync(file)) { file = join(root, "404.html"); status = 404; }
    res.writeHead(status, { "content-type": MIME[extname(file).toLowerCase()] ?? "application/octet-stream" });
    res.end(readFileSync(file));
  });
  return new Promise((ok) => server.listen(0, "127.0.0.1", () => ok({ server, origin: `http://127.0.0.1:${server.address().port}` })));
}

/** Chrome or Edge: $CHROME_PATH, else the usual places on Windows, Linux and macOS. */
export function findBrowser() {
  const env = process.env.CHROME_PATH;
  const pf = process.env.ProgramFiles ?? "C:\\Program Files";
  const pf86 = process.env["ProgramFiles(x86)"] ?? "C:\\Program Files (x86)";
  const all = [
    env,
    `${pf}\\Google\\Chrome\\Application\\chrome.exe`, `${pf86}\\Google\\Chrome\\Application\\chrome.exe`,
    `${pf86}\\Microsoft\\Edge\\Application\\msedge.exe`, `${pf}\\Microsoft\\Edge\\Application\\msedge.exe`,
    "/usr/bin/google-chrome", "/usr/bin/google-chrome-stable", "/usr/bin/chromium", "/usr/bin/chromium-browser",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  ];
  const found = all.find((p) => p && existsSync(p));
  if (!found) throw new Error("no Chrome or Edge found: set CHROME_PATH");
  return found;
}

/** A headless browser and one DevTools connection to it. */
export async function launch(browserPath = findBrowser()) {
  const profile = mkdtempSync(join(tmpdir(), "cantus-cdp-"));
  const proc = spawn(browserPath, [
    "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
    "--no-first-run", "--no-default-browser-check", "--disable-extensions", "--hide-scrollbars",
    "--mute-audio", "--disable-background-timer-throttling", "--disable-renderer-backgrounding",
    "--force-color-profile=srgb", "--font-render-hinting=none", "--lang=en-US",
    ...(process.platform === "linux" ? ["--no-sandbox"] : []),
    "about:blank",
  ], { stdio: ["ignore", "ignore", "pipe"] });
  const wsUrl = await new Promise((ok, fail) => {
    let buf = "";
    const t = setTimeout(() => fail(new Error("the browser did not start")), 30000);
    proc.stderr.on("data", (d) => {
      buf += d;
      const m = buf.match(/DevTools listening on (ws:\/\/\S+)/);
      if (m) { clearTimeout(t); ok(m[1]); }
    });
    proc.on("exit", (c) => fail(new Error(`the browser exited (${c})`)));
  });
  const ws = new WebSocket(wsUrl);
  await new Promise((ok, fail) => { ws.onopen = ok; ws.onerror = fail; });
  let next = 1;
  const waiting = new Map();
  const listeners = [];
  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.id && waiting.has(msg.id)) {
      const { ok, fail, method } = waiting.get(msg.id);
      waiting.delete(msg.id);
      msg.error ? fail(new Error(`${method}: ${msg.error.message}`)) : ok(msg.result);
    } else if (msg.method) {
      for (const l of listeners) l(msg);
    }
  };
  const send = (method, params = {}, sessionId) => new Promise((ok, fail) => {
    const id = next++;
    waiting.set(id, { ok, fail, method });
    ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
  });
  const close = async () => {
    try { await send("Browser.close"); } catch { /* already gone */ }
    await new Promise((ok) => { if (proc.exitCode !== null) ok(); else { proc.on("exit", ok); setTimeout(ok, 5000); } });
    try { rmSync(profile, { recursive: true, force: true }); } catch { /* the OS holds it a moment */ }
  };
  return { send, listeners, close };
}

/** A tab: `send` within its session, `once(event)` waits for one event, `eval` runs a function. */
export async function openTab(b) {
  // Each tab in a window of its own: a tab behind another is hidden, and a hidden page never
  // draws a frame (requestAnimationFrame waits for ever).
  const { targetId } = await b.send("Target.createTarget", { url: "about:blank", newWindow: true });
  const { sessionId } = await b.send("Target.attachToTarget", { targetId, flatten: true });
  const send = (m, p) => b.send(m, p, sessionId);
  const once = (method, ms = 30000) => new Promise((ok, fail) => {
    const t = setTimeout(() => { drop(); fail(new Error(`timed out waiting for ${method}`)); }, ms);
    const l = (msg) => { if (msg.sessionId === sessionId && msg.method === method) { clearTimeout(t); drop(); ok(msg.params); } };
    const drop = () => { const i = b.listeners.indexOf(l); if (i >= 0) b.listeners.splice(i, 1); };
    b.listeners.push(l);
  });
  await send("Page.enable");
  await send("Runtime.enable");
  const evaluate = async (fn, ...args) => {
    const r = await send("Runtime.evaluate", {
      expression: `(${fn})(...${JSON.stringify(args)})`, awaitPromise: true, returnByValue: true,
    });
    if (r.exceptionDetails) throw new Error(`in the page: ${r.exceptionDetails.exception?.description ?? r.exceptionDetails.text}`);
    return r.result.value;
  };
  return { send, once, eval: evaluate, close: () => b.send("Target.closeTarget", { targetId }) };
}

/** Sets the viewport and the colour scheme / motion the page sees, then opens `url` and waits
 *  for the app to take the page over (`html.app-on`) and the fonts to load. */
export async function visit(tab, url, { width, height, scheme, reducedMotion = true }) {
  await tab.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: width < 600 });
  await tab.send("Emulation.setEmulatedMedia", { features: [
    { name: "prefers-color-scheme", value: scheme },
    { name: "prefers-reduced-motion", value: reducedMotion ? "reduce" : "no-preference" },
  ] });
  const loaded = tab.once("Page.loadEventFired", 60000);
  await tab.send("Page.navigate", { url });
  await loaded;
  await tab.eval(async () => {
    const until = Date.now() + 15000;
    // The 404 and the manual are static pages: no app takes them over.
    const app = !!document.getElementById("app");
    while (app && !document.documentElement.classList.contains("app-on") && Date.now() < until) {
      await new Promise((r) => setTimeout(r, 50));
    }
    await document.fonts.ready;
    await new Promise((r) => setTimeout(r, 150));
  });
}

/** Decodes an 8-bit, non-interlaced RGB or RGBA PNG (what the browser's screenshots are). */
export function decodePng(buf) {
  let pos = 8, width = 0, height = 0, type = 0;
  const idat = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const kind = buf.toString("ascii", pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (kind === "IHDR") {
      width = data.readUInt32BE(0); height = data.readUInt32BE(4);
      if (data[8] !== 8 || data[12] !== 0) throw new Error("only 8-bit non-interlaced PNGs");
      type = data[9];
    } else if (kind === "IDAT") idat.push(data);
    else if (kind === "IEND") break;
    pos += 12 + len;
  }
  const bpp = type === 6 ? 4 : type === 2 ? 3 : 0;
  if (!bpp) throw new Error(`PNG colour type ${type} not handled`);
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * bpp;
  const out = Buffer.alloc(width * height * 4);
  let prev = Buffer.alloc(stride);
  for (let y = 0; y < height; y++) {
    const f = raw[y * (stride + 1)];
    const line = Buffer.from(raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1)));
    for (let i = 0; i < stride; i++) {
      const a = i >= bpp ? line[i - bpp] : 0, b = prev[i], c = i >= bpp ? prev[i - bpp] : 0;
      let v = line[i];
      if (f === 1) v += a;
      else if (f === 2) v += b;
      else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) { const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c); v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c; }
      line[i] = v & 255;
    }
    for (let x = 0; x < width; x++) {
      const o = (y * width + x) * 4, s = x * bpp;
      out[o] = line[s]; out[o + 1] = line[s + 1]; out[o + 2] = line[s + 2]; out[o + 3] = bpp === 4 ? line[s + 3] : 255;
    }
    prev = line;
  }
  return { width, height, data: out };
}
