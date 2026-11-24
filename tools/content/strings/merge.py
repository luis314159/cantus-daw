"""Round 42 (strings-rec, ADR 0158): put the recorded strings' packs into the manifest.

    python -I tools/content/strings/merge.py <sources> <strings.json> <content.json> [levels.json]

<sources> is what fetch_sources.sh fetched (the zips, vsco/); <strings.json> what the
strings_map example measured (each pack's `strings` build); <content.json> the manifest,
rewritten in place with each strings pack added or replaced (the others kept as they are).
[deltas.json] (written by `strings_listen -- fit`, `render.sh fit`): the decibels each
instrument and preset needs to sit at its role's level within the headroom of a velocity-127
riff with the mod wheel full (the house's preset levels, `preset_measure`), added to its gain.
Sizes and SHA-256 are computed here from the bytes, never typed (ADR 0030). The cards say
honestly what the packs are: a finished sketch — two to five dynamics, no recorded legato.
"""
import hashlib, json, os, sys, zipfile

SRC, MEASURED, MANIFEST = sys.argv[1], sys.argv[2], sys.argv[3]
DELTAS = json.load(open(sys.argv[4], encoding="utf-8")) if len(sys.argv) > 4 else {}
# Each strings instrument's gain in the manifest as it stands (what the deltas were measured on).
CURRENT = {p["id"] + "/" + i["sfz"]: i["gain_db"]
           for p in json.load(open(MANIFEST, encoding="utf-8"))["packs"] if p.get("strings")
           for i in p["strings"]["instruments"]}
CC0 = "https://creativecommons.org/publicdomain/zero/1.0/legalcode"
CHECKED = "2026-11-22"
VSCO_SHA = "440300901dfe9275fd84e0b7763af1f8443ae62e"
SKETCH_EN = " A finished sketch, honestly: {layers}, no recorded legato (Cantus scripts it by overlap), loops found by Cantus."
SKETCH_ES = " Un boceto acabado, con honradez: {capas}, sin legato grabado (Cantus lo hace por solapamiento), bucles hallados por Cantus."

PACKS = {
    "vsco2-strings": dict(
        name=("VSCO 2 Strings", "Cuerdas de VSCO 2"),
        desc=("Violin, viola and cello sections, a solo violin and a solo double bass from VSCO 2 Community Edition: sustain, tremolo, spiccato and pizzicato on keyswitches or by each note's articulation, the mod wheel playing the dynamics.",
              "Secciones de violines, violas y violonchelos, un violín solista y un contrabajo solista de VSCO 2 Community Edition: sostenido, trémolo, spiccato y pizzicato por cambio de tecla o por la articulación de cada nota, la rueda de modulación tocando la dinámica."),
        layers=("two dynamic layers", "dos capas dinámicas"),
        credits=[("VSCO 2 Community Edition — Versilian Studios (Sam Gossner), with Ivy Audio (Simon Dalzell)", "https://github.com/sgossner/VSCO-2-CE", f"https://github.com/sgossner/VSCO-2-CE/blob/{VSCO_SHA}/LICENSE")],
        licence_file="Licences/LICENSE.txt",
        shelf={"bass": "bass", "bass-nv": "bass"},
    ),
    "karoryfer-bigcat-cello": dict(
        name=("Bigcat Cello", "Violonchelo Bigcat"),
        desc=("A solo cello played by Kamila Borowiak for Karoryfer x bigcat: sustains at four dynamics in both bow directions, staccato with four takes, pizzicato — the reference Cantus's modelled cello is judged against.",
              "Un violonchelo solista tocado por Kamila Borowiak para Karoryfer x bigcat: sostenidos a cuatro dinámicas en las dos direcciones del arco, staccato con cuatro tomas, pizzicato — la referencia con la que se juzga el violonchelo modelado de Cantus."),
        layers=("four dynamics, both bows", "cuatro dinámicas, los dos arcos"),
        credits=[("Karoryfer x bigcat cello — Karoryfer Lecolds and bigcat instruments, played by Kamila Borowiak", "https://github.com/sfzinstruments/karoryfer-bigcat.cello", "https://github.com/sfzinstruments/karoryfer-bigcat.cello/blob/6fd75fbfc1dbb3109bf26220ba1adea46188a18b/LICENSE")],
        zip=("bigcat.zip", "https://github.com/sfzinstruments/karoryfer-bigcat.cello/releases/download/v1.001/Karoryfer_Bigcat_cello.v1.001.zip", "Karoryfer_Bigcat_cello/"),
        licence_file="LICENSE",
        shelf={},
    ),
    "dsmolken-double-bass": dict(
        name=("Smolken Double Bass", "Contrabajo Smolken"),
        desc=("D. Smolken's 1958 Otto Rubner double bass, tuned in fifths (C G D A): arco at five dynamics in both bow directions and pizzicato at three — the reference Cantus's modelled bass is judged against.",
              "El contrabajo Otto Rubner de 1958 de D. Smolken, afinado en quintas (Do Sol Re La): arco a cinco dinámicas en las dos direcciones del arco y pizzicato a tres — la referencia con la que se juzga el contrabajo modelado de Cantus."),
        layers=("five dynamics, both bows", "cinco dinámicas, los dos arcos"),
        credits=[("Double bass — D. Smolken", "https://github.com/sfzinstruments/dsmolken.double-bass", "https://github.com/sfzinstruments/dsmolken.double-bass/blob/c2985eb647109d2a8f30a70071e3e163339d7396/LICENSE")],
        zip=("smolken.zip", "https://github.com/sfzinstruments/dsmolken.double-bass/releases/download/v1.001/DSmolken.double_bass.v1.001.zip", "dsmolken_double_bass/"),
        licence_file="LICENSE",
        shelf={"bass": "bass"},
    ),
    "karoryfer-meatbass": dict(
        name=("Meatbass", "Meatbass"),
        desc=("Karoryfer's take on the same Rubner double bass: arco in both bow directions with its velocity layers crossfaded on the mod wheel, and pizzicato with four takes.",
              "La versión de Karoryfer del mismo contrabajo Rubner: arco en las dos direcciones con sus capas de velocidad fundidas en la rueda de modulación, y pizzicato con cuatro tomas."),
        layers=("its velocity layers", "sus capas de velocidad"),
        credits=[("Meatbass — Karoryfer Samples", "https://github.com/sfzinstruments/karoryfer.meatbass", "https://github.com/sfzinstruments/karoryfer.meatbass/blob/ac9e859564bda286ab5ec672d00ff1aa2fef2895/LICENSE")],
        zip=("meatbass.zip", "https://github.com/sfzinstruments/karoryfer.meatbass/releases/download/v1.001/Karoryfer.Meatbass.v1.001.zip", "Meatbass/"),
        licence_file="LICENSE",
        shelf={"meatbass": "bass"},
    ),
    "karoryfer-sneakybass": dict(
        name=("Sneakybass", "Sneakybass"),
        desc=("A very quiet pizzicato double bass from Karoryfer, played close and soft, four takes a note.",
              "Un contrabajo en pizzicato muy suave de Karoryfer, tocado cerca y quedo, cuatro tomas por nota."),
        layers=("one dynamic", "una dinámica"),
        credits=[("Sneakybass — Karoryfer Samples", "https://github.com/sfzinstruments/karoryfer.sneakybass", "https://github.com/sfzinstruments/karoryfer.sneakybass/blob/cc1a67e5c913678b85eaad1a3b5cb180d3be3e2d/LICENSE")],
        zip=("sneakybass.zip", "https://github.com/sfzinstruments/karoryfer.sneakybass/releases/download/v1.000/Sneakybass_v1.000.zip", "Sneakybass/"),
        licence_file="readme.txt",
        shelf={"sneakybass": "bass"},
    ),
}

ES_NAMES = {
    "Violins (section)": "Violines (sección)", "Violas (section)": "Violas (sección)",
    "Celli (section)": "Violonchelos (sección)", "Solo violin": "Violín solista",
    "Solo double bass": "Contrabajo solista", "Solo double bass, senza vibrato": "Contrabajo solista, senza vibrato",
    "Bigcat cello": "Violonchelo Bigcat", "Smolken double bass": "Contrabajo Smolken",
    "Meatbass": "Meatbass", "Sneakybass (pizzicato)": "Sneakybass (pizzicato)",
}


def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def sfz_size(build):
    zones = {i["sfz"]: len(i["zones"]) for i in build["instruments"]}
    return sum(200 * zones.get(i.get("like") or i["sfz"], 0) + 2000 for i in build["instruments"])


ARTS = {"tremolo": ("Tremolo", "Trémolo"), "spiccato": ("Spiccato", "Spiccato"),
        "staccato": ("Staccato", "Staccato"), "pizzicato": ("Pizzicato", "Pizzicato")}


def presets(build):
    """Each instrument as the pack's presets: itself (every articulation on keyswitches and by
    the note's own, the sustain first, the scripted legato on: its legato lines), and one per
    other articulation it has, that one in force (the legato off: short or trembling notes do
    not slur)."""
    out = []
    for i in build["instruments"]:
        out.append(i)
        arts = {z["art"] for z in i["zones"]}
        if len(arts) < 2:
            continue
        for art in ARTS:
            if art in arts:
                v = {k: x for k, x in i.items() if k != "zones"}
                v["zones"] = []
                v["like"] = i["sfz"]
                v["sfz"] = i["sfz"][:-4] + "-" + art + ".sfz"
                v["name"] = i["name"] + " — " + ARTS[art][0]
                v["default_art"] = art
                v.pop("legato", None)
                out.append(v)
    return {"instruments": out}


def es_name(name):
    base, _, art = name.partition(" — ")
    es = ES_NAMES.get(base, base)
    if art:
        es += " — " + next((e for k, (en, e) in ARTS.items() if en == art), art)
    return es


def entry(pid, spec, build):
    used = sorted({z["file"] for i in build["instruments"] for z in i["zones"]})
    zones_of = {i["sfz"]: len(i["zones"]) for i in build["instruments"]}
    p = {
        "id": pid, "version": 1,
        "name": {"en": spec["name"][0], "es": spec["name"][1]},
        "description": {"en": spec["desc"][0] + SKETCH_EN.format(layers=spec["layers"][0]),
                        "es": spec["desc"][1] + SKETCH_ES.format(capas=spec["layers"][1])},
        "category": "instruments", "licence": "CC0-1.0", "licence_url": CC0,
        "licence_file": spec["licence_file"],
        "credits": [{"name": n, "url": u, "licence_url": l} for n, u, l in spec["credits"]],
        "checked": CHECKED,
    }
    if "zip" in spec:
        zname, url, strip = spec["zip"]
        zpath = os.path.join(SRC, zname)
        z = zipfile.ZipFile(zpath)
        sizes = {i.filename[len(strip):]: i.file_size for i in z.infolist() if i.filename.startswith(strip)}
        folders = sorted({f.rsplit("/", 1)[0] + "/*.wav" for f in used})
        prefixes = [x[: -len("*.wav")] for x in folders]
        keep = [f for f in sizes if f.lower().endswith(".wav") and any(f.startswith(q) and "/" not in f[len(q):] for q in prefixes)]
        extra = [n for n in ("LICENSE", "readme.txt", "README.md") if n in sizes]
        p["installed_size"] = sum(sizes[f] for f in keep + extra) + sfz_size(build)
        p["source"] = {"zip": {"url": url, "size": os.path.getsize(zpath), "sha256": sha(zpath),
                               "strip": strip, "include": folders + extra}}
    else:
        files = []
        lic = os.path.join(SRC, "vsco_LICENSE")
        files.append({"url": f"https://raw.githubusercontent.com/sgossner/VSCO-2-CE/{VSCO_SHA}/LICENSE",
                      "to": "Licences/LICENSE.txt", "size": os.path.getsize(lic), "sha256": sha(lic)})
        import urllib.parse
        for f in used:
            path = os.path.join(SRC, "vsco", f)
            files.append({"url": f"https://raw.githubusercontent.com/sgossner/VSCO-2-CE/{VSCO_SHA}/" + urllib.parse.quote(f),
                          "to": f, "size": os.path.getsize(path), "sha256": sha(path)})
        p["installed_size"] = sum(x["size"] for x in files) + sfz_size(build)
        p["source"] = {"files": files}
    p["instruments"] = [{
        "id": i["sfz"].split("/")[-1][:-4], "name": {"en": i["name"], "es": es_name(i["name"])},
        "file": i["sfz"], "shelf": spec["shelf"].get(i["sfz"].split("/")[-1][:-4].split("-")[0], "samplers"),
        "gain_db": 0.0, "voices": 48,
    } for i in build["instruments"]]
    for i in build["instruments"]:
        d = DELTAS.get(pid + "/" + i["sfz"])
        if d is not None:
            # The deltas were measured on the manifest as it stands: added to its gain.
            base = CURRENT.get(pid + "/" + i["sfz"], i["gain_db"])
            i["gain_db"] = round(max(-24.0, min(24.0, base + d)), 1)
    p["strings"] = build
    return p


def main():
    measured = json.load(open(MEASURED, encoding="utf-8"))
    m = json.load(open(MANIFEST, encoding="utf-8"))
    for pid, got in measured.items():
        new = entry(pid, PACKS[pid], presets(got["strings"]))
        at = [i for i, p in enumerate(m["packs"]) if p["id"] == pid]
        if at:
            m["packs"][at[0]] = new
        else:
            m["packs"].append(new)
        print(pid, new["installed_size"] // 1_000_000, "MB", len(new["instruments"]), "instruments")
    m["format"] = max(m["format"], 6)
    m["updated"] = CHECKED
    write(m, measured.keys())


def leaf(x):
    return isinstance(x, dict) and all(not isinstance(v, (dict, list)) or (isinstance(v, list) and all(not isinstance(e, (dict, list)) for e in v)) for v in x.values())


def fmt(x, ind):
    """indent=1 JSON, with a list of flat objects (files, zones) one object to a line."""
    pad = " " * ind
    if isinstance(x, dict):
        if not x:
            return "{}"
        items = [pad + " " + json.dumps(k, ensure_ascii=False) + ": " + fmt(v, ind + 1) for k, v in x.items()]
        return "{\n" + ",\n".join(items) + "\n" + pad + "}"
    if isinstance(x, list):
        if not x:
            return "[]"
        if all(leaf(e) for e in x) and all(isinstance(e, dict) for e in x):
            items = [pad + " " + json.dumps(e, ensure_ascii=False) for e in x]
        else:
            items = [pad + " " + fmt(e, ind + 1) for e in x]
        return "[\n" + ",\n".join(items) + "\n" + pad + "]"
    return json.dumps(x, ensure_ascii=False)


def write(m, ids):
    """The manifest's text with its header updated and the packs `ids` replaced or appended,
    every other pack's text as it was."""
    text = open(MANIFEST, encoding="utf-8").read().replace("\r\n", "\n")
    text = text.replace('"format": 5,', '"format": %d,' % m["format"], 1)
    import re
    text = re.sub(r'"updated": "[0-9-]+"', '"updated": "%s"' % m["updated"], text, count=1)
    for pid in ids:
        new = fmt(next(p for p in m["packs"] if p["id"] == pid), 2)
        key = '"id": "%s",' % pid
        at = text.find("\n   " + key)
        if at >= 0:
            start = text.rfind("\n  {", 0, at) + 3
            depth, k = 0, start
            while True:
                if text[k] == "{":
                    depth += 1
                elif text[k] == "}":
                    depth -= 1
                    if depth == 0:
                        break
                elif text[k] == '"':
                    k += 1
                    while text[k] != '"':
                        k += 2 if text[k] == "\\" else 1
                k += 1
            text = text[:start] + new + text[k + 1:]
        else:
            end = text.rindex("\n ]\n}")
            text = text[:end] + ",\n  " + new + text[end:]
    json.loads(text)
    with open(MANIFEST, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


main()
