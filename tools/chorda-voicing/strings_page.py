"""Chorda's every-string page (round 41, BACKLOG §3e): the owner's view of every loop of
every key measured alone inside the instrument.

    python strings_page.py <strings.json> <out.html>

The JSON is what `chorda::tests_strings::strings_map` writes with `CHORDA_STRINGS_JSON` set
(`docs/round41/chorda-strings.md` says how). The page is self-contained: the data, the
styles and a small script inline, no external file of any kind.
"""

import json
import sys

PAGE = r"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Chorda string tuning</title>
<style>
:root {
  --bg: #f7f7f5; --panel: #ffffff; --ink: #1d1f22; --muted: #5d636b; --rule: #d9dbde;
  --band: #2e7d4f22; --band-edge: #2e7d4f88; --zero: #8a9099;
  --s1: #1f5fbf; --s2: #c2410c; --s3: #7c3aed; --polar: #0f766e; --bad: #b91c1c;
  --bad-bg: #fde8e8;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #15171a; --panel: #1d2024; --ink: #e8eaed; --muted: #a3a9b1; --rule: #33373d;
    --band: #4ade8022; --band-edge: #4ade8088; --zero: #6b7280;
    --s1: #7aa7ff; --s2: #fb923c; --s3: #c4a5ff; --polar: #2dd4bf; --bad: #f87171;
    --bad-bg: #3b1d1d;
  }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--ink);
  font: 15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; }
main { max-width: 1180px; margin: 0 auto; padding: 24px 16px 64px; }
h1 { font-size: 26px; margin: 0 0 4px; }
h2 { font-size: 18px; margin: 32px 0 8px; }
p { margin: 6px 0; color: var(--muted); max-width: 78ch; }
.tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
  gap: 12px; margin: 20px 0; }
.tile { background: var(--panel); border: 1px solid var(--rule); border-radius: 8px;
  padding: 12px 14px; }
.tile .label { font-size: 13px; color: var(--muted); }
.tile .value { font-size: 22px; font-variant-numeric: tabular-nums; }
.panel { background: var(--panel); border: 1px solid var(--rule); border-radius: 8px;
  padding: 12px; overflow-x: auto; }
.controls { display: flex; flex-wrap: wrap; gap: 8px; margin: 8px 0 12px; align-items: center; }
.controls span { color: var(--muted); font-size: 13px; margin-right: 4px; }
button { font: inherit; font-size: 13px; color: var(--ink); background: var(--bg);
  border: 1px solid var(--rule); border-radius: 6px; padding: 3px 10px; cursor: pointer; }
button[aria-pressed="true"] { border-color: var(--ink); font-weight: 600; }
.legend { display: flex; flex-wrap: wrap; gap: 16px; font-size: 13px; color: var(--muted);
  margin-top: 6px; }
.dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%;
  margin-right: 5px; vertical-align: -1px; }
svg text { fill: var(--muted); font-size: 11px; }
table { border-collapse: collapse; width: 100%; font-variant-numeric: tabular-nums;
  font-size: 13px; }
th, td { padding: 3px 8px; border-bottom: 1px solid var(--rule); text-align: right;
  white-space: nowrap; }
th { color: var(--muted); font-weight: 600; position: sticky; top: 0; background: var(--panel); }
td.l, th.l { text-align: left; }
td.bad { color: var(--bad); background: var(--bad-bg); font-weight: 600; }
tr.key td { border-top: 2px solid var(--rule); }
.scroll { max-height: 640px; overflow: auto; }
</style>
</head>
<body>
<main>
<h1>Chorda: every string alone</h1>
<p>Each string of each key A0 to C8 is struck alone inside the instrument at velocities 0.3,
0.6 and 0.9. The other strings of the key are muted the way a piano technician mutes them
with felt: they stay on the bridge but do not sound. The horizontal polarisation loop of the
one- and two-string keys is measured the same way. Each string is compared with the pitch it
was designed to have: the keyboard's curve plus that string's designed unison offset.
These are measurements of the model. Nobody has listened to them yet.</p>
<div class="tiles" id="tiles"></div>

<h2>Error against the designed pitch</h2>
<p>The ear's pitch of each string alone, in cents. Zero means the string is exactly where it
was designed to be. The green band is the tolerance the tests hold that string to.</p>
<div class="panel">
  <div class="controls" id="controls"></div>
  <svg id="chart" role="img" aria-label="Error in cents of every string, key by key"></svg>
  <div class="legend">
    <span><i class="dot" style="background:var(--s1)"></i>string 1</span>
    <span><i class="dot" style="background:var(--s2)"></i>string 2</span>
    <span><i class="dot" style="background:var(--s3)"></i>string 3</span>
    <span><i class="dot" style="background:var(--polar)"></i>horizontal polarisation</span>
    <span><i class="dot" style="background:var(--band-edge)"></i>tolerance</span>
  </div>
</div>

<h2>Every key, every string</h2>
<p>Designed offset: the string's designed detune from the curve, in cents. Error columns:
the ear's pitch of that string alone minus its designed pitch, one per velocity. Partial 1:
the fundamental's error at velocity 0.6. Predicted: what the bridge's pull and the
technician's table account for at velocity 0.6. Red cells are outside the tolerance.</p>
<div class="panel scroll"><table id="table"></table></div>
</main>
<script>
const DATA = __DATA__;
const NAMES = { s1: "string 1", s2: "string 2", s3: "string 3", polar: "polarisation" };
const COLOUR = { s1: "var(--s1)", s2: "var(--s2)", s3: "var(--s3)", polar: "var(--polar)" };
const NOTE = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const noteName = m => NOTE[m % 12] + (Math.floor(m / 12) - 1);
const fmt = (x, d = 2) => x === null || x === undefined ? "–"
  : (x >= 0 ? "+" : "−") + Math.abs(x).toFixed(d);
let vel = "all";

function tiles() {
  const n = DATA.length;
  const out = DATA.filter(r => Math.abs(r.err) > r.tol).length;
  const worst = DATA.reduce((a, r) => Math.abs(r.err) > Math.abs(a.err) ? r : a, DATA[0]);
  const loops = new Set(DATA.map(r => r.key + r.loop)).size;
  const items = [
    ["Strings measured", loops + " (" + n + " strikes)"],
    ["Outside tolerance", out + " of " + n],
    ["Worst error", fmt(worst.err) + " ct"],
    ["Worst string", noteName(worst.key) + " " + NAMES[worst.loop] + " at " + worst.vel],
  ];
  document.getElementById("tiles").innerHTML = items.map(([l, v]) =>
    `<div class="tile"><div class="label">${l}</div><div class="value">${v}</div></div>`
  ).join("");
}

function controls() {
  const el = document.getElementById("controls");
  const opts = ["all", "0.3", "0.6", "0.9"];
  el.innerHTML = "<span>Velocity</span>" + opts.map(o =>
    `<button data-v="${o}" aria-pressed="${o === vel}">${o === "all" ? "all three" : o}</button>`
  ).join("");
  el.querySelectorAll("button").forEach(b => b.onclick = () => {
    vel = b.dataset.v; controls(); chart();
  });
}

function chart() {
  const svg = document.getElementById("chart");
  const W = 1120, H = 340, L = 40, R = 10, T = 10, B = 34;
  const rows = DATA.filter(r => vel === "all" || r.vel.toFixed(1) === vel);
  const lim = Math.max(2, Math.ceil(Math.max(...DATA.map(r => Math.max(Math.abs(r.err), r.tol))) * 2) / 2);
  const x = m => L + (m - 20.5) / 88 * (W - L - R);
  const y = c => T + (lim - c) / (2 * lim) * (H - T - B);
  let s = "";
  // The tolerance, key by key.
  const tol = {};
  DATA.forEach(r => tol[r.key] = Math.max(tol[r.key] || 0, r.tol));
  let top = "", bot = "";
  for (let m = 21; m <= 108; m++) {
    const t = tol[m] || 0;
    top += `${x(m - 0.5)},${y(t)} ${x(m + 0.5)},${y(t)} `;
    bot = `${x(m + 0.5)},${y(-t)} ${x(m - 0.5)},${y(-t)} ` + bot;
  }
  s += `<polygon points="${top}${bot}" fill="var(--band)" stroke="var(--band-edge)" stroke-width="1"/>`;
  for (let c = -lim; c <= lim + 1e-9; c += lim > 3 ? 1 : 0.5) {
    s += `<line x1="${L}" x2="${W - R}" y1="${y(c)}" y2="${y(c)}" stroke="var(--rule)"/>`;
    s += `<text x="${L - 6}" y="${y(c) + 4}" text-anchor="end">${fmt(c, 1)}</text>`;
  }
  s += `<line x1="${L}" x2="${W - R}" y1="${y(0)}" y2="${y(0)}" stroke="var(--zero)"/>`;
  for (let m = 21; m <= 108; m++) {
    if (m % 12 === 0 || m === 21) {
      s += `<text x="${x(m)}" y="${H - 12}" text-anchor="middle">${noteName(m)}</text>`;
      s += `<line x1="${x(m)}" x2="${x(m)}" y1="${T}" y2="${H - B}" stroke="var(--rule)" stroke-dasharray="2 3"/>`;
    }
  }
  s += `<text x="${L}" y="${H - 1}">key</text>`;
  const off = { s1: -0.27, s2: -0.09, s3: 0.09, polar: 0.27 };
  rows.forEach(r => {
    const bad = Math.abs(r.err) > r.tol;
    const cy = y(Math.max(-lim, Math.min(lim, r.err)));
    s += `<circle cx="${x(r.key + off[r.loop])}" cy="${cy}" r="${bad ? 4 : 2.6}" fill="${COLOUR[r.loop]}"`
      + (bad ? ` stroke="var(--bad)" stroke-width="2"` : "")
      + `><title>${noteName(r.key)} (${r.key}) ${NAMES[r.loop]}, velocity ${r.vel}: `
      + `${fmt(r.err)} cents (tolerance ±${r.tol.toFixed(2)})</title></circle>`;
  });
  svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
  svg.setAttribute("width", W);
  svg.style.maxWidth = "none";
  svg.innerHTML = s;
}

function table() {
  const by = {};
  DATA.forEach(r => { (by[r.key + "|" + r.loop] = by[r.key + "|" + r.loop] || []).push(r); });
  let h = `<tr><th class="l">Key</th><th class="l">String</th><th>Designed offset</th>`
    + `<th>Error 0.3</th><th>Error 0.6</th><th>Error 0.9</th><th>Partial 1 (0.6)</th>`
    + `<th>Predicted (0.6)</th><th>Tolerance</th></tr>`;
  let last = 0;
  Object.values(by).forEach(rs => {
    rs.sort((a, b) => a.vel - b.vel);
    const r6 = rs.find(r => r.vel.toFixed(1) === "0.6") || rs[0];
    const p1 = (r6.partials.find(p => p[0] === 1) || [])[1];
    const cell = r => `<td class="${Math.abs(r.err) > r.tol ? "bad" : ""}">${fmt(r.err)}</td>`;
    h += `<tr class="${r6.key !== last ? "key" : ""}"><td class="l">${r6.key !== last
      ? noteName(r6.key) + " (" + r6.key + ")" : ""}</td><td class="l">${NAMES[r6.loop]}</td>`
      + `<td>${fmt(r6.detune)}</td>${rs.map(cell).join("")}<td>${fmt(p1)}</td>`
      + `<td>${fmt(r6.pred)}</td><td>±${r6.tol.toFixed(2)}</td></tr>`;
    last = r6.key;
  });
  document.getElementById("table").innerHTML = h;
}

tiles(); controls(); chart(); table();
</script>
</body>
</html>
"""


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    with open(sys.argv[1], encoding="utf-8") as f:
        data = json.load(f)
    for r in data:
        if "tol" not in r:
            sys.exit("the JSON has no tolerances: write it with the round-41 diagnostic")
    page = PAGE.replace("__DATA__", json.dumps(data, separators=(",", ":")))
    with open(sys.argv[2], "w", encoding="utf-8", newline="\n") as f:
        f.write(page)


if __name__ == "__main__":
    main()
