"""The ladder's blind listening kit (round 40, ladder): a MUSHRA page (hidden reference R0,
anchor R5, rungs R1-R4, the per-bin table R3b and the oracle R3o) for a few articulations, then ABX of R3 and R4
against the recording for the snare and the crash, and of R3b for the crash (16 trials each).

    python -I ladder_kit.py build <ladder dir> <kit dir>      (the key goes to <kit dir>-key)
    python -I ladder_kit.py score <key dir> <results.json> ...

Every file gets a random name; the page knows only names and tokens; which rung each one is
lives in the key folder. Each item's files are matched in loudness to its recording
(ITU-R BS.1770 K-weighting over the hit's first second, two for cymbals; no gating) and
have the same length; nothing else is done to them.
"""
import json
import os
import sys
import numpy as np
import soundfile as sf
from scipy import signal

HERE = os.path.dirname(os.path.abspath(__file__))
SR = 48000
KIT_ID = "tympanum-escalera-1"
# (kit, articulation, velocity) — MUSHRA items, and the ABX tests (+ the rung against R0)
MUSHRA = [("drs", "snare", 127), ("drs", "kick", 127), ("virt", "tom_floor", 127),
          ("drs", "hat_closed", 127), ("virt", "hat_open", 127), ("drs", "crash", 127),
          ("virt", "ride", 80)]
STIMS = ["R0", "R1", "R2", "R3", "R3b", "R3o", "R4", "R5"]
ABX = [("drs", "snare", 127, "R3"), ("drs", "crash", 127, "R3"),
       ("drs", "snare", 127, "R4"), ("drs", "crash", 127, "R4"), ("drs", "crash", 127, "R3b")]
TRIALS = 16
LONG = {"hat_half", "hat_open", "ride", "ride_bell", "crash"}
NAMES_ES = {"kick": "bombo", "snare": "caja", "tom_floor": "tom de piso", "hat_closed": "hi-hat cerrado",
            "hat_open": "hi-hat abierto", "crash": "crash", "ride": "ride"}
RUNG_ES = {"R0": "la grabación", "R1": "modelo 5 (modos + 38 bandas + baqueta genérica)",
           "R2": "R1 + ataque de fase mínima del propio golpe",
           "R3": "R2 + ruido en tabla fina (110 bandas x 40 tiempos)",
           "R3b": "R2 + ruido en tabla por bin (11,7 Hz) x 40 tiempos, fase al azar",
           "R3o": "oráculo: magnitud STFT del propio golpe, fase al azar",
           "R4": "R3 + co-modulación, anchura de parciales, ráfagas", "R5": "ancla: R1 con paso bajo a 3,5 kHz"}


def kweight(x):
    b1, a1 = [1.53512485958697, -2.69169618940638, 1.19839281085285], [1.0, -1.69065929318241, 0.73248077421585]
    b2, a2 = [1.0, -2.0, 1.0], [1.0, -1.99004745483398, 0.99007225036621]
    return signal.lfilter(b2, a2, signal.lfilter(b1, a1, x))


def loudness(x, on, art):
    y = kweight(x)[on:on + int((2.0 if art in LONG else 1.0) * SR)]
    return -0.691 + 10 * np.log10(np.mean(y ** 2) + 1e-30)


def build(ladder, out):
    rng = np.random.default_rng(40_2026_11_13)
    tok = lambda k=10: "".join(rng.choice(list("abcdefghijkmnpqrstuvwxyz23456789"), k))
    info = json.load(open(os.path.join(ladder, "ladder.json")))
    key_dir = out.rstrip("/\\") + "-key"
    os.makedirs(os.path.join(out, "audio"), exist_ok=True)
    os.makedirs(key_dir, exist_ok=True)
    # a rebuild draws new names: the kit's own earlier clips go
    for f in os.listdir(os.path.join(out, "audio")):
        if f.endswith(".wav") and len(f) == 14:
            os.remove(os.path.join(out, "audio", f))
    key = {"kit": KIT_ID, "mushra": {}, "abx": {}}
    cfg = {"kit": KIT_ID, "trials": TRIALS, "mushra": [], "abx": []}

    def put(x, gain):
        name = tok() + ".wav"
        sf.write(os.path.join(out, "audio", name), x * gain, SR, subtype="PCM_24")
        return "audio/" + name

    def matched(kit, art, vel, rungs):
        """The rungs' signals, each at its recording's loudness (-23 LUFS-like), and the
        item's common gain kept under -1 dBFS peak."""
        stem = os.path.join(ladder, f"{kit}-{art}-v{vel}")
        on = info[f"{kit}-{art}-v{vel}"]["onset"]
        x = {r: sf.read(f"{stem}-{r}.wav")[0] for r in set(rungs) | {"R0"}}
        L0 = loudness(x["R0"], on, art)
        g = {r: 10 ** ((-23 - loudness(x[r], on, art)) / 20) for r in x}
        peak = max(np.abs(x[r]).max() * g[r] for r in x)
        common = min(1.0, 10 ** (-1 / 20) / peak)
        return x, {r: g[r] * common for r in x}, L0

    for kit, art, vel in MUSHRA:
        x, g, L0 = matched(kit, art, vel, STIMS)
        iid = tok(6)
        ref = put(x["R0"], g["R0"])
        stims, kst = [], {}
        for r in STIMS:
            sid = tok(6)
            f = put(x[r], g[r])
            stims.append({"id": sid, "file": f})
            kst[sid] = {"rung": r, "file": f}
        rng.shuffle(stims)
        cfg["mushra"].append({"id": iid, "art": art, "vel": vel, "ref": ref, "stims": stims})
        key["mushra"][iid] = {"kit": kit, "art": art, "vel": vel, "ref": ref, "stims": kst,
                              "recording_lufs": round(L0, 1)}
    for kit, art, vel, rung in ABX:
        x, g, _ = matched(kit, art, vel, [rung])
        tid = tok(6)
        pair = [("R0", put(x["R0"], g["R0"])), (rung, put(x[rung], g[rung]))]
        if rng.integers(2):
            pair.reverse()
        cfg["abx"].append({"id": tid, "art": art, "vel": vel, "files": [p[1] for p in pair]})
        key["abx"][tid] = {"kit": kit, "art": art, "vel": vel, "rung": rung,
                           "files": {p[1]: p[0] for p in pair}}
    html = open(os.path.join(HERE, "ladder_kit.html"), encoding="utf-8").read()
    js = open(os.path.join(HERE, "ladder_kit.js"), encoding="utf-8").read()
    html = html.replace("__CONFIG__", json.dumps(cfg, ensure_ascii=False))
    html = html.replace('<script src="kit.js"></script>', "<script>\n" + js + "</script>")
    open(os.path.join(out, "index.html"), "w", encoding="utf-8", newline="\n").write(html)
    open(os.path.join(out, "LEEME.txt"), "w", encoding="utf-8", newline="\n").write(README)
    json.dump(key, open(os.path.join(key_dir, "clave.json"), "w", encoding="utf-8"), indent=1, ensure_ascii=False)
    lines = ["Clave de la escalera de batería (no la abras antes de escuchar)\n"]
    for iid, it in key["mushra"].items():
        lines.append(f"MUSHRA {NAMES_ES[it['art']]} v{it['vel']} ({'DRSKit' if it['kit'] == 'drs' else 'Virtuosity'}): referencia {it['ref']}")
        for sid, s in it["stims"].items():
            lines.append(f"   {s['file']}  {s['rung']:3s}  {RUNG_ES[s['rung']]}")
    for tid, t in key["abx"].items():
        lines.append(f"ABX {NAMES_ES[t['art']]} v{t['vel']}: {t['rung']} contra R0 — " +
                     ", ".join(f"{f} = {r}" for f, r in t["files"].items()))
    lines.append("\nPuntuar: python -I tools/tympanum-fit/ladder_kit.py score <esta carpeta> <resultados.json>")
    open(os.path.join(key_dir, "clave.txt"), "w", encoding="utf-8", newline="\n").write("\n".join(lines) + "\n")
    size = sum(os.path.getsize(os.path.join(dp, f)) for dp, _, fs in os.walk(out) for f in fs)
    print(f"kit {out}: {size / 1e6:.1f} MB; key {key_dir}")


README = """Escalera de batería — prueba de escucha
========================================

1. Abre index.html con el navegador (doble clic). Funciona sin internet.
2. Usa auriculares o buenos monitores, en silencio. Ajusta el volumen con la primera
   Referencia y no lo cambies.
3. Parte 1: en cada golpe, puntúa cada muestra de 0 a 100 según cuánto se parece a la
   Referencia (la grabación). Una de las muestras es la propia grabación, escondida.
4. Parte 2: cinco pruebas ABX de 16 ensayos: ¿X es A o es B?
5. Al terminar, pulsa «Descargar resultados (JSON)» y envía ese archivo.

Las respuestas se guardan en el navegador: puedes parar y seguir otro día.
Créditos: DRSKit 2.1 (DrumGizmo — Deva y Lars Muldjord — y Jes Eiler de DRSDrums), CC BY 4.0;
Virtuosity Drums (Versilian Studios y Karoryfer Samples, Austin McMahon), CC0.

---

Drum ladder — listening test

1. Open index.html in a browser (double-click). It works offline.
2. Headphones or good monitors, somewhere quiet. Set the volume on the first Reference.
3. Part 1: rate each sample 0-100 by how close it is to the Reference (the recording).
   One of the samples is the recording itself, hidden.
4. Part 2: five ABX tests of 16 trials: is X A or B?
5. When done, press "Download results (JSON)" and send that file.
"""


def binom_p(k, n):
    from math import comb
    return sum(comb(n, i) for i in range(k, n + 1)) / 2 ** n if n else 1.0


def score(key_dir, files):
    key = json.load(open(os.path.join(key_dir, "clave.json"), encoding="utf-8"))
    for f in files:
        st = json.load(open(f, encoding="utf-8"))
        print(f"\n== {f}: {st.get('name') or '?'} ({st.get('device')})")
        allr = {r: [] for r in STIMS}
        for it in st["mushra"]:
            k = key["mushra"][it["item"]]
            row = {k["stims"][sid]["rung"]: v for sid, v in it["scores"].items()}
            for r, v in row.items():
                allr[r].append(v)
            print(f"MUSHRA {k['kit']:4s} {k['art']:10s} v{k['vel']}: " + "  ".join(f"{r} {row.get(r, '-')}" for r in STIMS))
        print("MUSHRA mean:  " + "  ".join(f"{r} {np.mean(v):.0f}" for r, v in allr.items() if v))
        for t in st["abx"]:
            k = key["abx"][t["test"]]
            tr = [x for x in t["trials"] if x["answer"]]
            right = sum(x["answer"] == x["x"] for x in tr)
            print(f"ABX {k['kit']} {k['art']} v{k['vel']} {k['rung']} vs R0: {right}/{len(tr)} p = {binom_p(right, len(tr)):.3f}")


if __name__ == "__main__":
    if sys.argv[1] == "build":
        build(sys.argv[2], sys.argv[3])
    else:
        score(sys.argv[2], sys.argv[3:])
