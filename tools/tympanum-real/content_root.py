#!/usr/bin/env python3
r"""The drums library's content root (round 42, tympanum-real): every kit under one folder that
Tympanum's own loader reads (`tympanum_real <root> ... pack:<pack>/<kit>`).

    python -I tools/tympanum-real/content_root.py

Cantus's CC0 packs as the app installs them (%LOCALAPPDATA%\Cantus\Content, or the copies
`fetch.py fetch drums-packs --packs` makes) are joined in by junction, never copied, each with
its own receipt plus the programs Tympanum plays (`instruments`, as a Tympanum pack's receipt
names them); the DrumGizmo kits of `drums-dg` get a receipt naming their kit XML; DRSKit and
the hybrid packs come from the `tympanum` set. Nothing in the app's own folder is written.
"""
import json
import os
import subprocess
from pathlib import Path
app = Path(os.environ["LOCALAPPDATA"]) / "Cantus" / "Content"
REFS = Path(os.environ.get("CANTUS_REFS", r"F:\projects\cantus-refs"))
root = REFS / "drums" / "content"
root.mkdir(parents=True, exist_ok=True)
# pack -> list of (kit id, program file, name)
SFZ = {
    "big-rusty-drums": [("sticks", "Programs/02-basic.sfz", "Big Rusty basic"), ("full", "Programs/01-full.sfz", "Big Rusty full")],
    "gogodze-phu-vol-ii": [("kit", "Programs/Kit.sfz", "Gogodze Phu II kit")],
    "unruly-drums": [("sticks", "Programs/01-kit-sticks.sfz", "Unruly sticks"), ("brushes", "Programs/02-kit-brushes.sfz", "Unruly brushes")],
    "virtuosity-drums": [("kit", "Programs/02-full-kit.sfz", "Virtuosity full")],
    "frankensnare": [("all", "Programs/01-frankensnare.sfz", "Frankensnare")],
}
def junction(dst, src):
    if dst.exists():
        return
    subprocess.run(["cmd", "/c", "mklink", "/J", str(dst), str(src)], check=True, capture_output=True)
for pack, kits in SFZ.items():
    src = app / pack
    d = root / pack
    d.mkdir(exist_ok=True)
    for sub in src.iterdir():
        if sub.name == "cantus-pack.json":
            continue
        if sub.is_dir():
            junction(d / sub.name, sub)
        elif not (d / sub.name).exists():
            (d / sub.name).write_bytes(sub.read_bytes())
    r = json.loads((src / "cantus-pack.json").read_text(encoding="utf-8"))
    r["instruments"] = [{"id": k, "name": {"en": n, "es": n}, "file": f, "shelf": "drums", "gain_db": 0.0, "plays": "tympanum"} for k, f, n in kits]
    (d / "cantus-pack.json").write_text(json.dumps(r, indent=2, ensure_ascii=False), encoding="utf-8")
# DrumGizmo kits downloaded into drums/dg and DRSKit from the tympanum set
DG = {
    "shittykit": ("ShittyKit.xml", "ShittyKit", "CC-BY-4.0", "ShittyKit, DrumGizmo (drumgizmo.org)"),
    "aasimonster": ("aasimonster.xml", "The Aasimonster", "CC-BY-4.0", "The Aasimonster, DrumGizmo (drumgizmo.org)"),
    "muldjordkit": ("MuldjordKit3.xml", "MuldjordKit", "CC-BY-4.0", "MuldjordKit, Lars Muldjord / DrumGizmo"),
}
for pack, (xml, name, lic, credit) in DG.items():
    src = (REFS / "drums" / "dg") / pack
    d = root / ("dg-" + pack)
    if not src.exists():
        print("not yet:", pack); continue
    junction(d, src) if not d.exists() else None
for pack, (xml, name, lic, credit) in DG.items():
    d = root / ("dg-" + pack)
    if not d.exists():
        continue
    if not (d / xml).exists():
        cands = sorted(p.name for p in d.glob("*.xml"))
        print(pack, "xml?", cands); continue
    r = {"id": "dg-" + pack, "version": 1, "name": {"en": name, "es": name}, "licence": lic,
         "licence_url": "https://creativecommons.org/licenses/by/4.0/legalcode", "licence_file": "",
         "credits": [{"name": credit, "url": "https://drumgizmo.org/wiki/doku.php?id=kits", "licence_url": "https://creativecommons.org/licenses/by/4.0/legalcode"}],
         "instruments": [{"id": "kit", "name": {"en": name, "es": name}, "file": xml, "shelf": "drums", "gain_db": 0.0, "plays": "tympanum"}],
         "installed_at": 0, "files": 0, "bytes": 0}
    (d / "cantus-pack.json").write_text(json.dumps(r, indent=2), encoding="utf-8")
drs = REFS / "tympanum" / "content" / "drskit-tympanum"
junction(root / "drskit-tympanum", drs)
for h in ("tympanum-hybrid-jazz", "tympanum-hybrid-studio"):
    junction(root / h, REFS / "tympanum" / "content" / h)
print(sorted(p.name for p in root.iterdir()))
