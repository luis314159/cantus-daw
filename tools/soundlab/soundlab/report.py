"""The report: JSON (everything, for tools and later runs) and a readable HTML page (the
summary, before -> now, a strip plot per scalar feature and small multiples per vector)."""
import html
import json
import time
from collections import Counter

import numpy as np

from .band import bands, summarise, compare


def build(subject, ref_rows, cand_rows, run, before_rows=None, before_run=None, min_notes=3):
    ex = set(subject.d.get("exclude", []))

    def keep(rows):
        return [dict(r, f={k: v for k, v in r["f"].items() if k not in ex}) for r in rows]
    held = keep([r for r in ref_rows if r["held_out"]])
    cand = keep([r for r in cand_rows if r["held_out"]])
    b = bands(held, cand, min_notes)
    s = summarise(b)
    rep = {"subject": subject.id, "name": subject.d.get("name"), "run": run,
           "made": time.strftime("%Y-%m-%d %H:%M"), "split": subject.split,
           "references": {"files": len({r["id"].split("#")[0] for r in ref_rows}), "notes": len(ref_rows),
                          "held_out_notes": len(held),
                          "by_source": dict(Counter(f"{r['source']} ({r['licence']})" for r in ref_rows)),
                          "instruments": sorted({r["inst"] for r in ref_rows})},
           "candidate": {"notes": len(cand_rows), "held_out_notes": len(cand),
                         "cells_outside_split": len(cand_rows) - len(cand)},
           "groups": b, "summary": s}
    if before_rows is not None:
        bb = bands(held, keep([r for r in before_rows if r["held_out"]]), min_notes)
        rep["before"] = {"run": before_run, "summary": summarise(bb), "groups": bb}
        rep["compare"] = compare(s, rep["before"]["summary"])
    return rep


def write(rep, out_dir, subject):
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "report.json").write_text(json.dumps(rep, indent=1), encoding="utf-8")
    (out_dir / "report.html").write_text(page(rep, subject), encoding="utf-8")
    return out_dir / "report.html"


# ---- the page

CSS = """
:root{--bg:#f6f5f2;--card:#fcfcfb;--ink:#0b0b0b;--ink2:#52514e;--mute:#8a8984;--line:#dedcd6;--band:#d9e3f1;--ref:#9b9a95;--s1:#2a78d6;--s2:#eb6834;--in:#0f7b3e;--out:#b3261e}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#121211;--card:#1a1a19;--ink:#f0efec;--ink2:#c3c2b7;--mute:#8f8e86;--line:#34332f;--band:#24344a;--ref:#77766f;--s1:#3987e5;--s2:#d95926;--in:#4fc27f;--out:#f07a72}}
:root[data-theme="dark"]{--bg:#121211;--card:#1a1a19;--ink:#f0efec;--ink2:#c3c2b7;--mute:#8f8e86;--line:#34332f;--band:#24344a;--ref:#77766f;--s1:#3987e5;--s2:#d95926;--in:#4fc27f;--out:#f07a72}
*{box-sizing:border-box}html,body{background:var(--bg)}body{margin:0;color:var(--ink);font:15px/1.5 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
main{max-width:1100px;margin:0 auto;padding:20px 16px 60px}h1{font-size:22px;margin:0 0 4px}h2{font-size:18px;margin:28px 0 8px}h3{font-size:15px;margin:18px 0 6px}
.card{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:14px 16px;margin:12px 0}
.mute{color:var(--ink2)}.small{font-size:13px}table{border-collapse:collapse;width:100%;font-size:13px}
td,th{text-align:left;padding:5px 8px;border-bottom:1px solid var(--line);vertical-align:top;font-variant-numeric:tabular-nums}
.scroll{overflow-x:auto}.in{color:var(--in)}.out{color:var(--out)}svg{display:block;max-width:100%;height:auto}
svg text{fill:var(--ink2);font:11px system-ui,sans-serif}.legend span{display:inline-flex;align-items:center;gap:6px;margin-right:16px;font-size:13px}
.sw{width:14px;height:10px;border-radius:2px;display:inline-block}.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(250px,1fr));gap:10px}
"""


def _f(v, d=2):
    if v is None:
        return "—"
    if abs(v) >= 1000:
        return f"{v:.0f}"
    if abs(v) < 0.01 and v != 0:
        return f"{v:.2e}"
    return f"{v:.{d}f}"


def _strip(key, groups, before):
    """One row per group: the band (p10-p90), the references' notes, the candidate's median
    (and the earlier run's)."""
    rows = [(g, d[key]) for g, d in groups.items() if key in d and "band" in d[key]]
    if not rows:
        return ""
    vals = []
    for g, c in rows:
        vals += [c["band"]["p10"], c["band"]["p90"], c["place"]["value"]] + c.get("refs", [])
        bc = (before or {}).get(g, {}).get(key)
        if bc and "place" in bc:
            vals.append(bc["place"]["value"])
    lo, hi = np.percentile(vals, 1), np.percentile(vals, 99)
    lo, hi = min(lo, min(v for _, c in rows for v in (c["band"]["p10"], c["place"]["value"]))), \
        max(hi, max(v for _, c in rows for v in (c["band"]["p90"], c["place"]["value"])))
    pad = 0.05 * (hi - lo or 1)
    lo, hi = lo - pad, hi + pad
    W, L, R, H = 760, 150, 20, 26
    x = lambda v: L + (W - L - R) * (min(max(v, lo), hi) - lo) / (hi - lo)  # noqa: E731
    out = [f'<svg viewBox="0 0 {W} {H * len(rows) + 30}" role="img" aria-label="{html.escape(key)}">']
    for i, (g, c) in enumerate(rows):
        y = 10 + i * H
        b = c["band"]
        out.append(f'<text x="{L - 8}" y="{y + 13}" text-anchor="end">{html.escape(g)}</text>')
        out.append(f'<rect x="{x(b["p10"]):.1f}" y="{y + 3}" width="{max(x(b["p90"]) - x(b["p10"]), 2):.1f}" height="14" rx="3" fill="var(--band)"><title>band p10 {_f(b["p10"])} · p50 {_f(b["p50"])} · p90 {_f(b["p90"])} ({b["n"]} notes, {b["n_inst"]} instruments)</title></rect>')
        for v in c.get("refs", [])[:200]:
            out.append(f'<circle cx="{x(v):.1f}" cy="{y + 10}" r="1.6" fill="var(--ref)" opacity="0.6"/>')
        bc = (before or {}).get(g, {}).get(key)
        if bc and "place" in bc:
            out.append(f'<circle cx="{x(bc["place"]["value"]):.1f}" cy="{y + 10}" r="5" fill="none" stroke="var(--s2)" stroke-width="2"><title>before {_f(bc["place"]["value"])} ({bc["place"]["status"]})</title></circle>')
        p = c["place"]
        out.append(f'<circle cx="{x(p["value"]):.1f}" cy="{y + 10}" r="5" fill="var(--s1)" stroke="var(--card)" stroke-width="2"><title>now {_f(p["value"])} ({p["status"]}, {_f(p["widths"])} band widths out; {c["n_cand"]} notes)</title></circle>')
    y = 10 + len(rows) * H + 4
    for t in np.linspace(lo, hi, 5):
        out.append(f'<text x="{x(t):.1f}" y="{y + 10}" text-anchor="middle">{_f(t)}</text>')
    out.append("</svg>")
    return "".join(out)


def _vector(key, groups, before):
    """Small multiples, one per group: the band per element, the candidate's medians."""
    cells = []
    for g, d in groups.items():
        c = d.get(key)
        if not c or "vector" not in c:
            continue
        els = c["vector"]
        pts = [(j, e["band"], e["place"]) for j, e in enumerate(els) if e["band"]]
        if len(pts) < 2:
            continue
        vs = [v for _, b, p in pts for v in (b["p10"], b["p90"])] + [p["value"] for _, _, p in pts if p]
        lo, hi = min(vs), max(vs)
        pad = 0.08 * (hi - lo or 1)
        lo, hi = lo - pad, hi + pad
        W, H, L, B = 250, 140, 34, 18
        n = len(els)
        x = lambda j: L + (W - L - 6) * j / max(n - 1, 1)  # noqa: E731
        y = lambda v: 6 + (H - B - 6) * (hi - v) / (hi - lo)  # noqa: E731
        top = " ".join(f"{x(j):.1f},{y(b['p90']):.1f}" for j, b, _ in pts)
        bot = " ".join(f"{x(j):.1f},{y(b['p10']):.1f}" for j, b, _ in reversed(pts))
        now = " ".join(f"{x(j):.1f},{y(p['value']):.1f}" for j, _, p in pts if p)
        s = [f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="{html.escape(key + " " + g)}">',
             f'<polygon points="{top} {bot}" fill="var(--band)"/>']
        bc = (before or {}).get(g, {}).get(key)
        if bc and "vector" in bc:
            bp = " ".join(f"{x(j):.1f},{y(e['place']['value']):.1f}" for j, e in enumerate(bc["vector"]) if e["place"])
            s.append(f'<polyline points="{bp}" fill="none" stroke="var(--s2)" stroke-width="2" stroke-dasharray="4 3"/>')
        s.append(f'<polyline points="{now}" fill="none" stroke="var(--s1)" stroke-width="2"/>')
        for j, b, p in pts:
            if p:
                s.append(f'<circle cx="{x(j):.1f}" cy="{y(p["value"]):.1f}" r="2.5" fill="var(--s1)"><title>{j + 1}: now {_f(p["value"])} · band {_f(b["p10"])} … {_f(b["p90"])}</title></circle>')
        for v in (lo + pad, hi - pad):
            s.append(f'<text x="{L - 4}" y="{y(v) + 4:.1f}" text-anchor="end">{_f(v, 1)}</text>')
        s.append(f'<text x="{L}" y="{H - 4}">1</text><text x="{W - 6}" y="{H - 4}" text-anchor="end">{n}</text>')
        s.append("</svg>")
        cells.append(f'<div><div class="small mute">{html.escape(g)} · RMS outside {_f(c["outside_rms"])}</div>{"".join(s)}</div>')
    return f'<div class="grid">{"".join(cells)}</div>' if cells else ""


def page(rep, subject):
    e = html.escape
    s, cmp_ = rep["summary"], rep.get("compare", {})
    before = rep.get("before", {}).get("groups")
    br = rep.get("before", {}).get("run")
    rows = []
    for k, v in s.items():
        if "in" in v:
            score = f'{v["in"]} of {v["groups"]} groups in the band · {_f(v["widths_mean"])} band widths out (mean)'
        else:
            score = f'{100 * v["in_share"]:.0f} % of elements in · RMS outside {_f(v["outside_rms_mean"])} (mean)'
        c = cmp_.get(k)
        bt = f'{_f(c["before"])} → {_f(c["now"])} <span class="{"in" if c["verdict"] == "closer" else ("out" if c["verdict"] == "further" else "mute")}">{c["verdict"]}</span>' if c else "—"
        rows.append(f"<tr><td>{e(k)}</td><td>{score}</td><td>{bt}</td></tr>")
    src = "".join(f"<tr><td>{e(k)}</td><td>{n}</td></tr>" for k, n in sorted(rep["references"]["by_source"].items()))
    legend = ('<p class="legend"><span><i class="sw" style="background:var(--band)"></i>real instruments, p10–p90 (held-out third)</span>'
              '<span><i class="sw" style="background:var(--ref);height:4px;border-radius:50%"></i>reference notes</span>'
              f'<span><i class="sw" style="background:var(--s1)"></i>{e(rep["run"])} (now)</span>'
              + (f'<span><i class="sw" style="background:var(--s2)"></i>{e(br)} (before)</span>' if br else "") + "</p>")
    plots = []
    for k, v in s.items():
        svg = _vector(k, rep["groups"], before) if "outside_rms_median" in v else _strip(k, rep["groups"], before)
        if svg:
            plots.append(f'<div class="card"><h3>{e(k)}</h3>{svg}</div>')
    return f"""<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Sound lab report</title><style>{CSS}</style></head><body><main>
<h1>{e(rep.get("name") or rep["subject"])} — sound lab report</h1>
<p class="mute">Run <b>{e(rep["run"])}</b>{f" against <b>{e(br)}</b>" if br else ""} · made {e(rep["made"])} · split {e(json.dumps(rep["split"].get("rule")))} {e(rep["split"].get("expr", ""))}{e(" seed " + str(rep["split"].get("seed"))) if rep["split"].get("seed") is not None else ""}</p>
<div class="card small"><b>How to read it.</b> The references are split by cell (articulation, note, dynamic) with the subject's seeded rule; the third held out is never used to fit the model. Per group (articulation · register octave · dynamic class) and per feature, the band is the held-out references' 10th–90th percentile, each physical instrument weighing one. The candidate's median in the same held-out cells is placed in the band or outside it: a scalar by how many band widths it lies out, a vector by the RMS of its elements' distances outside (in the feature's units). Lower is closer; 0 is inside.</div>
<div class="card"><h2 style="margin-top:0">Summary</h2><div class="scroll"><table><tr><th>feature</th><th>now</th><th>before → now</th></tr>{"".join(rows)}</table></div></div>
<div class="card small"><b>References</b>: {rep["references"]["files"]} files, {rep["references"]["notes"]} notes ({rep["references"]["held_out_notes"]} held out), {len(rep["references"]["instruments"])} instruments. Candidate: {rep["candidate"]["notes"]} notes ({rep["candidate"]["held_out_notes"]} in held-out cells).
<div class="scroll"><table><tr><th>source (licence)</th><th>notes</th></tr>{src}</table></div></div>
<h2>Every feature</h2>{legend}{"".join(plots)}
<p class="small mute">Hover a mark for its value. The JSON beside this page (report.json) holds every number.</p>
</main></body></html>"""
