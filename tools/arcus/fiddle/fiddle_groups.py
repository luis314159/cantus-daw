"""fiddle_groups.py - which files the fiddle study measures, by group (paths under $CANTUS_REFS
or $CANTUS_WORK; see fiddle_measure.py). Each entry: (path, kind, options); options may hold
t0/t1 (seconds), extra (paths summed in, e.g. our double-stop stem), truth (a .cantus and
the track names whose notes are the ground truth), ds_only (only the double-stop measure)."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sound-refs"))
import cantus_paths as cp  # noqa: E402

F = "fiddle"
FS = F + "/freesound/"
TFF = ["621593_Bluegrass_Fiddle", "621592_Bluegrass_Fiddle_Ending", "621598_Bluesy_Violin_repeating_double_stops",
       "621596_Bluesy_Violin_Fast_liks", "621595_Bluesy_Violin_Harmonic_licks_long_improv",
       "621591_Blues_Violin_improvisation", "621590_Bluesy_Violin_Ascending_Arpegio",
       "621599_Bluesy_Violin_descending_lick", "621597_Bluesy_Violin_Descending_lick_3",
       "621594_Bluesy_Violin_descending_lick_2", "641690_Solo_fiddle_playing_part_of_a_hornpipe",
       "641699_Sad_bluesy_violin"]
CNSM = "study/cnsm-violin/recordings/Session 1/"
PHX = "study/phenicx-anechoic/PHENICX-Anechoic/audio/"
HSD = "study/hsd-string-quartet-k465/Streicherquartett_Complete_Project/Audiodateien/"
DEMO = "listen/arcus-expr/fiddle/"


def refs(p):
    return str(cp.refs(*p.split("/")))


def work(p):
    return str(cp.work(*p.split("/")))


def ours(name, stem1, stem2=None):
    d = work(DEMO + name + "-stems")
    opt = {"truth": (work(DEMO + name + ".cantus"), [stem1.split(" ", 1)[1].rsplit(".", 1)[0]])}
    return d + "/" + stem1, opt, (d + "/" + stem2) if stem2 else None


def groups():
    g = {}
    g["fiddle-bluegrass"] = [(refs(FS + n + ".preview-hq.mp3"), "fiddle", {}) for n in TFF]
    g["fiddle-tunes"] = [(refs(FS + "49783_080313_fiddle_run_on_G_major_scale.preview-hq.mp3"), "fiddle", {}),
                         (refs(F + "/commons/Willy_Wagtail.ogg"), "fiddle", {}),
                         (refs(F + "/commons/The_Devil_s_Dream.ogg"), "fiddle", {}),
                         (refs(FS + "135128_Denver_Belle.preview-hq.mp3"), "pd78", {})]   # guitar backing: timing only
    g["fiddle-78s"] = [(refs(F + "/pd78/" + n), "pd78", {}) for n in (
        "Eck_Robertson-Sallie_Gooden-Victor_18956-1922.flac",
        "Gilliland_Robertson-Arkansaw_Traveler-Victor_18956-1922.flac",
        "Don_Richardson-Mississippi_Sawyer-1916.flac",
        "Eck_Robertson-Ragtime_Annie-1922.flac")]
    # Sallie Gooden is unaccompanied: its double stops (band-limited) are measured too
    g["fiddle-78s-dstops"] = [(refs(F + "/pd78/Eck_Robertson-Sallie_Gooden-Victor_18956-1922.flac"), "fiddle",
                               {"ds_only": True})]
    g["classical-scales"] = [(refs(CNSM + f"{v}/Player {p}/scale_01.wav"), "classical", {})
                             for v, p in (("Klimke", 0), ("Klimke", 11), ("Levaggi", 2), ("Levaggi", 7),
                                          ("Stoppani", 4), ("Stoppani", 9))]
    g["classical-music"] = [(refs(PHX + "mozart/violin1.wav"), "classical", {"t1": 120.0}),
                            (refs(PHX + "beethoven/violin1.wav"), "classical", {"t1": 120.0}),
                            (refs(HSD + "Quartet Long Excerpt/Quartet_Long_Vl1.wav"), "classical", {}),
                            (refs(HSD + "Violin Phrase 1/Violin_Phrase_1_Vl2.wav"), "classical", {})]
    g["classical-dstops"] = [(refs(F + "/commons/Bach_Chaconne_BWV1004_Ben_Goldstein.ogg"), "classical",
                              {"t1": 150.0, "ds_only": True}),
                             (refs(F + "/commons/Violin_double_stop.ogg"), "classical", {"ds_only": True}),
                             (refs(F + "/commons/Violin_chords.ogg"), "classical", {"ds_only": True})]
    og = []
    for name, s1, s2 in (("hoedown", "01 Violín country.wav", "02 Violín (dobles cuerdas).wav"),
                         ("tren", "01 Violín country.wav", "02 Violín (dobles cuerdas).wav"),
                         ("pasaje-country", "01 Violín.wav", "02 Violín (dobles cuerdas).wav")):
        p, opt, p2 = ours(name, s1, s2)
        og.append((p, "ours", opt))
    g["ours-country"] = og
    p, opt, _ = ours("pasaje-clasico", "01 Violín.wav")
    g["ours-classical"] = [(p, "ours", opt)]
    dd = []
    for name, s1, s2 in (("hoedown", "01 Violín country.wav", "02 Violín (dobles cuerdas).wav"),
                         ("tren", "01 Violín country.wav", "02 Violín (dobles cuerdas).wav"),
                         ("pasaje-country", "01 Violín.wav", "02 Violín (dobles cuerdas).wav")):
        p, opt, p2 = ours(name, s1, s2)
        dd.append((p, "ours", {"extra": [p2], "ds_only": True}))
    g["ours-dstops"] = dd
    g["ours-room"] = [(work(DEMO + "pasaje-country.wav"), "ours", {})]
    # controls: the same take as 16-bit WAV and as MP3 (~100-125 kbit/s mono, libsndfile's LAME,
    # written by hand into $CANTUS_WORK/arcus/fiddle/control): how much the previews' lossy
    # coding alone moves the noise and HNR measures
    g["control-wav"] = [(work("arcus/fiddle/control/" + n + ".wav"), "ours", {})
                        for n in ("ours_pasaje_country_01", "ours_tren_01", "cnsm_klimke_p0")]
    g["control-mp3"] = [(work("arcus/fiddle/control/" + n + ".mp3"), "ours", {})
                        for n in ("ours_pasaje_country_01", "ours_tren_01", "cnsm_klimke_p0")]
    return g
