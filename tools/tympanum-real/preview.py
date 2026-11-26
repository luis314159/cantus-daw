#!/usr/bin/env python3
"""The owner's named previews (round 42, tympanum-real): Tympanum before and after beside
the recorded kits, per genre, at one loudness, 16-bit WAV, and a page in Spanish.

    python -I tools/tympanum-real/preview.py <work> <listen dir> [--tags before,after]

For each groove of `pipeline.GROOVES`: the references of its genre (`PAIRS`) and Tympanum's
kits at each tag, cut to the groove's length + 1.5 s, brought to −20 LUFS (ITU-R BS.1770-4,
K-weighted, gated; lowered together if a peak would pass −1 dBFS), dithered to 16 bits (TPDF),
written as `<listen>/<groove>/<nn>-<name>.wav`; `escuchar.html` lists them with players and
says what to listen for. Named, not blind: the blind kit comes after he has heard these.
"""
import html
import json
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy import signal

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pipeline as P  # noqa: E402

TARGET = -20.0
CEIL = -1.0
# which recorded kits sit beside Tympanum in each genre, and what to listen for there
PAIRS = {
    "rock": (["drskit", "muldjord", "bigrusty"], "el golpe de caja contra el aro, el bombo que empuja, el crash que se abre"),
    "pop": (["drskit", "unruly"], "una caja suave y redonda, charles cerrados, el aire de la sala"),
    "funk": (["gogodze", "drskit"], "los golpes fantasma de la caja (casi solo bordonera), el charles abriéndose"),
    "jazz": (["virtuosity", "unruly-brushes"], "el ride: el «ping» y el lavado; el bombo abierto y afinado"),
    "samba": (["virtuosity", "drskit"], "el pulso del bombo, el aro, los toms"),
    "soul": (["gogodze", "drskit"], "toms apagados, la caja seca de los setenta"),
    "gospel": (["drskit", "bigrusty"], "los rellenos de toms, los crash, la dinámica"),
    "punk": (["muldjord", "aasimonster"], "velocidad y fuerza: ¿se cansa el sonido al repetirse?"),
    "hiphop": (["drskit", "unruly"], "un bombo grave y largo, la caja con cuerpo"),
}
TYMP = [("studio", "Tympanum Studio Maple"), ("jazz", "Tympanum Jazz Birch")]
# Real playing, transcribed (`transcribe.py`, templates off DRSKit) and played back: the
# recording itself (a Freesound CC0 preview), DRSKit and Tympanum playing its transcription.
REAL = [
    ("real50721", "Drums Funk Groove 6 (RutgerMuller, CC0)", "50721.mp3",
     "una grabación real de funk, transcrita a MIDI y tocada de nuevo: la caja y el charles"),
    ("real50722", "Drums Funk Groove 7 Ride (RutgerMuller, CC0)", "50722.mp3",
     "el ride de una grabación real, transcrito"),
    ("real478563", "Doom Metal Drums Improvisation (RutgerMuller, CC0)", "478563.mp3",
     "metal lento y pesado: bombo, crash y toms"),
]


def kweight(x, fs):
    # BS.1770-4's two stages, bilinear from their analogue prototypes at any rate
    f0, g, q = 1681.974450955533, 3.999843853973347, 0.7071752369554196
    k = np.tan(np.pi * f0 / fs)
    vh = 10 ** (g / 20)
    vb = vh ** 0.4996667741545416
    a0 = 1 + k / q + k * k
    b1 = [(vh + vb * k / q + k * k) / a0, 2 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0]
    a1 = [1, 2 * (k * k - 1) / a0, (1 - k / q + k * k) / a0]
    f0, q = 38.13547087602444, 0.5003270373238773
    k = np.tan(np.pi * f0 / fs)
    a0 = 1 + k / q + k * k
    b2 = [1, -2, 1]
    a2 = [1, 2 * (k * k - 1) / a0, (1 - k / q + k * k) / a0]
    return signal.lfilter(b2, a2, signal.lfilter(b1, a1, x, axis=0), axis=0)


def lufs(x, fs):
    y = kweight(x, fs)
    n, h = int(0.4 * fs), int(0.1 * fs)
    z = np.array([np.sum(np.mean(y[i:i + n] ** 2, axis=0)) for i in range(0, len(y) - n, h)])
    z = z[z > 0]
    lk = -0.691 + 10 * np.log10(z)
    z = z[lk > -70]
    if not len(z):
        return -120.0
    rel = -0.691 + 10 * np.log10(z.mean()) - 10
    z = z[-0.691 + 10 * np.log10(z) > rel]
    return float(-0.691 + 10 * np.log10(z.mean()))


def to16(x, rng):
    d = (rng.random(x.shape) - rng.random(x.shape)) / 32768.0
    return np.clip(np.round((x + d) * 32767.0), -32768, 32767).astype(np.int16)


def main():
    work, listen = Path(sys.argv[1]), Path(sys.argv[2])
    tags = (P.arg("--tags", "before,after")).split(",")
    rng = np.random.default_rng(42)
    labels = {k[0]: k[2] for k in P.KITS}
    page = []
    meta = []
    for name, mid, start, ln in P.GROOVES:
        refs, hint = PAIRS[name]
        clips = []
        for kid in refs:
            p = work / "grooves" / "ref" / f"{name}--{kid}.wav"
            if p.exists():
                clips.append((f"grabada · {labels[kid]}", p))
        for kid, lab in TYMP:
            for t in tags:
                p = work / "grooves" / t / f"{name}--{kid}.wav"
                if p.exists():
                    clips.append((f"{lab} · {'antes' if t == 'before' else 'después' if t == 'after' else t}", p))
        if not clips:
            continue
        audio = []
        for lab, p in clips:
            x, fs = sf.read(str(p), dtype="float64", always_2d=True)
            x = x[: int((ln + 2.0) * fs)]
            fade = int(0.1 * fs)
            x[-fade:] *= np.linspace(1, 0, fade)[:, None]
            audio.append((lab, p, x, fs))
        gains = [TARGET - lufs(x, fs) for _, _, x, fs in audio]
        peaks = [20 * np.log10(np.abs(x).max() + 1e-12) + g for (_, _, x, _), g in zip(audio, gains)]
        drop = max(0.0, max(peaks) - CEIL)
        d = listen / name
        d.mkdir(parents=True, exist_ok=True)
        items = []
        for i, ((lab, p, x, fs), g) in enumerate(zip(audio, gains)):
            fn = f"{i + 1:02d}-{lab.replace(' · ', '-').replace(' ', '_').lower()}.wav"
            y = x * 10 ** ((g - drop) / 20)
            sf.write(str(d / fn), to16(y, rng), fs, subtype="PCM_16")
            items.append((lab, f"{name}/{fn}"))
            meta.append({"groove": name, "clip": lab, "file": f"{name}/{fn}", "source": str(p),
                         "gain_db": g - drop, "lufs": TARGET - drop})
        page.append((name, mid, hint, items, TARGET - drop))
    for name, title, mp3, hint in REAL:
        src = P.REFS / "drums" / "freesound" / mp3
        clips = [("grabación original", src, 0.0)]
        for kid, lab, t in [("drskit", "grabada · DRSKit, la transcripción", "ref")] + [
                (k, f"{l} · {'antes' if t == 'before' else 'después'}, la transcripción", t)
                for k, l in TYMP for t in tags]:
            p = work / "grooves" / t / f"{name}--{kid}.wav"
            if p.exists():
                clips.append((lab, p, 0.5))
        if len(clips) < 2 or not src.exists():
            continue
        audio = []
        for lab, p, skip in clips:
            x, fs = sf.read(str(p), dtype="float64", always_2d=True)
            if x.shape[1] == 1:
                x = np.repeat(x, 2, axis=1)
            if fs != 48000:
                x = signal.resample_poly(x, 48000, fs, axis=0)
                fs = 48000
            x = x[int(skip * fs): int((skip + 20.0) * fs)]
            fade = int(0.1 * fs)
            x[-fade:] *= np.linspace(1, 0, fade)[:, None]
            audio.append((lab, p, x, fs))
        gains = [TARGET - lufs(x, fs) for _, _, x, fs in audio]
        peaks = [20 * np.log10(np.abs(x).max() + 1e-12) + g for (_, _, x, _), g in zip(audio, gains)]
        drop = max(0.0, max(peaks) - CEIL)
        d = listen / name
        d.mkdir(parents=True, exist_ok=True)
        items = []
        for i, ((lab, p, x, fs), g) in enumerate(zip(audio, gains)):
            fn = f"{i + 1:02d}-{lab.split(',')[0].replace(' · ', '-').replace(' ', '_').lower()}.wav"
            sf.write(str(d / fn), to16(x * 10 ** ((g - drop) / 20), rng), fs, subtype="PCM_16")
            items.append((lab, f"{name}/{fn}"))
            meta.append({"groove": name, "clip": lab, "file": f"{name}/{fn}", "source": str(p),
                         "gain_db": g - drop, "lufs": TARGET - drop})
        page.append((title, mp3, hint, items, TARGET - drop))
    write_page(listen, page)
    (listen / "previews.json").write_text(json.dumps(meta, indent=1, ensure_ascii=False), encoding="utf-8")
    print(f"{len(meta)} clips in {listen}")


def write_page(listen, page):
    css = """
:root{--bg:#fcfcfb;--ink:#0b0b0b;--muted:#52514e;--line:#e4e3df;--card:#ffffff;--acc:#2a78d6}
@media (prefers-color-scheme:dark){:root{--bg:#1a1a19;--ink:#f0efec;--muted:#c3c2b7;--line:#33322f;--card:#232321;--acc:#3987e5}}
body{margin:0;background:var(--bg);color:var(--ink);font:16px/1.5 system-ui,sans-serif}
main{max-width:880px;margin:0 auto;padding:24px 16px 64px}
h1{font-size:1.6rem;margin:.2em 0}h2{font-size:1.15rem;margin:1.6em 0 .3em;text-transform:capitalize}
p.lead,p.hint{color:var(--muted)}section{border-top:1px solid var(--line);padding-top:8px}
.row{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1.3fr);gap:12px;align-items:center;
background:var(--card);border:1px solid var(--line);border-radius:8px;padding:8px 12px;margin:6px 0}
.row.t{border-left:4px solid var(--acc)}.row.o{border-left:4px solid var(--muted)}audio{width:100%}small{color:var(--muted)}
@media (max-width:600px){.row{grid-template-columns:1fr}}
"""
    out = ["<!doctype html><html lang='es'><head><meta charset='utf-8'><meta name='viewport' "
           "content='width=device-width,initial-scale=1'><title>Tympanum real</title><style>"
           + css + "</style></head><body><main>",
           "<h1>Tympanum frente a baterías grabadas</h1>",
           "<p class='lead'>Cada ritmo es una interpretación humana real (Groove MIDI Dataset, "
           "CC BY 4.0, Google) tocada por baterías grabadas de verdad y por Tympanum, antes y "
           "después de esta ronda. Todos a la misma sonoridad (−20 LUFS, o menos si un pico pasaba "
           "de −1 dBFS). Con nombres, no a ciegas: la prueba ciega viene después. Escucha con "
           "auriculares; el informe con las medidas está en <a href='informe.html'>informe.html</a>.</p>"]
    for name, mid, hint, items, lv in page:
        out.append(f"<section><h2>{html.escape(name)}</h2><p class='hint'>Fíjate en: "
                   f"{html.escape(hint)}. <small>({html.escape(mid.split('/')[-1])}, {lv:.1f} LUFS)</small></p>")
        for lab, f in items:
            t = " t" if lab.startswith("Tympanum") else ""
            if lab.startswith("grabación original"):
                t = " o"
            out.append(f"<div class='row{t}'><div>{html.escape(lab)}</div>"
                       f"<audio controls preload='none' src='{html.escape(f)}'></audio></div>")
        out.append("</section>")
    out.append("<p><small>Créditos: DRSKit, MuldjordKit, The Aasimonster, ShittyKit (DrumGizmo, CC BY "
               "4.0); Big Rusty Drums, Unruly Drums, Gogodze Phu II, Virtuosity Drums (Karoryfer "
               "Samples, Versilian Studios, CC0); Groove MIDI Dataset (Google, CC BY 4.0).</small></p>")
    out.append("</main></body></html>")
    (listen / "escuchar.html").write_text("\n".join(out), encoding="utf-8")


if __name__ == "__main__":
    main()
