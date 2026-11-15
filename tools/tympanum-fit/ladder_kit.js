// The drum ladder's listening page (round 40, ladder): MUSHRA items, then ABX tests.
// CFG (set by ladder_kit.py) holds only random file names and tokens: which rung each one
// is lives in the separate key folder.
// Spanish first, whatever the browser says; EN is one press away.
let lang = "es";
function tr(k, v) { let s = T[lang][k] !== undefined ? T[lang][k] : k; for (const n in (v || {})) s = String(s).split("{" + n + "}").join(v[n]); return s; }
function rnd(n) { const a = new Uint32Array(1); crypto.getRandomValues(a); return a[0] % n; }
function shuffle(a) { for (let i = a.length - 1; i > 0; i--) { const j = rnd(i + 1); [a[i], a[j]] = [a[j], a[i]]; } return a; }
function now() { return new Date().toISOString(); }
function $(id) { return document.getElementById(id); }
function el(tag, attrs, kids) {
 const e = document.createElement(tag);
 for (const k in (attrs || {})) { if (k === "on") for (const ev in attrs.on) e.addEventListener(ev, attrs.on[ev]); else if (k === "text") e.textContent = attrs[k]; else e.setAttribute(k, attrs[k]); }
 for (const c of (kids || [])) if (c) e.appendChild(typeof c === "string" ? document.createTextNode(c) : c);
 return e;
}
function fresh() {
 return { kit: CFG.kit, version: 1, created: now(), sessions: [now()], lang: lang, device: "", name: "", view: "intro", mi: 0, ai: 0,
  mushra: CFG.mushra.map(it => ({ item: it.id, order: shuffle(it.stims.map(s => s.id)), scores: {}, plays: {}, at: null })),
  abx: CFG.abx.map(t => ({ test: t.id, trials: Array.from({ length: CFG.trials }, () => ({ swap: rnd(2) === 1, x: rnd(2) ? "a" : "b", answer: null, plays: 0, at: null })), pos: 0 })),
  ua: navigator.userAgent };
}
let st = null;
try { st = JSON.parse(localStorage.getItem(STORE)); } catch (e) { st = null; }
if (!st || st.kit !== CFG.kit || !Array.isArray(st.mushra)) st = fresh(); else { lang = st.lang || lang; st.sessions.push(now()); }
function save() { st.lang = lang; try { localStorage.setItem(STORE, JSON.stringify(st)); } catch (e) {} }

// ---- audio: one-shots, each press plays from the start and stops the one before
const player = { els: new Map(), cur: null, key: null, onchange: () => {},
 get(url) { if (!this.els.has(url)) { const a = new Audio(); a.preload = "auto"; a.src = url; this.els.set(url, a); } return this.els.get(url); },
 play(key, url) {
  this.stop();
  const a = this.get(url); this.cur = a; this.key = key;
  a.onended = () => { if (this.cur === a) { this.cur = null; this.key = null; this.onchange(); } };
  try { a.currentTime = 0; } catch (e) {}
  const p = a.play(); if (p && p.catch) p.catch(() => {});
  this.onchange();
 },
 stop() { if (this.cur) { this.cur.pause(); } this.cur = null; this.key = null; this.onchange(); }
};
let tick = null;
function progressBar(fill) { const b = el("div", { class: "bar" }, [el("div")]); fill(b.firstChild); return b; }

function header(text) {
 $("title").textContent = tr("title");
 $("progress").textContent = text || "";
 for (const l of ["es", "en"]) $("lang-" + l).setAttribute("aria-pressed", String(l === lang));
 document.documentElement.lang = lang;
}
function itemName(it) { return T[lang].items[it.art] + " · " + (it.vel >= 110 ? tr("hard") : tr("soft")); }

function render() {
 player.stop(); clearInterval(tick);
 const v = $("view"); v.innerHTML = "";
 if (st.view === "intro") return intro(v);
 if (st.view === "mushra") return mushra(v);
 if (st.view === "abx") return abx(v);
 return done(v);
}

function intro(v) {
 header("");
 const dev = el("fieldset", {}, [el("legend", { text: tr("q_device") }), el("div", { class: "pills" },
  [["headphones", "dev_hp"], ["monitors", "dev_mon"], ["speakers", "dev_spk"]].map(([k, l]) => {
   const i = el("input", { type: "radio", name: "dev", value: k }); if (st.device === k) i.checked = true;
   i.addEventListener("change", () => { st.device = k; save(); });
   return el("label", {}, [i, el("span", { text: tr(l) })]);
  }))]);
 const name = el("input", { type: "text", value: st.name || "", maxlength: "60" });
 name.addEventListener("input", () => { st.name = name.value; save(); });
 const msg = el("p", { class: "warn", hidden: "" });
 const go = el("button", { class: "primary", type: "button", text: st.mi || st.ai ? tr("resume") : tr("start"), on: { click: () => {
  if (!st.device) { msg.textContent = tr("need_device"); msg.hidden = false; return; }
  st.view = st.mi < CFG.mushra.length ? "mushra" : (st.ai < CFG.abx.length ? "abx" : "done"); save(); render();
 } } });
 v.append(el("div", { class: "card" }, [el("p", { text: tr("lead") }),
  el("p", { text: tr("howlong", { m: CFG.mushra.length, a: CFG.abx.length, n: CFG.trials }) }), el("p", { class: "mute", text: tr("setup") })]),
  el("div", { class: "card" }, [dev, el("fieldset", {}, [el("legend", { text: tr("q_name") }), name]), msg, go]),
  el("div", { class: "card small mute" }, [el("h2", { text: tr("credits") }), el("p", { text: tr("credit") })]));
}

function mushra(v) {
 const it = CFG.mushra[st.mi], s = st.mushra[st.mi];
 header(tr("part1") + " · " + tr("test", { i: st.mi + 1, n: CFG.mushra.length }));
 const file = id => it.stims.find(x => x.id === id).file;
 const btns = [];
 const btn = (key, label, url, cls) => { const b = el("button", { type: "button", class: cls || "", text: label, on: { click: () => {
  if (player.key === key) { player.stop(); return; } player.play(key, url); if (key !== "ref") { s.plays[key] = (s.plays[key] || 0) + 1; save(); check(); } } } }); btns.push([key, b]); return b; };
 player.onchange = () => { for (const [k, b] of btns) b.classList.toggle("on", player.key === k); };
 const rows = [];
 const next = el("button", { class: "primary", type: "button", text: tr("next") });
 const msg = el("p", { class: "small mute", text: tr("need_all") });
 function check() { const ok = s.order.every(id => s.scores[id] !== undefined && s.plays[id]); next.disabled = !ok; msg.hidden = ok; }
 s.order.forEach((id, i) => {
  const val = el("span", { class: "val" + (s.scores[id] === undefined ? " unset" : ""), text: s.scores[id] === undefined ? tr("unset") : String(s.scores[id]) });
  const r = el("input", { type: "range", min: "0", max: "100", step: "1", value: String(s.scores[id] === undefined ? 50 : s.scores[id]), "aria-label": tr("sample", { i: i + 1 }) });
  r.addEventListener("input", () => { s.scores[id] = +r.value; s.at = now(); val.textContent = r.value; val.classList.remove("unset"); save(); check(); });
  rows.push(el("div", { class: "stim" }, [btn(id, tr("sample", { i: i + 1 }), file(id)), r, val]));
 });
 const scale = el("div", { class: "scale" }, [el("div"), el("div", {}, T[lang].scale.map(w => el("span", { text: w }))), el("div")]);
 const prev = el("button", { type: "button", text: tr("prev"), on: { click: () => { st.mi -= 1; if (st.mi < 0) { st.mi = 0; st.view = "intro"; } save(); render(); } } });
 next.addEventListener("click", () => { st.mi += 1; if (st.mi >= CFG.mushra.length) st.view = "abx"; save(); render(); });
 v.append(el("div", { class: "card" }, [el("h2", { text: itemName(it) }), el("p", { class: "small mute", text: tr("mushra_help") }),
  el("div", { class: "row" }, [btn("ref", tr("ref"), it.ref, "big"), el("button", { type: "button", text: tr("stop"), on: { click: () => player.stop() } })]),
  scale, ...rows, msg, el("div", { class: "row", style: "margin-top:12px" }, [prev, next])]));
 check();
}

function abx(v) {
 const t = CFG.abx[st.ai], s = st.abx[st.ai];
 const tri = s.trials[s.pos];
 header(tr("part2") + " · " + tr("test", { i: st.ai + 1, n: CFG.abx.length }) + " · " + tr("trial", { i: s.pos + 1, n: CFG.trials }));
 const a = tri.swap ? t.files[1] : t.files[0], b = tri.swap ? t.files[0] : t.files[1];
 const files = { a: a, b: b, x: tri.x === "a" ? a : b };
 const btns = [];
 for (const k of ["a", "b", "x"]) btns.push([k, el("button", { type: "button", class: "big", text: k.toUpperCase(), on: { click: () => {
  if (player.key === k) { player.stop(); return; } player.play(k, files[k]); tri.plays++; save(); } } })]);
 player.onchange = () => { for (const [k, bt] of btns) bt.classList.toggle("on", player.key === k); };
 const ans = [["a", "x_is_a"], ["b", "x_is_b"]].map(([k, l]) => el("button", { type: "button", class: "answer" + (tri.answer === k ? " chosen" : ""), text: tr(l), on: { click: () => {
  tri.answer = k; tri.at = now(); save();
  if (s.pos < CFG.trials - 1) s.pos++; else { st.ai++; if (st.ai >= CFG.abx.length) st.view = "done"; }
  save(); render(); } } }));
 const prev = el("button", { type: "button", text: tr("prev"), on: { click: () => {
  if (s.pos > 0) s.pos--; else if (st.ai > 0) st.ai--; else { st.view = "mushra"; st.mi = CFG.mushra.length - 1; }
  save(); render(); } } });
 const bar = progressBar(d => { d.style.width = (100 * s.pos / CFG.trials) + "%"; });
 v.append(el("div", { class: "card" }, [el("h2", { text: T[lang].abx_names[t.art] + " · " + (t.vel >= 110 ? tr("hard") : tr("soft")) }),
  el("p", { class: "small mute", text: tr("abx_help") }), el("div", { class: "row" }, btns.map(x => x[1]).concat([el("button", { type: "button", text: tr("stop"), on: { click: () => player.stop() } })])),
  el("div", { class: "row", style: "margin-top:14px" }, ans), el("div", { class: "row", style: "margin-top:14px" }, [prev, bar])]));
}

function binomP(k, n) { let p = 0; for (let i = k; i <= n; i++) { let c = 1; for (let j = 0; j < i; j++) c = c * (n - j) / (j + 1); p += c; } return p / Math.pow(2, n); }

function done(v) {
 header(tr("done"));
 st.finished = st.finished || now(); save();
 const rows = CFG.abx.map((t, i) => {
  const tr_ = st.abx[i].trials, n = tr_.filter(x => x.answer).length, k = tr_.filter(x => x.answer && x.answer === x.x).length, p = binomP(k, n);
  return el("p", { text: tr("res_abx", { name: tr("test", { i: i + 1, n: CFG.abx.length }) + " (" + T[lang].abx_names[t.art] + ")", k: k, n: n, p: p.toFixed(3).replace(".", lang === "es" ? "," : ".") }) + " — " + (p < 0.05 ? tr("sig") : tr("nsig")) + "." });
 });
 const dl = el("button", { class: "primary", type: "button", text: tr("download"), on: { click: () => {
  const blob = new Blob([JSON.stringify(st, null, 1)], { type: "application/json" });
  const u = URL.createObjectURL(blob); const a = el("a", { href: u, download: "escalera-" + CFG.kit + "-" + (st.name || "oyente").replace(/[^\w-]+/g, "_") + ".json" });
  document.body.appendChild(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(u), 2000);
 } } });
 let sure = false;
 const reset = el("button", { type: "button", text: tr("reset"), on: { click: () => { if (!sure) { sure = true; reset.textContent = tr("reset_sure"); return; } st = fresh(); save(); render(); } } });
 v.append(el("div", { class: "card" }, [el("h2", { text: tr("results") }), ...rows, el("p", { class: "small mute", text: tr("p_explain") }), el("p", { class: "small mute", text: tr("res_mushra") }),
  el("div", { class: "row" }, [dl]), el("p", { class: "small", text: tr("send") })]), el("div", { class: "card" }, [reset]));
}

for (const l of ["es", "en"]) $("lang-" + l).addEventListener("click", () => { lang = l; save(); render(); });
document.addEventListener("keydown", e => {
 if (e.target && e.target.tagName === "INPUT" && e.target.type === "text") return;
 if (e.key === " ") { e.preventDefault(); player.stop(); }
});
render();
