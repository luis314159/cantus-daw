// Cantus's social-account images: one page, one asset per load (?asset=<name>&lang=en|es).
// Every word is here, in both languages, and every claim is one the site and the README
// make: free, open source, no account, no ads, the counts as web/src/counts.rs counts them
// (11 instrument cards and Firmus, 43 effects offered as inserts — the launch video's own).
"use strict";

// The assets: their element and their CSS size. render.mjs scales them to each platform.
const ASSETS = {
  avatar: { el: "avatar", w: 1080, h: 1080 },
  watermark: { el: "watermark", w: 150, h: 150 },
  banner: { el: "banner", w: 2560, h: 1440 },
  header: { el: "header", w: 1500, h: 500 },
  thumb: { el: "thumb", w: 1280, h: 720 },
};

const WORDS = {
  // the banner carries both languages: the channel speaks both
  "banner-en": { en: "A free, open-source DAW. No account, no ads.", es: "A free, open-source DAW. No account, no ads." },
  "banner-es": { en: "Un DAW libre y gratuito. Sin cuenta, sin anuncios.", es: "Un DAW libre y gratuito. Sin cuenta, sin anuncios." },
  os: { en: "Windows · Linux · macOS (preview)", es: "Windows · Linux · macOS (vista previa)" },
  // the site's own headline (web/src/top.rs)
  hero: { en: "Finish the song<i>.</i>", es: "Termina la canción<i>.</i>" },
  "header-line": {
    en: "A free, open-source DAW · 12 instruments · 43 effects",
    es: "Un DAW libre y gratuito · 12 instrumentos · 43 efectos",
  },
  big1: { en: "Free", es: "DAW" },
  big2: { en: "DAW", es: "gratis" },
  chip1: { en: "12 instruments · 43 effects", es: "12 instrumentos · 43 efectos" },
  chip2: { en: "No account · No ads", es: "Sin cuenta · Sin anuncios" },
  piano: { en: "Chorda · modelled grand", es: "Chorda · piano de cola modelado" },
};

const q = new URLSearchParams(location.search);
const name = q.get("asset") || "avatar";
const lang = q.get("lang") === "es" ? "es" : "en";
const asset = ASSETS[name];
if (!asset) throw new Error("no asset " + name);
// ?w=&h= draw the avatar straight at a platform's size (it fills its page, whatever the size)
if (name === "avatar" && q.get("w")) asset.w = asset.h = Number(q.get("w"));
document.documentElement.lang = lang;
document.documentElement.style.setProperty("--w", asset.w + "px");
document.documentElement.style.setProperty("--h", asset.h + "px");
document.body.classList.add("is-" + name, "lang-" + lang);
if (q.get("debug")) document.body.classList.add("debug");
const root = document.getElementById(asset.el);
root.classList.add("on");
for (const el of root.querySelectorAll("[data-t]")) {
  const w = WORDS[el.dataset.t];
  el.innerHTML = w ? w[lang] : "";
}
window.assetSize = { w: asset.w, h: asset.h };

// Ready when the fonts and every picture have loaded and decoded.
window.assetReady = (async () => {
  await document.fonts.ready;
  const imgs = [...root.querySelectorAll("img")];
  await Promise.all(imgs.map((i) => (i.complete ? Promise.resolve() : new Promise((ok) => { i.onload = ok; i.onerror = ok; }))));
  await Promise.all(imgs.map((i) => i.decode().catch(() => {})));
  const bad = imgs.filter((i) => !i.naturalWidth).map((i) => i.getAttribute("src"));
  if (bad.length) throw new Error("missing pictures: " + bad.join(", "));
  return true;
})();
