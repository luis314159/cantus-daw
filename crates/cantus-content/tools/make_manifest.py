"""Write crates/cantus-content/manifest/content.json from the hashed file lists and the
downloaded archives: sizes and SHA-256 are computed here, never typed by hand (ADR 0030).

    python make_manifest.py <work dir> <out content.json>

<work dir> holds the lists build_packs.py wrote and, in <work dir>/dl, the release
archives (download them with curl; check each starts with "PK")."""
import hashlib, json, os, re, sys, zipfile

HERE = os.path.abspath(sys.argv[1])
DL = os.path.join(HERE, "dl")
OUT = sys.argv[2]
CC0 = "https://creativecommons.org/publicdomain/zero/1.0/legalcode"
# ADR 0042: the licences content may be under, and their texts. A CC BY pack also needs
# its credit line: attribution(title, author, url[, line, note]).
LICENCES = {
    "CC0-1.0": CC0,
    "CC-BY-3.0": "https://creativecommons.org/licenses/by/3.0/legalcode",
    "CC-BY-4.0": "https://creativecommons.org/licenses/by/4.0/legalcode",
}
CHECKED = "2026-10-05"


def glob(p, s):
    rx = ""
    i = 0
    while i < len(p):
        if p.startswith("**/", i):
            rx += "(?:.*/)?"; i += 3
        elif p.startswith("**", i):
            rx += ".*"; i += 2
        elif p[i] == "*":
            rx += "[^/]*"; i += 1
        else:
            rx += re.escape(p[i]); i += 1
    return re.fullmatch(rx, s, re.I) is not None


def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def zip_pack(file, url, strip, include, exclude, sha256=None):
    path = os.path.join(DL, file)
    z = zipfile.ZipFile(path)
    size = 0
    for i in z.infolist():
        if i.is_dir() or not i.filename.startswith(strip):
            continue
        rel = i.filename[len(strip):]
        if rel and (not include or any(glob(p, rel) for p in include)) and not any(glob(p, rel) for p in exclude):
            size += i.file_size
    src = {"url": url, "size": os.path.getsize(path), "sha256": sha256 or sha(path)}
    if strip:
        src["strip"] = strip
    if include:
        src["include"] = include
    if exclude:
        src["exclude"] = exclude
    return {"zip": src}, size


def tar_gz_pack(file, url, strip, include, exclude, sha256=None):
    """A `.tar.gz` source (round 26): the same rules as a zip, sizes from the archive."""
    import tarfile
    path = os.path.join(DL, file)
    size = 0
    with tarfile.open(path, "r:gz") as t:
        for i in t:
            if not i.isfile() or not i.name.startswith(strip):
                continue
            rel = i.name[len(strip):]
            if rel and (not include or any(glob(p, rel) for p in include)) and not any(glob(p, rel) for p in exclude):
                size += i.size
    src = {"url": url, "size": os.path.getsize(path), "sha256": sha256 or sha(path)}
    if strip:
        src["strip"] = strip
    if include:
        src["include"] = include
    if exclude:
        src["exclude"] = exclude
    return {"tar_gz": src}, size


def files_pack(name):
    files = json.load(open(os.path.join(HERE, name + ".json")))
    return {"files": files}, sum(f["size"] for f in files)


def credit(name, url, licence_url):
    return {"name": name, "url": url, "licence_url": licence_url}


VD = credit("Virtuosity Drums — Versilian Studios and Karoryfer Samples",
            "https://github.com/sfzinstruments/virtuosity_drums",
            "https://github.com/sfzinstruments/virtuosity_drums/blob/9f04cf9a734527edfbb0a4eee1f674e45bbf71bc/LICENSE")
BP = credit("Body Percussion — sfzinstruments",
            "https://github.com/sfzinstruments/body_percussion",
            "https://github.com/sfzinstruments/body_percussion/blob/4ac9d8966679c648b62fa10a188179e186b97f24/LICENSE")
VC = credit("Versilian Community Sample Library (VCSL) — Versilian Studios",
            "https://github.com/sgossner/VCSL",
            "https://github.com/sgossner/VCSL/blob/c1ea7bcc3c7309650ab0da9d15c9cd1fbc4a4c7e/LICENSE")
VS = credit("VSCO 2 Community Edition — Versilian Studios",
            "https://github.com/sgossner/VSCO-2-CE",
            "https://github.com/sgossner/VSCO-2-CE/blob/440300901dfe9275fd84e0b7763af1f8443ae62e/LICENSE")


def kar(repo, branch, file="LICENSE"):
    return credit("Karoryfer Samples", "https://github.com/sfzinstruments/" + repo,
                  "https://github.com/sfzinstruments/%s/blob/%s/%s" % (repo, branch, file))


packs = []


def attribution(title, author, url, line="", note=""):
    """A CC BY pack's credit as its licensor asks: `line` is their own wording, word for
    word, when they give one; `note` their words about crediting music made with it."""
    a = {"title": title, "author": author, "url": url}
    if line:
        a["line"] = line
    if note:
        a["note"] = note
    return a


def add(id, cat, name, desc, source_size, licence_file, credits, recommended=False,
        licence="CC0-1.0", attribution=None, instruments=None, checked=CHECKED):
    source, size = source_size
    assert licence in LICENCES, licence + " is not accepted (ADR 0042)"
    # CC BY needs its credit line; a CC0 pack may carry one as a courtesy its author asked for.
    assert attribution is not None or licence == "CC0-1.0", id + ": CC BY needs a credit line"
    p = {"id": id, "version": 1, "name": {"en": name[0], "es": name[1]},
         "description": {"en": desc[0], "es": desc[1]}, "category": cat}
    if recommended:
        p["recommended"] = True
    p.update({"licence": licence, "licence_url": LICENCES[licence], "licence_file": licence_file,
              "credits": credits, "checked": checked, "installed_size": size, "source": source})
    if attribution is not None:
        p["attribution"] = attribution
    if instruments:
        p["instruments"] = instruments
    packs.append(p)


add("cantus-starter-drums", "drums",
    ("Cantus Starter Drums", "Batería inicial de Cantus"),
    ("A curated acoustic kit to start with: kicks, snares, hats, toms, cymbals, claps and "
     "percussion, one hit per sound, from three CC0 libraries.",
     "Un kit acústico escogido para empezar: bombos, cajas, charles, toms, platillos, "
     "palmas y percusión, un golpe por sonido, de tres bibliotecas CC0."),
    files_pack("starter"), "Licences/Virtuosity Drums - LICENSE.txt", [VD, BP, VC], True)
add("afr-kicks-2024", "drums",
    ("AFR Kicks 2024", "AFR Kicks 2024"),
    ("Sixteen synthesized electronic kicks — 606, 707, 808 and 909 styles, deep house, "
     "experimental — by Alexander F. Rødseth.",
     "Dieciséis bombos electrónicos sintetizados — estilos 606, 707, 808 y 909, deep house, "
     "experimentales — de Alexander F. Rødseth."),
    zip_pack("afr-kicks-2024.zip", "https://github.com/xyproto/synth/releases/download/v1.4.4/afr-kicks-2024.zip",
             "afr-kicks-2024/", [], []),
    "CC0-license.txt",
    [credit("AFR Kick Drum Sample Pack 2024 — Alexander F. Rødseth (xyproto)",
            "https://github.com/xyproto/synth/releases/tag/v1.4.4",
            "https://github.com/xyproto/synth/releases/tag/v1.4.4")])
add("body-percussion", "percussion",
    ("Body Percussion", "Percusión corporal"),
    ("Claps, snaps, stomps, chest and belly hits, with round robins and two dynamics.",
     "Palmas, chasquidos, pisotones, golpes de pecho y vientre, con variaciones y dos dinámicas."),
    files_pack("body"), "Licences/LICENSE.txt", [BP])
for vol, file, tag, sz_name in [
        ("II", "Karoryfer_Gogodze_Phu_vol_II.v1.001.zip", "v1.001", "gogodze-phu-vol-ii"),
        ("I", "Karoryfer.Gogodze_Phu_vol_I.v1.001.zip", "v1.001", "gogodze-phu-vol-i")]:
    repo = "karoryfer." + sz_name
    if vol == "II":
        desc = ("A vintage kit with character: kick, snare, three deadened toms and a hi-hat in "
                "five openness steps, seven microphones.",
                "Una batería antigua con carácter: bombo, caja, tres toms apagados y un charles en "
                "cinco aperturas, siete micrófonos.")
        cat = "drums"
    else:
        desc = ("Five Ghanaian bobobo drums and a cajón.",
                "Cinco tambores bobobo de Ghana y un cajón.")
        cat = "percussion"
    add(sz_name, cat, ("Gogodze Phu Vol. " + vol, "Gogodze Phu Vol. " + vol), desc,
        zip_pack(file, "https://github.com/sfzinstruments/%s/releases/download/%s/%s" % (repo, tag, file),
                 "Gogodze_Phu_vol_%s/" % vol, ["Samples/**", "Programs/**", "LICENSE", "readme.txt"], []),
        "LICENSE", [kar(repo, "master")])
add("frankensnare", "drums",
    ("Frankensnare", "Frankensnare"),
    ("About a dozen snares from 10\" to 20\", sticks and brushes, rimshots, sympathetic "
     "snares, claps and tambourine — the sample folders and SFZ programs.",
     "Una docena de cajas de 10\" a 20\", baquetas y escobillas, aros, cajas por simpatía, "
     "palmas y pandereta — las carpetas de muestras y los programas SFZ."),
    zip_pack("Frankensnare_2100.zip",
             "https://github.com/sfzinstruments/karoryfer.frankensnare/releases/download/v2.100/Frankensnare_2100.zip",
             "", ["Samples/**", "Programs/**", "license"], []),
    "license", [kar("karoryfer.frankensnare", "main", "license")])
add("big-rusty-drums", "drums",
    ("Big Rusty Drums", "Big Rusty Drums"),
    ("A deeply sampled 1980s Polish kit: sticks, brushes, mallets and stirs, a cymbal stack.",
     "Una batería polaca de los ochenta muestreada a fondo: baquetas, escobillas, mazas y "
     "barridos, una pila de platillos."),
    zip_pack("Big_Rusty_Drums_1100.zip",
             "https://github.com/sfzinstruments/karoryfer.big-rusty-drums/releases/download/v1.100/Big_Rusty_Drums_1100.zip",
             "", ["Samples/**", "Programs/**", "LICENSE"], []),
    "LICENSE", [kar("karoryfer.big-rusty-drums", "main")])
add("swirly-drums-dry", "drums",
    ("Swirly Drums (dry)", "Swirly Drums (seca)"),
    ("A brushed kit with unusual percussion — cajón kick, darbouka, djembe, bongos, a brushed "
     "cowbell. The dry samples only: the two reverberant copies (1.4 GB) are left out.",
     "Una batería con escobillas y percusión poco común — cajón como bombo, darbuka, yembé, "
     "bongós, cencerro con escobilla. Solo las muestras secas: las dos copias con "
     "reverberación (1,4 GB) se dejan fuera."),
    zip_pack("Swirly.Drums_1104.zip",
             "https://github.com/sfzinstruments/karoryfer.swirly-drums/releases/download/v1.104/Swirly.Drums_1104.zip",
             "", ["Samples/**", "license"], ["Samples/wet/**", "Samples/wetter/**"]),
    "license", [kar("karoryfer.swirly-drums", "main", "license")])
if os.path.exists(os.path.join(DL, "Unruly_Drums_1100.zip")):
    add("unruly-drums", "drums",
        ("Unruly Drums", "Unruly Drums"),
        ("A quirky kit where every drum has snare wires on it, kick included; a hi-hat in six "
         "openness steps.",
         "Una batería singular en la que todos los tambores llevan bordonero, bombo incluido; "
         "un charles en seis aperturas."),
        zip_pack("Unruly_Drums_1100.zip",
                 "https://github.com/sfzinstruments/karoryfer.unruly-drums/releases/download/v1.100/Unruly_Drums_1100.zip",
                 "", ["Samples/**", "Programs/**", "LICENSE", "license"], []),
        "LICENSE", [kar("karoryfer.unruly-drums", "main")])
add("vsco2-percussion", "percussion",
    ("VSCO 2 Percussion", "Percusión de VSCO 2"),
    ("Orchestral and hand percussion from VSCO 2 Community Edition: bass drum, timpani, "
     "snares, cymbals, congas, claves, cowbell, triangle, glockenspiel, marimba and more.",
     "Percusión orquestal y de mano de VSCO 2 Community Edition: bombo, timbales, cajas, "
     "platillos, congas, claves, cencerro, triángulo, glockenspiel, marimba y más."),
    files_pack("vsco-perc"), "Licences/LICENSE.txt", [VS])
add("vcsl-tx81z", "instruments",
    ("TX81Z Multisamples (VCSL)", "Multimuestras TX81Z (VCSL)"),
    ("Multisampled patches of a Yamaha TX81Z FM synthesizer from the Versilian Community "
     "Sample Library: basses, keys, pads and leads to load into Exemplar.",
     "Sonidos multimuestreados de un sintetizador FM Yamaha TX81Z de la Versilian Community "
     "Sample Library: bajos, teclados, pads y solistas para cargar en Exemplar."),
    files_pack("tx81z"), "Licences/LICENSE.txt", [VC])

SAL = "SalamanderGrandPiano-SFZ+FLAC-V3+20200602"
SAL_PAGE = "https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html"
SAL_SFZ = "SalamanderGrandPiano-V3+20200602.sfz"


def piano(id, en, es, layers=None):
    i = {"id": id, "name": {"en": en, "es": es}, "file": SAL_SFZ, "shelf": "keys",
         "gain_db": -5.5}
    if layers:
        i["layers"] = layers
    return i


add("salamander-grand-piano", "instruments",
    ("Salamander Grand Piano", "Piano de cola Salamander"),
    ("A Yamaha C5 grand recorded by Alexander Holm: 16 velocity layers sampled in minor "
     "thirds, string resonance and hammer releases (48 kHz, 24-bit FLAC). Plays in Exemplar, "
     "streamed from disk: the instrument picker offers it with 8 velocity layers (about "
     "120 MB of memory) and with all 16 (about 185 MB); its first load decodes the samples "
     "once into Cantus's cache (1 GB for 8 layers, 2 GB for 16).",
     "Un Yamaha C5 de cola grabado por Alexander Holm: 16 capas de velocidad muestreadas en "
     "terceras menores, resonancias de cuerda y relajaciones de martillo (48 kHz, FLAC de 24 "
     "bits). Suena en Exemplar, leído del disco: el selector de instrumentos lo ofrece con 8 "
     "capas de velocidad (unos 120 MB de memoria) y con las 16 (unos 185 MB); la primera "
     "carga descodifica las muestras una vez en la caché de Cantus (1 GB con 8 capas, 2 GB "
     "con 16)."),
    tar_gz_pack(SAL + ".tar.gz",
                "https://freepats.zenvoid.org/Piano/SalamanderGrandPiano/" + SAL + ".tar.gz",
                SAL + "/", ["samples/**", "readme.txt", SAL_SFZ], []),
    "readme.txt",
    [credit("Salamander Grand Piano V3 — Alexander Holm; FLAC edition by FreePats",
            SAL_PAGE, "https://creativecommons.org/licenses/by/3.0/")],
    licence="CC-BY-3.0",
    attribution=attribution("Salamander Grand Piano V3", "Alexander Holm", SAL_PAGE),
    checked="2026-10-13",
    instruments=[piano("grand", "Salamander Grand Piano", "Piano de cola Salamander", 8),
                 piano("grand-full", "Salamander Grand Piano · 16 layers",
                       "Piano de cola Salamander · 16 capas")])

# Format 2 only when a pack needs it (a credit line, a tar.gz, instruments), so format-1
# builds keep reading the rest.
fmt = 2 if any("attribution" in p or "instruments" in p or "tar_gz" in p["source"] for p in packs) else 1
manifest = {"format": fmt, "updated": max(p["checked"] for p in packs), "packs": packs}
with open(OUT, "w", encoding="utf-8", newline="\n") as f:
    text = json.dumps(manifest, indent=1, ensure_ascii=False)
    # One line per file entry: the lists stay readable in a diff.
    text = re.sub(r'\{\n\s*("url": [^\n]*)\n\s*("to": [^\n]*)\n\s*("size": [^\n]*)\n\s*("sha256": "[0-9a-f]+")\n\s*\}',
                  lambda m: "{" + " ".join(m.groups()) + "}", text)
    f.write(text + "\n")
for p in packs:
    src = p["source"]
    dl = src["zip"]["size"] if "zip" in src else sum(x["size"] for x in src["files"])
    print("%-24s %8.1f MB download %8.1f MB installed" % (p["id"], dl / 1e6, p["installed_size"] / 1e6))
print(os.path.getsize(OUT), "bytes of manifest")
