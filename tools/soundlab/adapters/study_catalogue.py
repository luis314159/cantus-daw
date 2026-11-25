"""The Arcus study's reference catalogue (`tools/arcus/study/catalog.py`'s catalog.json) as a
lab catalogue, free sources only: one family, its arco and pizzicato notes, every file's
licence, credit and physical instrument; Philharmonia (its own terms) and PHENICX (NC) left
out, VSCO's sections left out (a section is not one instrument), lossy files left out.

    python -I tools/soundlab/adapters/study_catalogue.py <catalog.json> <family> <downloads root> <out.json>

Paths are written relative to the downloads root (the subject's `references.root`).
"""
import json
import sys
from pathlib import Path

NAMES = "C C# D D# E F F# G G# A A# B".split()
FLATS = {"Db": "C#", "Eb": "D#", "Gb": "F#", "Ab": "G#", "Bb": "A#"}
LICENCE = {
    "iowa": ("Iowa-MIS", "University of Iowa Electronic Music Studios, Musical Instrument Samples"),
    "tinysol": ("CC-BY-4.0", "TinySOL (Cella, Ghisi, Lostanlen, Lévy, Fineberg, Assayag), IRCAM, Zenodo 3685367"),
    "karoryfer": ("CC0-1.0", "Karoryfer Samples, Bigcat cello"),
    "vsco": ("CC0-1.0", "Versilian Studios, VSCO 2 Community Edition"),
    "smolken": ("CC0-1.0", "D. Smolken, double bass"),
    "good-sounds": ("CC-BY-4.0", "Good-sounds dataset, MTG-UPF (Romani Picas et al.), Zenodo 820937"),
}
DYN = {"pp": "pp", "p": "p", "mp": "mp", "mf": "mf", "f": "f", "ff": "ff", "pianissimo": "pp",
       "piano": "p", "mezzo-piano": "mp", "mezzo-forte": "mf", "forte": "f", "fortissimo": "ff"}


def midi(name):
    """'A3' -> 57; 'A1(+1 oct)' -> 45 (Karoryfer's names sit an octave low); else None."""
    if not name or not isinstance(name, str):
        return None
    up = 12 if "(+1 oct)" in name else 0
    s = name.split("(")[0].strip()
    for fl, sh in FLATS.items():
        if s.startswith(fl):
            s = sh + s[2:]
    for n in sorted(NAMES, key=len, reverse=True):
        if s.startswith(n) and s[len(n):].lstrip("-").isdigit():
            return 12 * (int(s[len(n):]) + 1) + NAMES.index(n) + up
    return None


def main(cat_path, family, root, out_path):
    root = Path(root).resolve()
    out = []
    for j in json.loads(Path(cat_path).read_text(encoding="utf-8")):
        if j["family"] != family or j["source"] not in LICENCE or j.get("section") or j.get("lossy"):
            continue
        if j["tech"] not in ("arco", "pizz"):
            continue
        p = Path(j["path"])
        try:
            rel = p.resolve().relative_to(root).as_posix()
        except ValueError:
            continue
        lic, credit = LICENCE[j["source"]]
        e = {"path": rel, "source": j["source"], "licence": lic, "credit": credit, "inst": j["inst"],
             "art": j["tech"], "dyn": DYN.get(j["dyn"], "mf")}
        if j.get("multi"):
            e["note"], e["multi"] = None, True
        else:
            m = midi(j.get("nominal"))
            e["note"] = m
            if m is None:
                e["auto"] = True
        out.append(e)
    Path(out_path).write_text(json.dumps(out, indent=0, ensure_ascii=False) + "\n", encoding="utf-8")
    by = {}
    for e in out:
        by[(e["source"], e["art"])] = by.get((e["source"], e["art"]), 0) + 1
    print(f"{len(out)} references -> {out_path}: " + ", ".join(f"{s} {a} {n}" for (s, a), n in sorted(by.items())))


if __name__ == "__main__":
    main(*sys.argv[1:5])
