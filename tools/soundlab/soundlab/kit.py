"""The blind kit from a subject file: any instrument's clips on the shared page
(`crates/cantus-ui/examples/abx_kit/abx.html`, the one Chorda's and Tympanum's generators fill),
with the guarantees of `docs/round40/abx5.md`:

- every item's clips cut to one length, faded out, brought to one loudness (ITU-R BS.1770,
  `target_lufs`), lowered together when a peak would pass `ceiling_db` (`abx_kit/kit.rs`'s
  `match_group`, ported: the same two passes and the same final correction);
- every file named by 64 random bits (the OS's CSPRNG), redrawn until no two names share
  structure (`names.rs`'s rule: > 6 leading or > 9 positional hex digits);
- sources known to the page only by random tokens, the table veiled (`names.rs`'s `veil`);
- catch questions (one clip under two names), a MUSHRA hidden reference (the reference's
  clip under another name) and anchors (the reference low-passed at 3.5 and 7 kHz,
  ITU-R BS.1534-3 §5.1);
- the kit folder holds only what a listener receives (index.html, audio/, LEEME.md,
  README.md); the key (`<kit>-key/key.json`, `levels.csv`) is written beside it, never in it.

The subject's `listening` section:

```json
{"page": { ...the page's subject: texts es/en, items' names, credits... },
 "kit_prefix": "kit1", "items": ["phrase", "note_p"],
 "voices": {"cand": {"render": [["cmd", "{out}"]], "files": {"phrase": "a.wav"}},
            "recording": {"dir": "$SOUNDLAB_REFS/x", "files": {...}}},
 "parts": ["abx", "mushra", "pref"], "abx": ["cand", "recording"], "extra": null,
 "pref": [["cand", "recording"]], "catch": 2,
 "mushra": {"reference": "recording", "conditions": ["cand"], "anchors": [3500, 7000]},
 "target_lufs": -23, "ceiling_db": -1, "fade_s": 0.1, "pre_roll_s": 0.08, "max_s": 12,
 "readme": {"es": "...", "en": "..."}}
```
"""
import json
import secrets
import shutil
import subprocess
import time
from pathlib import Path

import numpy as np
from scipy import signal

from . import audio
from .loudness import lufs
from .subject import REPO, expand

PAGE = REPO / "crates" / "cantus-ui" / "examples" / "abx_kit" / "abx.html"
PLACEHOLDERS = ("__KIT__", "__PHRASES__", "__FILES__", "__TESTS__", "__SEED__", "__REVEAL__", "__SUBJECT__")
SR = 48000
PREFIX, SAME = 6, 9
ANCHOR_NAMES = {"es": "Ancla: la referencia con paso bajo a {k} kHz", "en": "Anchor: the reference low-passed at {k} kHz"}


# ---- blinding (names.rs)

def name():
    return secrets.token_hex(8)


def token():
    return secrets.token_hex(4)


def names_look_random(names):
    """None, or why the names carry structure (names.rs's `names_look_random`)."""
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            if a == b:
                return f"{a} twice"
            prefix = next((k for k, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
            same = sum(x == y for x, y in zip(a, b))
            if prefix > PREFIX or same > SAME:
                return f"{a} and {b} share {same} digits ({prefix} leading)"
    return None


def draw_names(n):
    while True:
        v = [name() for _ in range(n)]
        if names_look_random(v) is None:
            return v


def _xorshift_stream(data, seed):
    x = max(seed & 0xFFFFFFFF, 1)
    out = bytearray()
    for b in data:
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        out.append(b ^ (x >> 24))
    return bytes(out)


def veil(text, seed):
    """names.rs's `veil`: the text's bytes XOR a xorshift32 stream, in base64."""
    import base64
    return base64.b64encode(_xorshift_stream(text.encode("utf-8"), seed)).decode("ascii")


def unveil(b64, seed):
    """The page's `unveil`, for the tests and the key's own check."""
    import base64
    return _xorshift_stream(base64.b64decode(b64), seed).decode("utf-8")


# ---- the clips (kit.rs's match_group)

def onset(x, sr):
    """The first 2 ms frame over max(peak - 40, floor + 6) dB (the lab's onset rule)."""
    m = np.abs(x).max(axis=1)
    h = int(0.002 * sr)
    e = np.array([np.sqrt(np.mean(m[i:i + 2 * h] ** 2) + 1e-20) for i in range(0, max(len(m) - 2 * h, 1), h)])
    d = 20 * np.log10(e)
    i = int(np.argmax(d > max(np.percentile(d, 99) - 40, np.percentile(d, 3) + 6)))
    return i * h


def prepare(x, sr, pre_roll_s=None, max_s=None):
    """Stereo at 48 kHz, its first onset moved to pre_roll_s (silence added or cut), cut to max_s."""
    if sr != SR:
        x = audio.resample(x, sr, SR)
    if x.shape[1] == 1:
        x = np.repeat(x, 2, axis=1)
    if pre_roll_s is not None:
        on = onset(x, SR)
        want = int(pre_roll_s * SR)
        x = np.concatenate([np.zeros((want - on, 2)), x]) if on < want else x[on - want:]
    if max_s:
        x = x[: int(max_s * SR)]
    return np.ascontiguousarray(x)


def match_group(clips, fade_s, target, ceiling):
    """One item's clips cut to the shortest, faded out over fade_s, brought to `target` LUFS
    (lowered together if a peak would pass `ceiling` dBFS): (clips, target, before, gains)."""
    n = min(len(c) for c in clips)
    f = int(fade_s * SR)
    ramp = np.ones(n)
    if f:
        ramp[n - min(f, n):] = (np.arange(min(f, n), 0, -1) - 1) / f
    clips = [c[:n] * ramp[:, None] for c in clips]
    db = lambda v: 20 * np.log10(max(v, 1e-9))  # noqa: E731
    before = [(lufs(c, SR), db(np.abs(c).max())) for c in clips]
    target = min([target] + [lu + (ceiling - pk) for lu, pk in before])
    gains = []
    for i, (c, (lu, _)) in enumerate(zip(clips, before)):
        g = target - lu
        c = c * 10 ** (g / 20)
        again = target - lufs(c, SR)
        clips[i] = c * 10 ** (again / 20)
        gains.append(g + again)
    over = max(db(np.abs(c).max()) for c in clips) - ceiling
    if over > 0:
        clips = [c * 10 ** (-(over + 0.005) / 20) for c in clips]
        gains = [g - (over + 0.005) for g in gains]
    return clips, target, before, gains


def anchor(x, hz):
    """A low-passed copy (12th-order Butterworth, zero phase): BS.1534-3's anchors."""
    sos = signal.butter(12, hz, btype="lowpass", fs=SR, output="sos")
    return signal.sosfiltfilt(sos, x, axis=0)


# ---- the voices' files

def voice_files(subject, L, run, log):
    """{voice: {item: path}}: each voice's folder, or its render commands run into
    <work>/runs/<run>/listen/<voice> (once; a rerun reuses them)."""
    out = {}
    for v, spec in L["voices"].items():
        if spec.get("render"):
            if not run:
                raise SystemExit(f"voice {v} is rendered: pass --run <run>")
            d = subject.run_dir(run) / "listen" / v
            if not all((d / f).exists() for f in spec["files"].values()):
                d.mkdir(parents=True, exist_ok=True)
                for cmd in spec["render"]:
                    c = [expand(a, subject).replace("{out}", str(d)) for a in cmd]
                    log("render: " + " ".join(c))
                    subprocess.run(c, cwd=expand(spec.get("cwd", "{repo}"), subject), check=True)
        else:
            d = Path(expand(spec["dir"], subject))
        out[v] = {it: d / f for it, f in spec["files"].items()}
        for it, p in out[v].items():
            if not p.exists():
                raise SystemExit(f"voice {v}, item {it}: {p} is missing")
    return out


# ---- the kit

def page_subject(L):
    """The page's subject with the parts and the anchors' names (when the subject gave none)."""
    p = json.loads(json.dumps(L["page"]))
    p["parts"] = L.get("parts", ["abx", "pref"])
    m = L.get("mushra")
    for hz in (m or {}).get("anchors", []):
        for lang in ("es", "en"):
            k = f"voice_anchor{int(hz // 100)}"
            p["text"][lang].setdefault(k, ANCHOR_NAMES[lang].format(k=f"{hz / 1000:g}".replace(".", "," if lang == "es" else ".")))
    for k in ("id", "play", "trials", "secs", "pairs", "items", "credits", "text"):
        if k not in p:
            raise SystemExit(f"the page's subject names no {k}")
    return p


def build(subject, out, flac=True, twin=None, run=None, log=print):
    L = subject.d["listening"]
    out = Path(out)
    key_dir = Path(str(out) + "-key")
    ext = "flac" if flac else "wav"
    items = L["items"]
    page = page_subject(L)
    for it in items:
        for lang in ("es", "en"):
            if it not in page["items"][lang]:
                raise SystemExit(f"item {it} has no {lang} name in the page's subject")
    files = voice_files(subject, L, run, log)
    voices = list(L["voices"])
    m = L.get("mushra")
    anchors = [f"anchor{int(hz // 100)}" for hz in (m or {}).get("anchors", [])]
    # tokens: one per voice and anchor; one more per catch question and the hidden reference
    tok = {v: token() for v in voices + anchors}
    n_catch = int(L.get("catch", 2)) if "pref" in page["parts"] else 0
    abx = L.get("abx") or page["pairs"][0]
    catches = []
    order = items[:]
    secrets.SystemRandom().shuffle(order)
    for i in range(n_catch):
        v = abx[i % 2]
        catches.append({"phrase": order[i % len(order)], "voice": v, "two": token()})
    hidden = token() if m else None
    table = {t: v for v, t in tok.items()}
    for c in catches:
        table[c["two"]] = c["voice"]
    if hidden:
        table[hidden] = m["reference"]
    if len(set(table)) != len(table) or len(set(tok.values())) != len(tok):
        return build(subject, out, flac, twin, run, log)
    # the clips, matched per item
    shutil.rmtree(out / "audio", ignore_errors=True)
    (out / "audio").mkdir(parents=True, exist_ok=True)
    key_dir.mkdir(parents=True, exist_ok=True)
    if twin:
        shutil.rmtree(Path(twin) / "audio", ignore_errors=True)
        (Path(twin) / "audio").mkdir(parents=True, exist_ok=True)
    count = len(items) * (len(voices) + len(anchors)) + len(catches) + (len(items) if hidden else 0)
    names = iter(draw_names(count))
    FILES, key_files, key_items = {}, {}, {}
    levels = ["item,voice,file,lufs_before,peak_before_dbfs,gain_db,lufs_after,peak_after_dbfs"]
    for it in items:
        clips = []
        for v in voices:
            x, sr = audio.read(files[v][it])
            clips.append(prepare(x, sr, L.get("pre_roll_s"), L.get("max_s")))
        ids = voices[:]
        if m:
            ref = clips[voices.index(m["reference"])]
            for hz, a in zip(m.get("anchors", []), anchors):
                clips.append(anchor(ref, hz))
                ids.append(a)
        clips, target, before, gains = match_group(clips, L.get("fade_s", 0.1), L.get("target_lufs", -23.0), L.get("ceiling_db", -1.0))
        for v, c, (lu, pk), g in zip(ids, clips, before, gains):
            extra = [cc["two"] for cc in catches if cc["phrase"] == it and cc["voice"] == v]
            if hidden and v == m["reference"]:
                extra.append(hidden)
            first = None
            for t in [tok[v]] + extra:
                fn = f"{next(names)}.{ext}"
                path = out / "audio" / fn
                if first is None:
                    audio.write(path, c, SR)
                    if twin:
                        audio.write(Path(twin) / "audio" / (fn[:-len(ext)] + "wav"), c, SR)
                    first = path
                else:
                    shutil.copyfile(first, path)
                    if twin:
                        shutil.copyfile(Path(twin) / "audio" / (first.stem + ".wav"), Path(twin) / "audio" / (path.stem + ".wav"))
                FILES[f"{it}|{t}"] = fn
                role = "catch" if any(t == cc["two"] for cc in catches) else ("hidden_reference" if t == hidden else ("anchor" if v in anchors else "voice"))
                key_files[fn] = {"item": it, "voice": v, "role": role,
                                 "source": str(files[v][it]) if v in files else f"low-pass of {m['reference']}"}
                levels.append(f"{it},{v},{fn},{lu:.2f},{pk:.2f},{g:.2f},{lufs(c, SR):.2f},{20 * np.log10(max(np.abs(c).max(), 1e-9)):.2f}")
        key_items[it] = {"target_lufs": round(float(target), 2), "seconds": round(len(clips[0]) / SR, 2)}
        log(f"  {it:12s} {len(clips[0]) / SR:5.1f} s at {target:.1f} LUFS")
    assert next(names, None) is None, "every drawn name used"
    pairs = L.get("pref") or page["pairs"]
    tests = {"abx": [tok[v] for v in abx],
             "pref": [[tok[a], tok[b]] for a, b in pairs],
             "extra": [tok[v] for v in L["extra"]] if L.get("extra") else None,
             "catch": [{"phrase": c["phrase"], "one": tok[c["voice"]], "two": c["two"]} for c in catches]}
    if m:
        tests["mushra"] = {"items": items, "ref": tok[m["reference"]],
                           "stims": [hidden] + [tok[v] for v in m["conditions"]] + [tok[a] for a in anchors]}
    seed = secrets.randbits(32) | 1
    kit_id = f"{L.get('kit_prefix', 'kit1')}-{name()[:10]}"
    fills = [json.dumps(kit_id), json.dumps(items), json.dumps(FILES), json.dumps(tests), str(seed),
             json.dumps(veil(json.dumps(table), seed)), json.dumps(page, ensure_ascii=False)]
    html = PAGE.read_text(encoding="utf-8")
    for p, v in zip(PLACEHOLDERS, fills):
        assert p in html, p
        html = html.replace(p, v)
    assert not any(p in html for p in PLACEHOLDERS)
    (out / "index.html").write_text(html, encoding="utf-8")
    for lang, fn in (("es", "LEEME.md"), ("en", "README.md")):
        (out / fn).write_text(L.get("readme", {}).get(lang, ""), encoding="utf-8")
    if twin:
        Path(twin).mkdir(parents=True, exist_ok=True)
        (Path(twin) / "index.html").write_text(html.replace(f'.{ext}"', '.wav"'), encoding="utf-8")
        for fn in ("LEEME.md", "README.md"):
            shutil.copyfile(out / fn, Path(twin) / fn)
    key = {"kit": kit_id, "created": int(time.time()), "subject": subject.id, "tokens": table,
           "catch": [c["phrase"] for c in catches], "hidden_reference": hidden,
           "mushra": m, "items": key_items, "files": key_files,
           "voices": {v: {it: str(p) for it, p in files[v].items()} for v in voices}}
    (key_dir / "key.json").write_text(json.dumps(key, indent=1, ensure_ascii=False), encoding="utf-8")
    (key_dir / "levels.csv").write_text("\n".join(levels) + "\n", encoding="utf-8")
    size = sum(p.stat().st_size for p in out.rglob("*") if p.is_file())
    log(f"kit {kit_id} in {out} — {size / 1e6:.1f} MB; the key in {key_dir}")
    return kit_id
