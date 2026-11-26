"""The strings-hq references (tools/sound-refs/catalog.toml, set `strings-hq`) added to a
family's lab catalogue (round 42, strings-refs).

    python -I tools/soundlab/adapters/strings_hq.py <family> <refs root> <base.json> <out.json>

<base.json> is the study's free catalogue for the family (adapters/study_catalogue.py); the
output is it plus, under the same root:
- the University of Iowa MIS **2012** recordings (anechoic chamber, Earthworks QTC40 at 5 ft,
  mono 24-bit 96 kHz): runs of notes at pp / mf / ff, arco and pizzicato (`multi`: the lab cuts
  each run at its silences and names each note by its own pitch);
- the Good-sounds double bass (player "alejandro", Neumann mic, 24-bit 48 kHz), bowed and
  plucked single notes, named by the dataset's database (`study/good-sounds`);
- for the cello, Good-sounds' margarita open strings at p / mf / f (bow at the middle of the
  string only: the tasto and ponticello takes are left out).
The phrases (Musopen K465's spot microphones, HSD's stems) are not single notes: the
subject's `phrases` section names them, for the phrase measures.
"""
import json
import re
import sqlite3
import sys
from pathlib import Path

NAMES = "C C# D D# E F F# G G# A A# B".split()
IOWA = "University of Iowa Electronic Music Studios, Musical Instrument Samples (2012)"
IOWA_INST = {
    "violin": "Nicolai Tambovsky violin (1998), played by Leonid Iogansen, 2011-12-11",
    "viola": "Anton Krutz viola (2001), played by Manuel Tabora, 2012-05-11",
    "cello": "Charles Quenoil cello (1923), played by Yoo-Jung Chang, 2012-05-11",
    "bass": "Anton Krutz double bass (2004), played by Mike White, 2012-05-11",
}
GS = "Good-sounds dataset, MTG-UPF (Romani Picas, Parra Rodriguez, Dabiri, Serra), Zenodo 820937"
GS_DB = "study/good-sounds/zenodo/good-sounds/database.sqlite"


def midi(note, octave):
    return 12 * (int(octave) + 1) + NAMES.index(note.strip())


def iowa(root, family):
    out = []
    d = root / "strings-hq" / "iowa2012" / family
    for p in sorted(d.glob("*.aif")):
        n = p.name
        art = re.search(r"\.(arco|pizz)\.", n).group(1)
        dyn = re.search(r"\.(pp|mf|ff)\.", n).group(1)
        out.append({"path": p.relative_to(root).as_posix(), "source": "iowa2012", "licence": "Iowa-MIS",
                    "credit": f"{IOWA}: {IOWA_INST[family]}", "inst": f"iowa2012-{family}",
                    "art": art, "dyn": dyn, "note": None, "multi": True})
    return out


def gs_rows(root, sql, args=()):
    c = sqlite3.connect(f"file:{(root / GS_DB).as_posix()}?mode=ro", uri=True)
    try:
        return list(c.execute(sql, args))
    finally:
        c.close()


def gs_bass(root):
    out = []
    rows = gs_rows(root, "select s.note, s.octave, trim(s.klass), t.filename from sounds s join takes t "
                         "on t.sound_id = s.id where s.instrument = 'bass' and t.microphone = 'neumann'")
    for note, octave, klass, fn in rows:
        p = root / "strings-hq" / "good-sounds-bass" / fn
        if not p.exists():
            continue
        art, dyn = {"good-sound-bow": ("arco", "mf"), "good-sound-pizzicato-hard": ("pizz", "f"),
                    "good-sound-pizzicato-soft": ("pizz", "p")}[klass]
        out.append({"path": p.relative_to(root).as_posix(), "source": "good-sounds", "licence": "CC-BY-4.0",
                    "credit": f"{GS} (double bass, player alejandro)", "inst": "goodsounds-bass-alejandro",
                    "art": art, "dyn": dyn, "note": midi(note, octave)})
    return out


def gs_cello_open(root):
    out = []
    rows = gs_rows(root, "select s.note, s.octave, s.dynamics, t.filename from sounds s join packs p on "
                         "p.id = s.pack_id join takes t on t.sound_id = s.id where p.name = "
                         "'cello_margarita_open_strings' and s.dynamics in ('p', 'mf', 'f') and "
                         "t.microphone = 'neumann' and (s.bridge_position is null or "
                         "s.bridge_position in ('', 'middle'))")
    for note, octave, dyn, fn in rows:
        rel = Path("study/good-sounds/zenodo/good-sounds") / fn
        if not (root / rel).exists():
            continue
        out.append({"path": rel.as_posix(), "source": "good-sounds", "licence": "CC-BY-4.0",
                    "credit": f"{GS} (cello, player margarita, open strings)",
                    "inst": "goodsounds-cello-margarita", "art": "arco", "dyn": dyn,
                    "note": midi(note, octave)})
    return out


def main(family, root, base, out_path):
    root = Path(root).resolve()
    cat = json.loads(Path(base).read_text(encoding="utf-8"))
    have = {e["path"] for e in cat}
    add = iowa(root, family)
    if family == "bass":
        add += gs_bass(root)
    if family == "cello":
        add += gs_cello_open(root)
    add = [e for e in add if e["path"] not in have]
    cat += add
    Path(out_path).write_text(json.dumps(cat, indent=0, ensure_ascii=False) + "\n", encoding="utf-8")
    by = {}
    for e in add:
        by[(e["inst"], e["art"])] = by.get((e["inst"], e["art"]), 0) + 1
    print(f"{family}: {len(cat)} references ({len(add)} added: "
          + ", ".join(f"{i} {a} {n}" for (i, a), n in sorted(by.items())) + f") -> {out_path}")


if __name__ == "__main__":
    main(*sys.argv[1:5])
