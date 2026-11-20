// The launch video's cards, moved by one time variable (docs/round41/video/README.md §2).
// Nothing here reads the clock: window.cardSetTime(t) sets every moving value for the time
// t (seconds into the card), so render.mjs can step it frame by frame and every frame is
// exact. CSS custom property --t on :root mirrors t, for anything that wants it.
"use strict";

// The cards: the element they use, how long they run (s), and their words.
const CARDS = {
  "title": { el: "title", dur: 6.0 },
  "count-instruments": { el: "count", dur: 4.5, n: 11, noun: { en: "instruments", es: "instrumentos" } },
  "count-effects": { el: "count", dur: 4.5, n: 42, noun: { en: "effects", es: "efectos" } },
  "lower-record": { el: "lower", dur: 2.6, what: { en: "Recording", es: "Grabación" } },
  "lower-session": { el: "lower", dur: 2.6, what: { en: "Session view", es: "Vista de sesión" } },
  "lower-plugins": { el: "lower", dur: 2.6, what: { en: "CLAP and VST3 plug-ins", es: "Plug-ins CLAP y VST3" } },
  "free": { el: "free", dur: 5.0, a: { en: "Free", es: "Gratis" }, b: { en: "open source", es: "código abierto" } },
  "end-a": { el: "end", dur: 12.0, verse: true },
  "end-b": { el: "end", dur: 12.0, verse: false },
};

// The end card's words. The verse is the app's own translation (locales/verses: the World
// English Bible and the Reina-Valera 1909, both public domain), its first clause.
const END = {
  os: {
    en: "<b>Windows</b> · <b>Linux</b> — macOS next",
    es: "<b>Windows</b> · <b>Linux</b> — macOS en camino",
  },
  btn: { en: "Download free", es: "Descarga gratis" },
  verse: {
    en: ["“And whatever you do, work heartily, as for the Lord”", "Colossians 3:23"],
    es: ["«Y todo lo que hagáis, hacedlo de ánimo, como al Señor»", "Colosenses 3:23"],
  },
};

const q = new URLSearchParams(location.search);
const name = q.get("card") || "title";
const lang = q.get("lang") === "es" ? "es" : "en";
const card = CARDS[name];
if (!card) throw new Error("no card " + name);
if (q.get("dur")) card.dur = Number(q.get("dur"));
document.documentElement.lang = lang;
if (q.get("debug")) document.body.classList.add("debug");
const root = document.getElementById(card.el);
root.classList.add("on");
const $ = (sel) => root.querySelector(sel);

// --- time helpers ---
const clamp = (x) => Math.min(1, Math.max(0, x));
// cubic in and out, as the film's camera and cursor
const ease = (x) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2);
// cubic out, for what arrives and settles
const out = (x) => 1 - Math.pow(1 - x, 3);
/** 0 before `start`, 1 after `start + dur`, eased between. */
const p = (t, start, dur, f = ease) => f(clamp((t - start) / dur));
const mix = (a, b, k) => a + (b - a) * k;
function set(el, { o, x = 0, y = 0, s = 1, sx, sy }) {
  if (!el) return;
  if (o !== undefined) el.style.opacity = o.toFixed(4);
  const scale = sx !== undefined || sy !== undefined ? `scale(${(sx ?? 1).toFixed(4)}, ${(sy ?? 1).toFixed(4)})` : `scale(${s.toFixed(4)})`;
  el.style.transform = `translate(${x.toFixed(2)}px, ${y.toFixed(2)}px) ${scale}`;
}
/** A stroke drawn on from 0 to 1 of its length. */
function draw(el, k) {
  const len = el._len || (el._len = el.getTotalLength());
  el.style.strokeDasharray = `${len} ${len}`;
  el.style.strokeDashoffset = (len * (1 - k)).toFixed(2);
  // a round cap on a zero-length dash would draw a dot before the stroke starts
  el.style.opacity = k > 0 ? "1" : "0";
}

// --- logo #03, read from the app's own asset and inlined, so its parts can move ---
async function inlineLogo(holder) {
  const svg = await (await fetch("/crates/cantus-ui/assets/logos/03.svg")).text();
  holder.innerHTML = svg;
  const s = holder.querySelector("svg");
  s.removeAttribute("width");
  s.removeAttribute("height");
  const kids = [...s.children];
  const parts = {
    // the lapis tile: its shadowed square, its dots, its gloss and its edge
    tile: kids.filter((k) => k.matches('g[filter="url(#sh)"], rect[fill="url(#dots)"], rect[fill="url(#gloss)"], rect[stroke="#ffffff"]')),
    border: s.querySelector('rect[stroke="url(#leaf)"]'),
    flourish: [...s.querySelectorAll('g[stroke="url(#leaf)"] path')],
    seeds: [...s.querySelectorAll('g[fill="url(#leaf)"] circle')],
    c: s.querySelector('g[filter="url(#emb)"]'),
    shine: s.querySelector('path[stroke="#FFF8DA"]'),
    bars: [...s.querySelectorAll('g[fill="url(#verm)"] rect')],
  };
  for (const el of [parts.c, ...parts.bars]) {
    el.style.transformBox = "fill-box";
    el.style.transformOrigin = "50% 50%";
  }
  for (const el of parts.tile) {
    el.style.transformOrigin = "512px 512px";
  }
  return parts;
}

/** The logo drawn on from `t0`: the tile, the gold border, the C, the waveform, the flourishes. */
function logoAt(L, t, t0) {
  const tile = p(t, t0, 0.8, out);
  for (const el of L.tile) set(el, { o: tile, s: mix(0.92, 1, tile) });
  draw(L.border, p(t, t0 + 0.3, 1.4));
  L.flourish.forEach((el, i) => draw(el, p(t, t0 + 0.9 + i * 0.12, 0.9)));
  L.seeds.forEach((el, i) => set(el, { o: p(t, t0 + 1.5 + i * 0.12, 0.4) }));
  const c = p(t, t0 + 0.7, 0.8, out);
  set(L.c, { o: c, s: mix(0.95, 1, c) });
  set(L.shine, { o: p(t, t0 + 1.3, 0.5) });
  L.bars.forEach((el, i) => {
    const k = p(t, t0 + 1.1 + i * 0.06, 0.5, out);
    set(el, { o: k, sx: 1, sy: mix(0.05, 1, k) });
  });
}

// --- each card's motion ---
const MOVES = {
  title(t, d, L) {
    const gone = 1 - p(t, d - 1.2, 1.0);
    set($(".scrim"), { o: p(t, 0, 0.8) * gone });
    const logo = $(".logo");
    logo.style.opacity = gone.toFixed(4);
    logoAt(L, t, 0.2);
    const w = p(t, 1.8, 0.8, out);
    set($(".word"), { o: w * gone, y: mix(16, 0, w) });
    set($(".rule"), { o: gone, sx: p(t, 2.3, 1.0), sy: 1 });
  },
  count(t, d) {
    const inn = p(t, 0, 0.5, out);
    const gone = p(t, d - 0.6, 0.6);
    set($(".panel"), { o: inn * (1 - gone), x: mix(-28, 0, inn) - 12 * gone });
    $(".panel").style.setProperty("--rule", p(t, 0.25, 0.7).toFixed(4));
    $(".n").textContent = String(Math.round(card.n * p(t, 0.2, 0.9, out)));
  },
  lower(t, d) {
    const inn = p(t, 0, 0.35, out);
    const gone = p(t, d - 0.4, 0.4);
    set($(".panel"), { o: inn * (1 - gone), x: mix(-20, 0, inn) });
  },
  free(t, d) {
    const gone = 1 - p(t, d - 0.7, 0.7);
    set($(".scrim"), { o: p(t, 0, 0.6) * gone });
    const k = p(t, 0.2, 0.7, out);
    set($(".line1"), { o: k * gone, y: mix(20, 0, k) });
    set($(".rule"), { o: gone, sx: p(t, 0.6, 0.8), sy: 1 });
  },
  end(t, d, L) {
    set($(".ground"), { o: p(t, 0, 0.6) });
    logoAt(L, t, 0.3);
    const rise = (sel, t0) => {
      const k = p(t, t0, 0.6, out);
      set($(sel), { o: k, y: mix(18, 0, k) });
    };
    const w = p(t, 0.5, 0.7, out);
    set($(".brand .word"), { o: w, x: mix(24, 0, w) });
    rise(".site", 0.9);
    rise(".os", 1.1);
    rise(".btn", 1.3);
    if (card.verse) {
      set($(".verse .rule"), { sx: p(t, 1.9, 1.0), sy: 1 });
      rise(".verse q", 2.3);
      rise(".verse cite", 2.5);
    }
  },
};

// --- the words ---
function fill() {
  if (card.el === "count") {
    $(".noun").textContent = card.noun[lang];
  } else if (card.el === "lower") {
    $(".what").textContent = card.what[lang];
  } else if (card.el === "free") {
    $(".line1").innerHTML = `${card.a[lang]}<span class="dot">·</span>${card.b[lang]}`;
  } else if (card.el === "end") {
    $(".os").innerHTML = END.os[lang];
    $(".btn .label").textContent = END.btn[lang];
    if (card.verse) {
      $(".verse").hidden = false;
      $(".verse q").textContent = END.verse[lang][0];
      $(".verse cite").textContent = END.verse[lang][1];
    }
  }
}

let logo = null;
window.CARDS = Object.fromEntries(Object.entries(CARDS).map(([k, v]) => [k, { dur: v.dur }]));
window.cardDuration = card.dur;
window.cardSetTime = (t) => {
  document.documentElement.style.setProperty("--t", String(t));
  MOVES[card.el](t, card.dur, logo);
};
window.cardReady = (async () => {
  fill();
  const holder = $("[data-logo]");
  if (holder) logo = await inlineLogo(holder);
  await document.fonts.ready;
  window.cardSetTime(Number(q.get("t") || 0));
})();
