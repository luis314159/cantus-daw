// The listening kits' page, driven in real browsers: every control clicked or keyed as a
// listener would, and the page's state and audio checked after each (Chorda's kit 5, round 40;
// any subject since round 42 — Tympanum's kit).
//
//   PW_DIR=<folder with node_modules/playwright-core> node abx_browser_check.mjs <kit dir> [--mock <abx.html> <subject.json>]
//
// `<kit dir>` is a generated kit (`chorda_abx`, `tympanum_abx`), or with `--mock` a folder to
// build a stand-in kit into (short tones, every item of the subject, the real template). The
// counts (trials, questions), the form's instrument question, the sources' names and the
// credits are read from the kit's own `SUBJECT`. Runs Edge and Chrome (the
// installed ones) and Firefox (Playwright's), each over http (Web Audio) and file:// (<audio>),
// plus the published page's online mode against a stub of `window.claude` (Edge).
import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
import http from "node:http";
import { pathToFileURL } from "node:url";

const require = createRequire(path.join(process.env.PW_DIR || ".", "x.js"));
const { chromium, firefox } = require("playwright-core");
const args = process.argv.slice(2);
const kit = path.resolve(args[0]);
const only = process.env.ABX_ONLY || "";
process.on("unhandledRejection", e => { console.log("unhandled: " + String(e).split("\n")[0]); });
let failures = 0, passes = 0;
const log = [];
function ok(cond, what) { if (cond) { passes++; log.push("  pass " + what); } else { failures++; log.push("  FAIL " + what); console.log("FAIL " + what); } }

// ---- A stand-in kit: the real page, short tones.
function veil(text, seed) {
 let x = seed >>> 0; const b = Buffer.from(text, "utf8");
 for (let i = 0; i < b.length; i++) { x = (x ^ (x << 13)) >>> 0; x = (x ^ (x >>> 17)) >>> 0; x = (x ^ (x << 5)) >>> 0; b[i] ^= x >>> 24; }
 return b.toString("base64");
}
function wav(freq, secs) {
 const sr = 48000, n = Math.round(sr * secs), d = Buffer.alloc(44 + n * 4);
 d.write("RIFF", 0); d.writeUInt32LE(36 + n * 4, 4); d.write("WAVEfmt ", 8); d.writeUInt32LE(16, 16); d.writeUInt16LE(1, 20); d.writeUInt16LE(2, 22);
 d.writeUInt32LE(sr, 24); d.writeUInt32LE(sr * 4, 28); d.writeUInt16LE(4, 32); d.writeUInt16LE(16, 34); d.write("data", 36); d.writeUInt32LE(n * 4, 40);
 for (let i = 0; i < n; i++) { const v = Math.round(8000 * Math.sin(2 * Math.PI * freq * i / sr)); d.writeInt16LE(v, 44 + i * 4); d.writeInt16LE(v, 46 + i * 4); }
 return d;
}
function mock(dir, template, subjectFile) {
 fs.rmSync(dir, { recursive: true, force: true }); fs.mkdirSync(path.join(dir, "audio"), { recursive: true });
 const subject = JSON.parse(fs.readFileSync(subjectFile, "utf8"));
 const phrases = Object.keys(subject.items.en);
 // The sources in the order the generators give tokens: the first pair's two, then the second's other.
 const v = [...new Set(subject.pairs.flat())];
 const tok = ["c3aa0001", "5a1a0002", "c2bb0003", "5a1a0d04", "c3aa0d05"], piano = [v[0], v[1], v[2], v[1], v[0]];
 const ti = id => tok[v.indexOf(id)];
 const hex = () => [...Array(16)].map(() => "0123456789abcdef"[Math.floor(Math.random() * 16)]).join("");
 const files = {}, catches = [{ phrase: phrases[phrases.length - 2], one: tok[1], two: tok[3] }, { phrase: phrases[phrases.length - 1], one: tok[0], two: tok[4] }];
 for (const p of phrases) for (let t = 0; t < 5; t++) {
  if (t >= 3 && !catches.some(c => c.phrase === p && c.two === tok[t])) continue;
  const name = hex() + ".wav"; files[p + "|" + tok[t]] = name;
  fs.writeFileSync(path.join(dir, "audio", name), wav([440, 330, 550, 330, 440][t], 2.0));
 }
 const table = JSON.stringify(Object.fromEntries(tok.map((t, i) => [t, piano[i]])));
 const seed = 12345;
 const pairs = subject.pairs.map(p => p.map(ti));
 const tests = `{abx:${JSON.stringify(pairs[0])},pref:${JSON.stringify(pairs)},extra:${JSON.stringify(pairs[1].slice().reverse())},catch:${JSON.stringify(catches)}}`;
 const page = fs.readFileSync(template, "utf8").replace("__KIT__", '"abx-mock000000"').replace("__PHRASES__", JSON.stringify(phrases))
  .replace("__FILES__", JSON.stringify(files)).replace("__TESTS__", tests).replace("__SEED__", String(seed)).replace("__REVEAL__", JSON.stringify(veil(table, seed)))
  .replace("__SUBJECT__", JSON.stringify(subject));
 fs.writeFileSync(path.join(dir, "index.html"), page);
}
if (args[1] === "--mock") mock(kit, path.resolve(args[2]), path.resolve(args[3]));

// ---- The kit's own consistency, read from its page.
const html = fs.readFileSync(path.join(kit, "index.html"), "utf8");
function constOf(name) { const m = html.match(new RegExp("const " + name + " = (.*);\\r?\\n")); return m && m[1]; }
const FILES = JSON.parse(constOf("FILES")), PHRASES = JSON.parse(constOf("PHRASES"));
const TESTS = Function("return " + constOf("TESTS"))();
const SUBJECT = JSON.parse(constOf("SUBJECT"));
const N1 = SUBJECT.trials.abx, NX = SUBJECT.trials.extra, NP = PHRASES.length * TESTS.pref.length + TESTS.catch.length, PLAY = SUBJECT.play;
const VOICES = [...new Set(SUBJECT.pairs.flat())];
// What must not show before the reveal, by subject: in the page's data, on the first screen, in
// the readmes.
const HIDE = {
 chorda: { data: ["chorda", "salamander", "cand", "beta", "round", "ronda", "luis"], screen: /salamander|chorda|cantus|yamaha/,
  readme: ["cargo", "Chorda", "Salamander", "candidat", "ronda", "round", "Luis", "voicing"] },
 tympanum: { data: ["tympanum", "tymp", "drskit", "virtuosity", "model", "hybrid", "hibrid", "generat", "generad", "record", "grabac", "studio", "estudio", "jazz", "beta", "round", "ronda", "luis", "cantus"],
  screen: /tympanum|drskit|virtuosity|drumgizmo|cantus|modelad|modelled|generad|generated|híbrid|hybrid|grabación|recording/,
  readme: ["cargo", "Tympanum", "DRSKit", "Virtuosity", "DrumGizmo", "ronda", "round", "Luis", "modelo", "generad", "híbrid", "hybrid"] }
}[SUBJECT.id];
if (!HIDE) { console.log("no hide list for the subject " + SUBJECT.id); process.exit(2); }
const tokens = [...new Set([...TESTS.abx, ...TESTS.pref.flat(), ...TESTS.extra])];
for (const p of PHRASES) for (const t of tokens) ok(FILES[p + "|" + t] && fs.existsSync(path.join(kit, "audio", FILES[p + "|" + t])), `file for ${p}|${t} exists`);
for (const c of TESTS.catch) {
 const a = FILES[c.phrase + "|" + c.one], b = FILES[c.phrase + "|" + c.two];
 ok(a && b && a !== b && fs.readFileSync(path.join(kit, "audio", a)).equals(fs.readFileSync(path.join(kit, "audio", b))), `check question ${c.phrase}: one clip under two names`);
}
const names = fs.readdirSync(path.join(kit, "audio"));
ok(names.length === Object.keys(FILES).length, `every audio file is in the page (${names.length})`);
let worst = 0;
for (let i = 0; i < names.length; i++) for (let j = i + 1; j < names.length; j++) { let s = 0; for (let k = 0; k < 16; k++) if (names[i][k] === names[j][k]) s++; worst = Math.max(worst, s); }
ok(worst <= 9, `no two names share structure (most shared digits ${worst} of 16)`);
// The kit's data in the page (its code names the pianos only to label the reveal).
const plain = ["KIT", "PHRASES", "FILES", "TESTS", "SEED", "REVEAL"].map(constOf).join("\n");
for (const w of HIDE.data) ok(!plain.toLowerCase().includes(w), `the page's data never says "${w}"`);
for (const f of ["LEEME.md", "README.md"]) { const t = fs.existsSync(path.join(kit, f)) ? fs.readFileSync(path.join(kit, f), "utf8") : ""; ok(t.length > 0 || args[1] === "--mock", `${f} is in the kit`); for (const w of HIDE.readme) ok(!t.includes(w), `${f} never says "${w}"`); }

// ---- A static server for the http runs.
const types = { ".html": "text/html; charset=utf-8", ".flac": "audio/flac", ".wav": "audio/wav", ".md": "text/markdown" };
const server = http.createServer((q, r) => {
 const f = path.join(kit, decodeURIComponent(q.url.split("?")[0].split("#")[0]).replace(/^\/+/, "") || "index.html");
 if (!f.startsWith(kit) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { r.writeHead(404); r.end(); return; }
 r.writeHead(200, { "content-type": types[path.extname(f)] || "application/octet-stream" }); fs.createReadStream(f).pipe(r);
});
await new Promise(ok => server.listen(0, "127.0.0.1", ok));
const httpUrl = `http://127.0.0.1:${server.address().port}/index.html`;
const fileUrl = pathToFileURL(path.join(kit, "index.html")).href;

const sleep = ms => new Promise(ok => setTimeout(ok, ms));
async function S(page, expr, arg) { return page.evaluate(expr, arg); }
const state = page => S(page, () => { const a = window.__abx.audio; return { playing: a.playing(), pos: a.position(), dur: a.duration(), loop: a.loop, mode: a.mode, view: window.__abx.st().view }; });
async function waitFor(page, fn, what, ms = 4000) { const t0 = Date.now(); while (Date.now() - t0 < ms) { if (await fn()) return true; await sleep(50); } ok(false, what + " (timed out)"); return false; }

async function run(name, browserType, launch, url, mode) {
 const tag = `[${name} ${mode}]`;
 log.push(tag);
 const browser = await browserType.launch(launch);
 const ctx = await browser.newContext({ locale: "es-ES", acceptDownloads: true });
 const page = await ctx.newPage();
 const errors = []; page.on("pageerror", e => errors.push(String(e))); page.on("crash", () => console.log(`${tag} RENDERER CRASHED`)); browser.on("disconnected", () => console.log(`${tag} browser disconnected`)); page.on("response", r => { if (r.status() >= 400 && !r.url().endsWith("/favicon.ico")) console.log(`${tag} HTTP ${r.status()} ${r.url()}`); });
 await page.goto(url);
 ok(await page.title() === "Prueba de escucha", `${tag} Spanish by default from es-ES`);
 // Nothing names a piano before the end.
 const before = (await page.innerText("body")).toLowerCase();
 ok(!HIDE.screen.test(before), `${tag} no source named on the first screen`);
 // The language toggle.
 await page.click("#lang-en"); ok(await page.title() === "Listening test" && (await page.getAttribute("#lang-en", "aria-pressed")) === "true", `${tag} EN toggle`);
 await page.click("#lang-es"); ok(await page.title() === "Prueba de escucha", `${tag} ES toggle`);
 // The form: Start needs the playback device.
 ok(await page.isDisabled("#start"), `${tag} Start disabled until the device is chosen`);
 await page.click("label:has(#device-headphones)");
 ok(!(await page.isDisabled("#start")), `${tag} device chosen enables Start`);
 await page.click(`label:has(#${PLAY}-some)`); await page.fill("#years", "4");
 await page.click("label:has(#audio-hobby)"); await page.click("label:has(#age-40)");
 await page.fill("#who", "Tester");
 let st = await S(page, () => window.__abx.st());
 ok(st.about.device === "headphones" && st.about[PLAY] === "some" && st.about.years === "4" && st.about.audio === "hobby" && st.about.age === "40" && st.listener === "Tester", `${tag} questionnaire recorded`);
 await page.click("#start");
 ok((await state(page)).view === "abx", `${tag} Start opens part 1`);
 ok((await page.textContent("#steps")).includes(`1 de ${N1}`), `${tag} progress "1 de ${N1}"`);
 ok((await state(page)).mode === mode, `${tag} audio engine is ${mode}`);
 // Transport: A plays, B switches keeping the place, B again restarts, X, Stop.
 await page.click("#play-a");
 await waitFor(page, async () => (await state(page)).pos > 0.5, `${tag} A plays`);
 let s = await state(page);
 ok(s.playing === "a" && (await page.getAttribute("#play-a", "class")).includes("on"), `${tag} A lit while playing`);
 const p1 = s.pos; await page.click("#play-b");
 await waitFor(page, async () => (await state(page)).playing === "b", `${tag} B takes over`);
 s = await state(page); ok(s.pos >= p1 - 0.05 && s.pos < p1 + 0.6, `${tag} switch A→B keeps the place (${p1.toFixed(2)} → ${s.pos.toFixed(2)} s)`);
 await sleep(200); await page.click("#play-b"); await sleep(150);
 s = await state(page); ok(s.playing === "b" && s.pos < 0.4, `${tag} B again starts over (${s.pos.toFixed(2)} s)`);
 await sleep(300); const p2 = (await state(page)).pos; await page.click("#play-x"); await sleep(120);
 s = await state(page); ok(s.playing === "x" && s.pos >= p2 - 0.05, `${tag} X keeps the place`);
 await page.click("#stop"); s = await state(page); ok(s.playing === null && !(await page.getAttribute("#play-x", "class")).includes("on"), `${tag} Stop`);
 st = await S(page, () => window.__abx.st());
 ok(st.abx[0].plays === 4 && st.abx[0].heard.a === 1 && st.abx[0].heard.b === 2 && st.abx[0].heard.x === 1, `${tag} every press counted (${JSON.stringify(st.abx[0].heard)})`);
 // Loop: lit, and a clip loops past its end; off, it ends.
 await page.click("#loop");
 ok((await state(page)).loop && (await page.getAttribute("#loop", "aria-pressed")) === "true" && (await page.getAttribute("#loop", "class")).includes("on"), `${tag} Loop on, lit`);
 await page.click("#play-a"); await waitFor(page, async () => (await state(page)).dur > 0, `${tag} duration known`);
 const dur = (await state(page)).dur;
 const seekBack = back => S(page, b => { const a = window.__abx.audio; a.seek(a.duration() - b); }, back);
 const seekEnd = () => seekBack(0.3);
 // A clip played into its end wraps. (Firefox's <audio> may take half a second to seek a
 // file:// clip: wait for the jump, then the wrap.)
 await sleep(150); await seekBack(1.0);
 let high = false;
 await waitFor(page, async () => { const q = await state(page); if (q.pos > q.dur - 1.2) high = true; return high && q.pos < 0.75; }, `${tag} Loop: the clip wraps`, 5000);
 s = await state(page); ok(high && s.playing === "a" && s.pos < 0.75, `${tag} Loop: past the end it starts over (at ${s.pos.toFixed(2)} of ${dur.toFixed(2)} s)`);
 // A jump (a switch, a click on the bar) into the last moments of a looping clip never stalls
 // (Firefox's <audio> FLAC did): it wraps and plays on.
 await seekEnd(); await sleep(400); const t1 = (await state(page)).pos; await sleep(1100); const t2 = (await state(page)).pos;
 ok((await state(page)).playing === "a" && t2 < 2.0 && t2 > t1, `${tag} Loop: a jump into the last 0.3 s plays on (${t1.toFixed(2)} → ${t2.toFixed(2)} s)`);
 await page.click("#loop");
 ok(!(await state(page)).loop && (await page.getAttribute("#loop", "aria-pressed")) === "false" && !(await page.getAttribute("#loop", "class")).includes("on"), `${tag} Loop off, unlit`);
 ok((await state(page)).playing === "a", `${tag} Loop off keeps the clip playing`);
 await seekEnd();
 await waitFor(page, async () => (await state(page)).playing === null, `${tag} without loop the clip ends`, 3000);
 ok((await state(page)).playing === null && !(await page.getAttribute("#play-a", "class")).includes("on"), `${tag} without loop the clip ends and A goes dark`);
 // The position bar: a click at three quarters jumps there.
 await page.click("#play-b"); await sleep(200);
 const box = await page.locator("#bar").boundingBox();
 await page.mouse.click(box.x + box.width * 0.75, box.y + box.height / 2);
 await waitFor(page, async () => { const q = await state(page); return Math.abs(q.pos - 0.75 * q.dur) < 0.35; }, `${tag} the bar's jump lands`, 3000);
 s = await state(page); ok(s.playing === "b" && Math.abs(s.pos - 0.75 * s.dur) < 0.35, `${tag} position bar click seeks (${s.pos.toFixed(2)} of ${s.dur.toFixed(2)} s)`);
 await page.click("#stop");
 // Keys: A plays, space stops, space again plays the last, L loops, space on a focused button
 // never presses it.
 await page.keyboard.press("a"); await sleep(150); ok((await state(page)).playing === "a", `${tag} key A`);
 await page.keyboard.press("x"); await sleep(150); ok((await state(page)).playing === "x", `${tag} key X`);
 await page.keyboard.press("b"); await sleep(150); ok((await state(page)).playing === "b", `${tag} key B`);
 await page.keyboard.press(" "); await sleep(100); ok((await state(page)).playing === null, `${tag} space stops`);
 await page.keyboard.press(" "); await sleep(150); ok((await state(page)).playing === "b", `${tag} space plays the last again`);
 await page.keyboard.press("l"); ok((await state(page)).loop, `${tag} key L loops`);
 await page.keyboard.press("l"); ok(!(await state(page)).loop, `${tag} key L again unloops`);
 await page.focus("#loop"); await page.keyboard.press(" "); await sleep(100);
 s = await state(page); ok(!s.loop && s.playing === null, `${tag} space on the focused Loop button stops, does not toggle it`);
 await page.focus("#answer-a"); await page.keyboard.press(" "); await sleep(100);
 ok((await S(page, () => window.__abx.st().abx[0].answer)) === null, `${tag} space on a focused answer button does not answer`);
 // Answers, Previous / Next.
 ok(await page.isDisabled("#prev") && await page.isDisabled("#next"), `${tag} Previous/Next disabled on the first unanswered trial`);
 await page.click("#answer-a");
 st = await S(page, () => window.__abx.st()); ok(st.abx[0].answer === "a" && st.pos.abx === 1 && st.abx[0].at, `${tag} "X es A" answers and moves on`);
 ok((await page.textContent("#steps")).includes(`2 de ${N1}`), `${tag} progress "2 de ${N1}"`);
 await page.click("#prev"); st = await S(page, () => window.__abx.st());
 ok(st.pos.abx === 0 && (await page.getAttribute("#answer-a", "class")).includes("chosen"), `${tag} Previous shows the answer given`);
 await page.click("#next"); ok((await S(page, () => window.__abx.st().pos.abx)) === 1, `${tag} Next`);
 await page.click("#prev"); await page.click("#answer-b"); st = await S(page, () => window.__abx.st());
 ok(st.abx[0].answer === "b" && st.abx[0].changes === 1 && st.pos.abx === 1, `${tag} a changed answer is kept and counted`);
 // The language changes while a clip plays and it keeps playing.
 await page.click("#play-a"); await sleep(200); await page.click("#lang-en"); await sleep(100);
 s = await state(page); ok(s.playing === "a" && (await page.textContent("#answer-a")) === "X is A", `${tag} language switch keeps the clip playing`);
 await page.click("#lang-es");
 // Every item as often as the others in part 1 (one more for those the remainder draws).
 st = await S(page, () => window.__abx.st());
 { const c = {}; for (const x of st.abx) c[x.phrase] = (c[x.phrase] || 0) + 1; const n = Object.values(c); ok(st.abx.length === N1 && Object.keys(c).length === Math.min(N1, PHRASES.length) && Math.max(...n) - Math.min(...n) <= 1, `${tag} part 1 draws every item evenly (${JSON.stringify(c)})`); }
 for (let i = 1; i < N1; i++) await page.click(i % 2 ? "#answer-a" : "#answer-b");
 ok((await state(page)).view === "pref", `${tag} after ${N1} trials, part 2`);
 // Preferences: 1 / 2 / no preference, the note, keys 1 and 2.
 await page.click("#play-one"); await sleep(150); ok((await state(page)).playing === "one", `${tag} Clip 1 plays`);
 await page.click("#play-two"); await sleep(150); ok((await state(page)).playing === "two", `${tag} Clip 2 plays`);
 await page.keyboard.press("1"); await sleep(120); ok((await state(page)).playing === "one", `${tag} key 1`);
 await page.keyboard.press("2"); await sleep(120); ok((await state(page)).playing === "two", `${tag} key 2`);
 await page.fill("#note", "suena natura"); await page.keyboard.press("1");
 ok((await S(page, () => window.__abx.st().pref[0].note)) === "suena natura1" && (await state(page)).playing === "two", `${tag} typing a note (with a "1") plays nothing and is kept`);
 await page.fill("#note", "suena natural");
 await page.click("#choose-same"); st = await S(page, () => window.__abx.st());
 ok(st.pref[0].choice === "same" && st.pref[0].note === "suena natural" && st.pref[0].plays === 4, `${tag} "Sin preferencia" recorded with its note and plays`);
 await page.click("#choose-one"); await page.click("#choose-two");
 // Pause: reload, carry on where it was.
 await page.reload();
 st = await S(page, () => window.__abx.st());
 ok(st.view === "pref" && st.pos.pref === 3 && st.pref[1].choice === "one" && st.pref[2].choice === "two", `${tag} reload resumes at question 4 with the answers`);
 ok(!(await page.isHidden("#msg")) && (await page.textContent("#msg")).includes("Bienvenido"), `${tag} welcome-back message`);
 ok((await S(page, () => window.__abx.st().pref.length)) === NP, `${tag} ${NP} preference questions`);
 for (let i = 3; i < NP; i++) await page.click(["#choose-one", "#choose-two", "#choose-same"][i % 3]);
 ok((await state(page)).view === "between", `${tag} after ${NP} questions, the end of the main test`);
 const checks = await S(page, () => { const s = window.__abx.st(), u = window.__abx.unveil(); return s.pref.filter(q => u[q.one] === u[q.two]).length; });
 ok(checks === 2, `${tag} two check questions among the preferences`);
 // Extra part, then results and the reveal.
 await page.click("#extra"); ok((await state(page)).view === "extra", `${tag} the extra part opens`);
 for (let i = 0; i < NX; i++) await page.click("#answer-a");
 ok((await state(page)).view === "results", `${tag} the extra part ends on the results`);
 const body = await page.innerText("body");
 ok(VOICES.every(v => body.includes(SUBJECT.text.es["voice_" + v])) && !body.includes("voice_") && (await page.$$("table tr")).length >= N1 + NP + NX + 3, `${tag} the reveal names the sources per trial`);
 for (const [t, u] of SUBJECT.credits.links) ok(!!(await page.$(`#credits a[href="${u}"]`)), `${tag} credits link ${t}`);
 ok(SUBJECT.credits.text.every(k => body.includes(SUBJECT.text.es[k])), `${tag} the credits' words`);
 {
  const [dl] = await Promise.all([page.waitForEvent("download"), page.click("#download")]);
  const text = fs.readFileSync(await dl.path(), "utf8"); const d = JSON.parse(text);
  ok(d.kit && d.about.device === "headphones" && d.listener === "Tester" && d.abx.every(x => x.answer && x.at) && d.pref.every(q => q.choice && q.at) && d.extra.every(x => x.answer) && d.started && d.mainDone && d.extraDone && d.exported, `${tag} Download: the JSON holds answers, form, timestamps (${dl.suggestedFilename()})`);
 }
 // Start again: two presses.
 await page.click("#reset"); ok((await state(page)).view === "results" && (await page.textContent("#reset")).includes("Seguro"), `${tag} Start again asks once`);
 await page.click("#reset"); st = await S(page, () => window.__abx.st());
 ok(st.view === "intro" && st.abx.every(x => x.answer === null) && st.earlier.length === 1 && st.about.device === "headphones", `${tag} Start again erases, keeps the old attempt and the form`);
 ok(errors.length === 0, `${tag} no page errors ${errors.join(" | ")}`);
 await browser.close();
}

// ---- The published page: `window.claude` stubbed (user, db, downloads) as claude.ai serves it.
const stub = ({ owner, canWrite }) => `
 window.__writes = []; window.__subs = []; window.__saves = [];
 const done = (who, dev, piano, right) => { const s = fresh(); s.listener = who; s.about = { device: dev, years: "", audio: "no", age: "" }; s.about[PLAY] = piano;
  s.started = "2026-11-13T10:00:00Z"; s.mainDone = "2026-11-13T10:17:00Z"; s.view = "results";
  s.abx.forEach((x, i) => { x.answer = i < right ? x.x : (x.x === "a" ? "b" : "a"); x.at = s.mainDone; });
  s.pref.forEach(q => { q.choice = unveil()[q.one] === unveil()[q.two] ? "same" : "one"; q.at = s.mainDone; }); return { kit: KIT, listener: who, about: s.about, results: s }; };
 window.__mkDocs = () => [done("L1", "headphones", "yes", TRIALS.abx), done("L2", "speakers", "no", TRIALS.abx / 2)];
 window.claude = { use: async name => {
  if (name === "user") return { id: async () => "uid-1", isOwner: async () => ${owner}, can: async () => ${canWrite} };
  if (name === "db") return {
   doc: p => ({ set: async d => { if (!${canWrite} && !${owner}) { const e = new Error("no"); e.code = "invalid_argument"; throw e; } window.__writes.push({ path: p, data: JSON.parse(JSON.stringify(d)) }); } }),
   collection: p => ({ onSnapshot: (cb) => { window.__subs.push(p); setTimeout(() => cb({ docs: window.__mkDocs().map(d => ({ data: () => d })) }), 50); return () => {}; } })
  };
  if (name === "downloads") return { save: async f => { window.__saves.push(f); } };
  return null;
 } };`;
async function online(browserType, launch) {
 const tag = "[online]"; log.push(tag);
 const browser = await browserType.launch(launch);
 // A listener who may write: saved as they go, nobody else's answers read.
 let ctx = await browser.newContext({ locale: "en-US" }); let page = await ctx.newPage();
 const errors = []; page.on("pageerror", e => errors.push(String(e)));
 await page.addInitScript(stub({ owner: false, canWrite: true }));
 await page.goto(httpUrl);
 ok(await page.title() === "Listening test", `${tag} English by default from en-US`);
 await page.click("label:has(#device-speakers)"); await page.click("#start");
 await page.click("#answer-a"); await sleep(1500);
 let w = await S(page, () => window.__writes);
 const last = w[w.length - 1];
 ok(last && last.path === "abx/uid-1" && last.data.kit && last.data.about.device === "speakers" && last.data.results.abx[0].answer === "a" && last.data.results.abx[0].at && last.data.results.started && last.data.updatedAt, `${tag} the listener's document abx/<id> holds kit, form, answers, timestamps`);
 ok(HIDE.data.every(w => !JSON.stringify(last.data).toLowerCase().includes(w)), `${tag} the saved document names no source`);
 ok((await S(page, () => window.__subs)).length === 0, `${tag} a listener never reads the collection`);
 ok((await page.textContent("#msg")).includes("saved automatically"), `${tag} "saved automatically" shown`);
 ok(!(await page.$("#owner-open")), `${tag} no summary button for a listener`);
 await page.goto(httpUrl + "#summary"); ok((await S(page, () => window.__subs)).length === 0, `${tag} #summary as a listener reads nobody's documents`);
 // The results' download through the capability.
 await S(page, () => { const s = window.__abx.st(); s.abx.forEach(x => x.answer = "a"); s.pref.forEach(q => q.choice = "same"); s.mainDone = new Date().toISOString(); s.view = "results"; window.__abx.render(); });
 await page.click("#download"); await sleep(200);
 const sv = await S(page, () => window.__saves);
 ok(sv.length === 1 && /^listening-.*\.json$/.test(sv[0].filename) && JSON.parse(sv[0].data).kit, `${tag} Download through the downloads capability (${sv[0] && sv[0].filename})`);
 ok(errors.length === 0, `${tag} no page errors ${errors.join(" | ")}`);
 await ctx.close();
 // A visitor who may not write: told to download and send the file.
 ctx = await browser.newContext({ locale: "es-ES" }); page = await ctx.newPage();
 await page.addInitScript(stub({ owner: false, canWrite: false }));
 await page.goto(httpUrl); await sleep(300);
 ok((await page.textContent("#msg")).includes("descarga el archivo"), `${tag} a visitor who cannot write is told to download and send the file`);
 await ctx.close();
 // The owner: every listener's scores, pooled by group.
 ctx = await browser.newContext({ locale: "es-ES" }); page = await ctx.newPage();
 await page.addInitScript(stub({ owner: true, canWrite: true }));
 await page.goto(httpUrl); await sleep(300);
 await page.click("#owner-open"); await sleep(300);
 const sum = await page.textContent("#summary");
 ok(sum.includes("L1") && sum.includes("L2") && sum.includes(`${N1}/${N1}`) && sum.includes(`${N1 / 2}/${N1}`) && sum.includes("2/2"), `${tag} owner's summary: each listener's ABX and checks`);
 const groups = await page.textContent("#groups");
 ok(groups.includes(`${N1 * 1.5}/${N1 * 2}`) && groups.includes("Auriculares") && groups.includes("Pasaron"), `${tag} owner's summary pooled by group (everyone ${N1 * 1.5}/${N1 * 2}, by device, by the instrument question, check-passers)`);
 ok((await S(page, () => window.__subs)).join() === "abx", `${tag} the owner reads the collection`);
 await page.click("#back"); ok((await S(page, () => window.__abx.st().view)) === "intro", `${tag} Back from the summary`);
 await browser.close();
}

const runs = [
 ["edge", chromium, { channel: "msedge" }],
 ["chrome", chromium, { channel: "chrome" }],
 ["firefox", firefox, {}],
];
for (const [name, type, launch] of runs) {
 if (only && !only.split(",").includes(name)) continue;
 for (const [url, mode] of [[httpUrl, "webaudio"], [fileUrl, "element"]]) {
  try { await run(name, type, launch, url, mode); } catch (e) { ok(false, `[${name} ${mode}] ${String(e).split("\n")[0]}`); }
 }
}
if (!only || only.split(",").includes("online")) { try { await online(chromium, { channel: "msedge" }); } catch (e) { ok(false, `[online] ${String(e).split("\n")[0]}`); } }
server.close();
console.log(log.join("\n"));
console.log(`${passes} passed, ${failures} failed`);
process.exit(failures ? 1 : 0);
