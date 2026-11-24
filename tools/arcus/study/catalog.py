"""catalog.py - list every reference note of the Arcus study as analysis jobs.

Usage:  venv\\Scripts\\python.exe -I tools\\catalog.py [refdb\\catalog.json]
Reads the reference folders below (nothing is written there) and writes one job per file:
{path, source, inst, family, tech, dyn, dyn_id, nominal, string, lossy, section, multi}.
`inst` names one physical instrument (or one player's instrument): the database's spread is
taken across these, never across files. `multi` marks a file holding several notes (Iowa).
Dynamics are ordinals: pp 1, p 2, mp 3, mf 4, f 5, ff 6.
"""
import os
import csv
import json
import re
import sys
from pathlib import Path

D = Path(os.environ.get("CANTUS_STUDY_DOWNLOADS", r"F:\projects\daw-r42\downloads"))
FAM = {"cello": (60.0, 1100.0), "bass": (28.0, 520.0), "viola": (120.0, 1700.0),
       "violin": (180.0, 3400.0)}
DYN = {"pp": 1, "p": 2, "mp": 3, "mf": 4, "f": 5, "ff": 6, "pianissimo": 1, "molto-pianissimo": 1,
       "piano": 2, "mezzo-piano": 3, "mezzo-forte": 4, "forte": 5, "fortissimo": 6}


def job(path, source, inst, family, tech, dyn, nominal=None, string=None, lossy=False,
        section=False, multi=False):
    return {"path": str(path), "source": source, "inst": inst, "family": family, "tech": tech,
            "dyn": dyn, "dyn_id": DYN.get(dyn), "nominal": nominal, "string": string,
            "lossy": lossy, "section": section, "multi": multi}


def iowa(jobs):
    fams = {"Cello": "cello", "Bass": "bass", "Violin": "violin", "Viola": "viola"}
    for root in [D / "arcus" / "iowa", D / "study" / "iowa-violin-viola"]:
        for p in sorted(root.rglob("*.aif*")):
            m = re.match(r"(Cello|Bass|Violin|Viola)\.(arco|pizz)\.(pp|mf|ff)\.sul([A-G])\.", p.name)
            if m:
                inst, tech, dyn, st = m.groups()
            else:  # the viola's files put the string before the dynamic
                m = re.match(r"(Cello|Bass|Violin|Viola)\.(arco|pizz)\.sul([A-G])\.(pp|mf|ff)\.", p.name)
                if not m:
                    continue
                inst, tech, st, dyn = m.groups()
            fam = fams[inst]
            jobs.append(job(p, "iowa", "iowa-" + fam, fam, tech, dyn, string=st, multi=True))


def tinysol(jobs):
    fams = {"Vc": "cello", "Cb": "bass", "Vn": "violin", "Va": "viola"}
    for root in [D / "arcus" / "tinysol", D / "study" / "tinysol-violin-viola"]:
        meta = next(iter(root.rglob("TinySOL_metadata.csv")), None)
        if not meta:
            continue
        with open(meta, newline="", encoding="utf-8") as fh:
            for row in csv.DictReader(fh):
                ab = row["Instrument (abbr.)"]
                if ab not in fams:
                    continue
                p = root / row["Path"]
                if not p.exists():
                    hits = list(root.rglob(Path(row["Path"]).name))
                    if not hits:
                        continue
                    p = hits[0]
                fam = fams[ab]
                jobs.append(job(p, "tinysol", "ircam-" + fam, fam, "arco", row["Dynamics"],
                                row["Pitch"], row["String ID (if applicable)"] or None))


def karoryfer(jobs):
    root = D / "strings-rec" / "x" / "karoryfer-bigcat-cello" / "Samples"
    for p in sorted((root / "sus").glob("*.wav")):
        m = re.match(r"([A-G]b?\d)_(p|mp|mf|f)_([dg])\.wav", p.name)
        if m:
            jobs.append(job(p, "karoryfer", "karoryfer-bigcat-cello", "cello", "arco", m.group(2),
                            m.group(1) + "(+1 oct)", None))
    for p in sorted((root / "pizzcato").glob("*_1.wav")):
        jobs.append(job(p, "karoryfer", "karoryfer-bigcat-cello", "cello", "pizz", "na", p.stem))


def smolken(jobs):
    root = D / "strings-rec" / "x" / "dsmolken-double-bass"
    for p in sorted((root / "arco").glob("arco_*.wav")):
        m = re.match(r"arco_([a-g]#?\d)_(pp|p|mp|mf|f|ff)_(down|up)\.wav", p.name)
        if m:
            jobs.append(job(p, "smolken", "smolken-bass", "bass", "arco", m.group(2), m.group(1)))
    for p in sorted((root / "pizz").glob("pizz_*.wav")):
        m = re.match(r"pizz_([a-g]#?\d)_(pp|p|mp|mf|f|ff)a\.wav", p.name)
        if m:
            jobs.append(job(p, "smolken", "smolken-bass", "bass", "pizz", m.group(2), m.group(1)))


def vsco(jobs):
    root = D / "strings-rec" / "vsco" / "Strings"
    sets = [("Solo Contrabass/SusVib", "vsco-solo-bass", "bass", "arco", False),
            ("Solo Contrabass/SusNV", "vsco-solo-bass", "bass", "arco-nv", False),
            ("Solo Contrabass/Pizz", "vsco-solo-bass", "bass", "pizz", False),
            ("Solo Violin/Arco Vib", "vsco-solo-violin", "violin", "arco", False),
            ("Solo Violin/Pizz", "vsco-solo-violin", "violin", "pizz", False),
            ("Cello Section/susvib", "vsco-cello-section", "cello", "arco", True),
            ("Viola Section/susvib", "vsco-viola-section", "viola", "arco", True),
            ("Violin Section/susVib", "vsco-violin-section", "violin", "arco", True)]
    for sub, inst, fam, tech, sec in sets:
        for p in sorted((root / sub).glob("*.wav")):
            n = p.stem
            if "_rr2" in n or re.search(r"_2$", n):
                continue
            m = re.search(r"_v(\d)", n)
            dyn = {"1": "p", "2": "mf", "3": "f"}.get(m.group(1)) if m else ("f" if n.endswith("_f") else "p" if n.endswith("_p") else None)
            jobs.append(job(p, "vsco", inst, fam, tech, dyn, n, section=sec))


def philharmonia(jobs):
    fams = {"cello": "cello", "double-bass": "bass", "violin": "violin", "viola": "viola"}
    for p in sorted((D / "study").glob("philharmonia*/**/*.mp3")):
        parts = p.stem.split("_")
        if len(parts) != 5 or parts[0] not in fams or parts[2] == "phrase":
            continue
        fam, note, dur, dyn, tech = fams[parts[0]], parts[1], parts[2], parts[3], parts[4]
        t = {"arco-normal": "arco", "molto-vibrato": "arco-mv", "non-vibrato": "arco-nv",
             "pizz-normal": "pizz"}.get(tech)
        if not t or (t.startswith("arco") and dur not in ("1", "15")):
            continue
        jobs.append(job(p, "philharmonia", "philharmonia-" + fam, fam, t, dyn, note, lossy=True))


def goodsounds(jobs):
    meta = D / "study" / "good-sounds" / "meta"  # one dataset; the violin's audio sits apart
    if not (meta / "sounds.json").exists():
        return
    sounds = json.loads((meta / "sounds.json").read_text(encoding="utf-8"))
    packs = json.loads((meta / "packs.json").read_text(encoding="utf-8"))
    pname = {str(v.get("id", k)): v.get("name") for k, v in packs.items()}
    for root in (D / "study").glob("good-sounds*"):
        for s in sounds.values():
            if s.get("instrument") not in ("cello", "violin", "viola") or s.get("klass") != "good-sound":
                continue
            pack = pname.get(str(s["pack_id"]))
            hits = list(root.rglob(f"sound_files/{pack}/neumann/{s['pack_filename']}"))
            if not hits:
                continue
            dyn = s.get("dynamics") if s.get("dynamics") in DYN else "na"
            jobs.append(job(hits[0], "good-sounds", f"goodsounds-{s['instrument']}-{s['player']}",
                            s["instrument"], "arco", dyn, f"{s['note']}{s['octave']}", s.get("string")))


def main():
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[3] / "research" / "instruments-next" / "bowed-refdb" / "catalog.json"
    jobs = []
    for f in (iowa, tinysol, karoryfer, smolken, vsco, philharmonia, goodsounds):
        f(jobs)
    out.write_text(json.dumps(jobs, indent=0))
    from collections import Counter
    c = Counter((j["family"], j["inst"], j["tech"]) for j in jobs)
    for k in sorted(c):
        print(k, c[k])
    print("total", len(jobs))


if __name__ == "__main__":
    main()
