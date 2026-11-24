"""make_abx.py - loudness-matched listening references for Arcus's ABX (cello, bass, viola, violin).

Usage:  venv\\Scripts\\python.exe -I tools\\make_abx.py [out_dir] [--lufs -23]
Writes to %CANTUS_STUDY_DOWNLOADS%\\study\\abx-refs\\ (default; never into a repository):
<family>/<family>_<source>_<note>_<dyn>.wav (a solo note at pp / mf / ff) and
<family>/<family>_<source>_phrase_*.wav (a legato phrase; an anechoic repertoire excerpt),
all mono 24-bit 48 kHz (resampled from 44.1 kHz with scipy's polyphase filter), each at the
same integrated loudness (ITU-R BS.1770-4: K-weighting, 400 ms blocks, -70 LUFS absolute and
-10 LU relative gates) unless its peak would pass -1 dBFS, in which case it is lowered and
says so in index.json. index.json lists every file with its source, licence and use.
"""
import os
import json
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy import signal

D = Path(os.environ.get("CANTUS_STUDY_DOWNLOADS", r"F:\projects\daw-r42\downloads"))
S = D / "study"
TS_C = D / "arcus" / "tinysol" / "Strings"
TS_U = S / "tinysol-violin-viola" / "audio" / "Strings"
SM = D / "strings-rec" / "x" / "dsmolken-double-bass" / "arco"
KA = D / "strings-rec" / "x" / "karoryfer-bigcat-cello" / "Samples" / "sus"
VS = D / "strings-rec" / "vsco" / "Strings"
PH = S / "philharmonia" / "raw" / "Strings"
PHU = S / "philharmonia-violin-viola" / "raw" / "Strings"
AN = S / "phenicx-anechoic" / "PHENICX-Anechoic" / "audio"
HS = S / "hsd-string-quartet-k465" / "Streicherquartett_Complete_Project" / "Audiodateien"
LIC = {"tinysol": ("CC BY 4.0 (IRCAM, TinySOL)", "may ship with credit"),
       "smolken": ("CC0 1.0", "may ship"), "karoryfer": ("CC0 1.0", "may ship"),
       "vsco": ("CC0 1.0", "may ship"),
       "hsd": ("CC BY 4.0 (Hochschule Duesseldorf, Mozart K465 stems, Zenodo 3601097)", "may ship with credit"),
       "philharmonia": ("Philharmonia terms: free to use, not to be sold or redistributed as samples", "measurement and listening only"),
       "phenicx": ("audio (c) Aalto University, academic research use; annotations CC BY-NC-SA 3.0", "measurement and listening only")}

NOTES = {
    "cello": [("tinysol", TS_C / "Violoncello/ordinario/Vc-ord-A3-{d}-2c-N.wav", "A3", ["pp", "mf", "ff"]),
              ("karoryfer", KA / "A2_{d}_d.wav", "A3", ["p", "mf", "f"])],
    "bass": [("tinysol", TS_C / "Contrabass/ordinario/Cb-ord-D2-{d}-2c-N.wav", "D2", ["pp", "mf", "ff"]),
             ("smolken", SM / "arco_d2_{d}_down.wav", "D2", ["pp", "mf", "ff"])],
    "viola": [("tinysol", TS_U / "Viola/ordinario/Va-ord-D4-{d}-2c-N.wav", "D4", ["pp", "mf", "ff"])],
    "violin": [("tinysol", TS_U / "Violin/ordinario/Vn-ord-A4-{d}-2c-N.wav", "A4", ["pp", "mf", "ff"]),
               ("vsco", VS / "Solo Violin/Arco Vib/LLVln_ArcoVib_A4_{d}.wav", "A4", ["p", "f"])],
}
PHRASES = {
    "cello": [("philharmonia", PH / "cello/cello_D3_phrase_mezzo-forte_arco-legato.mp3", None),
              ("phenicx", AN / "mozart/cello.wav", 8.0),
              ("hsd", HS / "Cello Phrase 1/Cello_Phrase_1_Vlc.wav", 10.0)],
    "bass": [("philharmonia", PH / "double bass/double-bass_E2_phrase_mezzo-forte_arco-legato.mp3", None),
             ("phenicx", AN / "mozart/doublebass.wav", 8.0)],
    "viola": [("philharmonia", PHU / "viola/viola_D4_phrase_mezzo-forte_arco-legato.mp3", None),
              ("phenicx", AN / "mozart/viola.wav", 8.0),
              ("hsd", HS / "Quartet Long Excerpt/Quartet_Long_Vla.wav", 10.0)],
    "violin": [("philharmonia", PHU / "violin/violin_A4_phrase_mezzo-forte_arco-legato.mp3", None),
               ("phenicx", AN / "mozart/violin1.wav", 8.0),
               ("hsd", HS / "Quartet Long Excerpt/Quartet_Long_Vl1.wav", 10.0)],
}


def kweight(x, sr):
    """BS.1770 K-weighting: the pre-filter shelf and the RLB high-pass, designed at sr."""
    f0, G, Q = 1681.974450955533, 3.999843853973347, 0.7071752369554196
    K = np.tan(np.pi * f0 / sr)
    Vh = 10 ** (G / 20)
    Vb = Vh ** 0.4996667741545416
    a0 = 1 + K / Q + K * K
    b = [(Vh + Vb * K / Q + K * K) / a0, 2 * (K * K - Vh) / a0, (Vh - Vb * K / Q + K * K) / a0]
    a = [1, 2 * (K * K - 1) / a0, (1 - K / Q + K * K) / a0]
    y = signal.lfilter(b, a, x)
    f0, Q = 38.13547087602444, 0.5003270373238773
    K = np.tan(np.pi * f0 / sr)
    a0 = 1 + K / Q + K * K
    a = [1, 2 * (K * K - 1) / a0, (1 - K / Q + K * K) / a0]
    return signal.lfilter([1, -2, 1], a, y)


def lufs(x, sr):
    y = kweight(x, sr)
    blk, hop = int(0.4 * sr), int(0.1 * sr)
    if len(y) < blk:
        y = np.pad(y, (0, blk - len(y)))
    z = np.array([np.mean(y[i:i + blk] ** 2) for i in range(0, len(y) - blk + 1, hop)])
    L = -0.691 + 10 * np.log10(z + 1e-20)
    g = z[L > -70]
    if not len(g):
        return -99.0
    rel = -0.691 + 10 * np.log10(np.mean(g)) - 10
    g = z[(L > -70) & (L > rel)]
    return float(-0.691 + 10 * np.log10(np.mean(g)))


def to48(x, sr):
    if sr == 48000:
        return x
    from math import gcd
    g = gcd(48000, sr)
    return signal.resample_poly(x, 48000 // g, sr // g)


def best_window(x, sr, dur):
    """The `dur`-second window of an anechoic part with the most sounding 50 ms frames."""
    h = int(0.05 * sr)
    e = np.array([np.mean(x[i:i + h] ** 2) for i in range(0, len(x) - h, h)])
    on = e > np.percentile(e, 95) * 10 ** (-30 / 10)
    w = int(dur / 0.05)
    c = np.convolve(on.astype(float), np.ones(w), mode="valid")
    i = int(np.argmax(c))
    return x[i * h:i * h + int(dur * sr)]


def write(out, name, x, sr, target, meta, index):
    x = to48(x, sr)
    fade = int(0.01 * 48000)
    x[:fade] *= np.linspace(0, 1, fade)
    x[-fade:] *= np.linspace(1, 0, fade)
    L = lufs(x, 48000)
    g = 10 ** ((target - L) / 20)
    pk = np.max(np.abs(x)) * g
    lowered = 0.0
    if pk > 10 ** (-1 / 20):
        lowered = 20 * np.log10(pk) + 1
        g *= 10 ** (-lowered / 20)
    y = x * g
    p = out / name
    p.parent.mkdir(parents=True, exist_ok=True)
    sf.write(str(p), y, 48000, subtype="PCM_24")
    meta.update({"file": str(p.relative_to(out)), "lufs": round(lufs(y, 48000), 2),
                 "peak_dbfs": round(20 * np.log10(np.max(np.abs(y))), 2), "lowered_db": round(lowered, 2),
                 "dur_s": round(len(y) / 48000, 2)})
    index.append(meta)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    out = Path(args[0]) if args else S / "abx-refs"
    target = float(sys.argv[sys.argv.index("--lufs") + 1]) if "--lufs" in sys.argv else -23.0
    index = []
    for fam, sets in NOTES.items():
        for src, pat, note, dyns in sets:
            for d in dyns:
                p = Path(str(pat).format(d=d))
                if not p.exists():
                    print("missing", p)
                    continue
                x, sr = sf.read(str(p), dtype="float64", always_2d=True)
                write(out, f"{fam}/{fam}_{src}_{note}_{d}.wav", x[:, 0].copy(), sr, target,
                      {"family": fam, "kind": "note", "source": src, "note": note, "dyn": d, "from": str(p),
                       "licence": LIC[src][0], "use": LIC[src][1]}, index)
        for src, p, dur in PHRASES[fam]:
            if not p.exists():
                print("missing", p)
                continue
            x, sr = sf.read(str(p), dtype="float64", always_2d=True)
            x = x[:, 0].copy()
            if dur:
                x = best_window(x, sr, dur)
            tag = {"philharmonia": "legato", "phenicx": "anechoic-excerpt", "hsd": "k465-excerpt"}[src]
            write(out, f"{fam}/{fam}_{src}_phrase_{tag}.wav", x, sr, target,
                  {"family": fam, "kind": "phrase", "source": src, "tag": tag, "from": str(p),
                   "licence": LIC[src][0], "use": LIC[src][1]}, index)
    (out / "index.json").write_text(json.dumps(index, indent=1))
    for m in index:
        print(m["file"], m["lufs"], m["peak_dbfs"], m["lowered_db"])


if __name__ == "__main__":
    main()
