"""The hybrid's content pack: the residual, its licence and credit, its fit's report, zipped
as the manifest expects (ADR 0030: downloaded on demand, never in the repository).

    python -I residual_zip.py <kit: jazz|studio> <residual.crs> <report.txt> <out dir>

Writes `<out dir>/tympanum-hybrid-<kit>-<version>.zip` and prints the manifest entry
(sizes and SHA-256 of what was written) to paste into
`crates/cantus-content/manifest/content.json`. The zip is uploaded by the owner as a
release asset (`content-r40`) of the public repository; the entry points there.
"""
import hashlib
import io
import json
import os
import sys
import zipfile

# Bumped by every refit whose zip is to be published: a published file name is never
# replaced (an asset under the same name would fail the hash of every manifest that points
# at it), and an installed older version shows as an update. 1: drums-hyb, model tables
# 0x80D9073A; 2: refitted on r40/integration's drum sound (tables 0xED1998F7); 3: refitted on
# drums-gen 3's (the bands' high- and low-pass, the eleventh listen's tables).
VERSION = 3
DATE = (2026, 11, 14)
URL = "https://github.com/luis314159/cantus-daw/releases/download/content-r40/"

KITS = {
    "jazz": dict(
        name_en="Tympanum hybrid: Jazz Birch attacks",
        name_es="Tympanum híbrido: ataques de Jazz Birch",
        desc_en=("The recorded first moments of each piece of the Jazz Birch kit (kick, snare, toms, "
                 "hi-hat, crash, ride), fitted from Virtuosity Drums: what Cantus's model does not "
                 "already play, played under it when the kit's voice is Hybrid. About {mb} MB."),
        desc_es=("Los primeros instantes grabados de cada pieza del kit Jazz Birch (bombo, caja, toms, "
                 "charles, crash, ride), ajustados a partir de Virtuosity Drums: lo que el modelo de "
                 "Cantus aún no toca, sonando debajo de él cuando la voz del kit es Híbrida. Unos {mb} MB."),
        licence="CC0-1.0",
        licence_url="https://creativecommons.org/publicdomain/zero/1.0/legalcode",
        credit=dict(
            name="Virtuosity Drums — Versilian Studios and Karoryfer Samples, played by Austin McMahon",
            url="https://github.com/sfzinstruments/virtuosity_drums",
            licence_url="https://github.com/sfzinstruments/virtuosity_drums/blob/master/LICENSE"),
        licence_text=(
            "Tympanum hybrid: Jazz Birch attacks\n\n"
            "Residuals fitted by Cantus (tools/tympanum-fit/residual_fit.py) from the hits of\n"
            "Virtuosity Drums by Versilian Studios and Karoryfer Samples, played by Austin McMahon,\n"
            "released under CC0 1.0 Universal (public domain dedication):\n"
            "https://github.com/sfzinstruments/virtuosity_drums\n\n"
            "This pack is likewise dedicated to the public domain under CC0 1.0:\n"
            "https://creativecommons.org/publicdomain/zero/1.0/legalcode\n"),
        attribution=None,
    ),
    "studio": dict(
        name_en="Tympanum hybrid: Studio Maple attacks",
        name_es="Tympanum híbrido: ataques de Studio Maple",
        desc_en=("The recorded first moments of each piece of the Studio Maple kit (kick, snare, toms, "
                 "hi-hat, crash, ride), fitted from DRSKit: what Cantus's model does not already play, "
                 "played under it when the kit's voice is Hybrid. About {mb} MB."),
        desc_es=("Los primeros instantes grabados de cada pieza del kit Studio Maple (bombo, caja, toms, "
                 "charles, crash, ride), ajustados a partir de DRSKit: lo que el modelo de Cantus aún no "
                 "toca, sonando debajo de él cuando la voz del kit es Híbrida. Unos {mb} MB."),
        licence="CC-BY-4.0",
        licence_url="https://creativecommons.org/licenses/by/4.0/legalcode",
        credit=dict(
            name="DRSKit 2.1 — the DrumGizmo team (Deva and Lars Muldjord) and Jes Eiler of DRSDrums",
            url="https://drumgizmo.org/wiki/doku.php?id=kits:drskit",
            licence_url="https://creativecommons.org/licenses/by/4.0/"),
        licence_text=(
            "Tympanum hybrid: Studio Maple attacks\n\n"
            "Residuals fitted by Cantus (tools/tympanum-fit/residual_fit.py) from the hits of\n"
            "DRSKit 2.1 by the DrumGizmo team (Deva and Lars Muldjord) and Jes Eiler of DRSDrums,\n"
            "https://drumgizmo.org/wiki/doku.php?id=kits:drskit, licensed CC BY 4.0:\n"
            "https://creativecommons.org/licenses/by/4.0/\n\n"
            "Changes: each hit's first 30-200 ms, less what Cantus's model plays (a spectral\n"
            "residual), deconvolved to the drum's source. This pack is licensed CC BY 4.0;\n"
            "credit: DRSKit by the DrumGizmo team and Jes Eiler (DRSDrums).\n"),
        attribution=dict(
            title="DRSKit",
            author="the DrumGizmo team and Jes Eiler (DRSDrums)",
            url="https://drumgizmo.org/wiki/doku.php?id=kits:drskit",
            note=("When the samples are used in a composition, any context in which other credits "
                  "(e.g. to performers) are presented must include an attribution.")),
    ),
}


def main():
    kit, crs, report, out = sys.argv[1:5]
    k = KITS[kit]
    pid = "tympanum-hybrid-%s" % kit
    data = open(crs, "rb").read()
    files = {
        "residual.crs": data,
        "LICENSE.txt": k["licence_text"].encode("utf-8"),
        "README.txt": (("%s, version %d.\n\nThe fit's report (how much of each recording the "
                        "model already explains):\n\n" % (k["name_en"], VERSION))
                       + open(report, encoding="utf-8").read()).encode("utf-8"),
    }
    os.makedirs(out, exist_ok=True)
    name = "%s-%d.zip" % (pid, VERSION)
    path = os.path.join(out, name)
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for f, b in files.items():
            info = zipfile.ZipInfo("%s/%s" % (pid, f), date_time=DATE + (0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, b)
    zb = buf.getvalue()
    open(path, "wb").write(zb)
    mb = "%.0f" % max(1, round(len(zb) / 1e6))
    entry = {
        "id": pid,
        "version": VERSION,
        "name": {"en": k["name_en"], "es": k["name_es"]},
        "description": {"en": k["desc_en"].format(mb=mb), "es": k["desc_es"].format(mb=mb)},
        "category": "drums",
        "licence": k["licence"],
        "licence_url": k["licence_url"],
        "licence_file": "LICENSE.txt",
        "credits": [k["credit"]],
        "checked": "%04d-%02d-%02d" % DATE,
        "installed_size": sum(len(b) for b in files.values()),
        "source": {"zip": {"url": URL + name, "size": len(zb),
                           "sha256": hashlib.sha256(zb).hexdigest(), "strip": pid + "/"}},
    }
    if k["attribution"]:
        entry["attribution"] = k["attribution"]
    print(json.dumps(entry, ensure_ascii=False, indent=1))
    json.dump(entry, open(os.path.join(out, pid + ".manifest.json"), "w", encoding="utf-8"),
              ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
