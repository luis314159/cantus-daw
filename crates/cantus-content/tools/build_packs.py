"""Build the file lists of Cantus's file-list packs (ADR 0030): download each file from a
pinned commit, check it is audio and not a proxy's page, hash it, and write a JSON list.

    python build_packs.py <work dir> starter|body|tx81z|vsco-perc

Needs `curl` and `git` on the PATH. Downloads are cached in <work dir>/cache; the lists
are written to <work dir>/<name>.json for make_manifest.py. Nothing here ships."""
import hashlib, json, os, subprocess, sys, time, urllib.parse

HERE = os.path.abspath(sys.argv[1])
CACHE = os.path.join(HERE, "cache")
REPOS = {
    "body_percussion": "https://github.com/sfzinstruments/body_percussion",
    "VCSL": "https://github.com/sgossner/VCSL",
    "VSCO-2-CE": "https://github.com/sgossner/VSCO-2-CE",
}
VD = ("https://raw.githubusercontent.com/sfzinstruments/virtuosity_drums/"
      "9f04cf9a734527edfbb0a4eee1f674e45bbf71bc/")
BP = ("https://raw.githubusercontent.com/sfzinstruments/body_percussion/"
      "4ac9d8966679c648b62fa10a188179e186b97f24/")
VC = ("https://raw.githubusercontent.com/sgossner/VCSL/"
      "c1ea7bcc3c7309650ab0da9d15c9cd1fbc4a4c7e/")
VS = ("https://raw.githubusercontent.com/sgossner/VSCO-2-CE/"
      "440300901dfe9275fd84e0b7763af1f8443ae62e/")


def fetch(url):
    """Download (cached), refusing anything that is not RIFF/fLaC/OggS/text licence."""
    key = hashlib.sha1(url.encode()).hexdigest()
    path = os.path.join(CACHE, key)
    if not os.path.exists(path):
        os.makedirs(CACHE, exist_ok=True)
        q = urllib.parse.quote(url, safe=":/")
        for attempt in range(8):
            r = subprocess.run(["curl", "-sSL", "--fail", "-m", "300", "-o", path + ".part", q])
            if r.returncode == 0:
                with open(path + ".part", "rb") as f:
                    head = f.read(4)
                if head in (b"RIFF", b"fLaC", b"OggS", b"FORM") or url.endswith(("LICENSE", ".txt", ".md")):
                    if not head.startswith(b"<"):
                        os.replace(path + ".part", path)
                        break
            print("retry", attempt, url, file=sys.stderr)
            time.sleep(20)
        else:
            raise SystemExit("could not fetch " + url)
    data = open(path, "rb").read()
    return len(data), hashlib.sha256(data).hexdigest()


def entry(url, to):
    size, sha = fetch(url)
    return {"url": urllib.parse.quote(url, safe=":/"), "to": to, "size": size, "sha256": sha}


def starter():
    out = []
    def vd(src, to):
        out.append(entry(VD + src, to))
    k = "Samples/kickmic/kick/kickmic_kick_"
    vd(k + "snoff_vl4_rr1.flac", "Kicks/Acoustic Kick hard 1.flac")
    vd(k + "snoff_vl4_rr2.flac", "Kicks/Acoustic Kick hard 2.flac")
    vd(k + "snoff_vl3_rr1.flac", "Kicks/Acoustic Kick medium.flac")
    vd(k + "snoff_vl2_rr1.flac", "Kicks/Acoustic Kick soft.flac")
    vd(k + "snon_vl4_rr1.flac", "Kicks/Acoustic Kick with snares.flac")
    s = "Samples/snaremic/snare/snaremic_snare_"
    vd(s + "center_vl36.flac", "Snares/Snare hard 1.flac")
    vd(s + "center_vl35.flac", "Snares/Snare hard 2.flac")
    vd(s + "center_vl26.flac", "Snares/Snare medium.flac")
    vd(s + "center_vl14.flac", "Snares/Snare soft.flac")
    vd(s + "rimshot_vl12.flac", "Snares/Snare rimshot hard.flac")
    vd(s + "rimshot_vl8.flac", "Snares/Snare rimshot medium.flac")
    vd(s + "crossstick_vl16.flac", "Snares/Snare cross-stick.flac")
    vd(s + "flam_vl12.flac", "Snares/Snare flam.flac")
    vd(s + "buzz_vl12.flac", "Snares/Snare buzz.flac")
    vd("Samples/snaremic/snareoff/snaremic_snareoff_center_vl12.flac", "Snares/Snare wires off.flac")
    h = "Samples/oh/hh/oh_hh_"
    vd(h + "closed_vl4_rr1.flac", "Hats/Hi-hat closed hard 1.flac")
    vd(h + "closed_vl4_rr2.flac", "Hats/Hi-hat closed hard 2.flac")
    vd(h + "closed_vl2_rr1.flac", "Hats/Hi-hat closed soft.flac")
    vd(h + "half_vl4_rr1.flac", "Hats/Hi-hat half open.flac")
    vd(h + "34_vl4_rr1.flac", "Hats/Hi-hat three-quarters open.flac")
    vd(h + "open_vl4_rr1.flac", "Hats/Hi-hat open hard.flac")
    vd(h + "open_vl2_rr1.flac", "Hats/Hi-hat open soft.flac")
    vd(h + "pedal_vl3_rr1.flac", "Hats/Hi-hat pedal.flac")
    vd(h + "splash_rr1.flac", "Hats/Hi-hat foot splash.flac")
    t = "Samples/mid/"
    vd(t + "htom/mid_htom_center_vl16.flac", "Toms/High tom hard.flac")
    vd(t + "htom/mid_htom_center_vl9.flac", "Toms/High tom medium.flac")
    vd(t + "ltom/mid_ltom_center_vl16.flac", "Toms/Low tom hard.flac")
    vd(t + "ltom/mid_ltom_center_vl9.flac", "Toms/Low tom medium.flac")
    vd(t + "ltom/mid_ltom_rimshot_vl10.flac", "Toms/Low tom rimshot.flac")
    vd(t + "ltom/mid_ltom_muted_vl16.flac", "Toms/Low tom muted.flac")
    c = "Samples/oh/"
    vd(c + "crash/oh_crash_crash_vl3_rr1.flac", "Cymbals/Crash hard.flac")
    vd(c + "crash/oh_crash_crash_vl2_rr1.flac", "Cymbals/Crash medium.flac")
    vd(c + "crash/oh_crash_sizzle_vl3_rr1.flac", "Cymbals/Crash sizzle.flac")
    vd(c + "ride/oh_ride_ride_vl3_rr1.flac", "Cymbals/Ride hard.flac")
    vd(c + "ride/oh_ride_ride_vl2_rr1.flac", "Cymbals/Ride medium.flac")
    vd(c + "ride/oh_ride_bell_vl3_rr1.flac", "Cymbals/Ride bell.flac")
    vd(c + "flatride/oh_flatride_ride_vl3_rr1.flac", "Cymbals/Flat ride.flac")
    vd(c + "flatride/oh_flatride_crash_vl4.flac", "Cymbals/Flat ride crashed.flac")
    p = "Samples/perc/mid/"
    vd(p + "agogo/Agogo_High_v3_rr1_Mid.wav", "Percussion/Agogo high.wav")
    vd(p + "agogo/Agogo_Low_v2_rr1_Mid.wav", "Percussion/Agogo low.wav")
    vd(p + "bongoh/BongoH_Hit1_v3_rr1_Mid.wav", "Percussion/Bongo high.wav")
    vd(p + "bongol/BongoL_Hit1_v3_rr1_Mid.wav", "Percussion/Bongo low.wav")
    vd(p + "conga/Conga_22_HitN_101_127_rr1_Mid.wav", "Percussion/Conga.wav")
    vd(p + "conga/Conga_17_HitHM1_71_127_rr1_Mid.wav", "Percussion/Conga muted.wav")
    vd(p + "tumba/Tumba_24_HitN_101_127_rr1_Mid.wav", "Percussion/Tumba.wav")
    vd(p + "claves/Claves1_Hit_v3_rr1_Mid.wav", "Percussion/Claves.wav")
    vd(p + "cowbell/Cowbell1_Normal_v2_rr1_Mid.wav", "Percussion/Cowbell.wav")
    vd(p + "cabasa/Cabasa1_Rub_v2_rr1_Mid.wav", "Percussion/Cabasa.wav")
    vd(p + "triangle/Triangle1_Hit_v2_rr1_Mid.wav", "Percussion/Triangle.wav")
    vd(p + "triangle/Triangle1_HitM_v1_rr1_Mid.wav", "Percussion/Triangle muted.wav")
    vd(p + "tambourine/Tamb1_Shake_rr1_Mid.wav", "Percussion/Tambourine shake.wav")
    vd(p + "vibraslap/Vibraslap1_1_Hit_71_127_rr1_Mid.wav", "Percussion/Vibraslap.wav")
    vd(p + "guiro/Guiro_Fast_rr1_Mid.wav", "Percussion/Guiro.wav")
    vd(p + "sleighbells/Sleighbells_Hit_rr1_Mid.wav", "Percussion/Sleigh bells.wav")
    vd(p + "shaker/LShaker_Shake1D_rr1_Mid.wav", "Percussion/Shaker down.wav")
    vd(p + "shaker/LShaker_Shake1U_rr1_Mid.wav", "Percussion/Shaker up.wav")
    vd("LICENSE", "Licences/Virtuosity Drums - LICENSE.txt")
    b = "Samples/body/"
    for src, to in [("handclap_vl2_rr1.wav", "Clap"), ("handclap_vl2_rr2.wav", "Clap 2"),
                    ("handclap_vl1_rr1.wav", "Clap soft"), ("fingerclap_vl2_rr1.wav", "Finger clap"),
                    ("snap_l_rr1.wav", "Snap"), ("snap_r_rr2.wav", "Snap 2"),
                    ("pinkysnap_l_rr1.wav", "Pinky snap"), ("stomp_l_vl2_rr1.wav", "Stomp"),
                    ("chestthump_l_vl2_rr1.wav", "Chest thump"), ("halfstomp_r_vl2_rr1.wav", "Half stomp")]:
        out.append(entry(BP + b + src, "Claps and Snaps/" + to + ".wav"))
    out.append(entry(BP + "LICENSE", "Licences/Body Percussion - LICENSE.txt"))
    i = "Idiophones/Struck Idiophones/"
    for src, to in [("Claps/Clap_rr1.wav", "Claps and Snaps/Group clap 1.wav"),
                    ("Claps/Clap_rr2.wav", "Claps and Snaps/Group clap 2.wav"),
                    ("Claps/SoloClap_vl4.wav", "Claps and Snaps/Solo clap.wav"),
                    ("Tambourine 1/Tamb1_Hit_v2_rr1_Mid.wav", "Percussion/Tambourine hit.wav"),
                    ("Shaker, Small/Mid_ShakerDouble_Down_rr1.wav", "Percussion/Small shaker down.wav"),
                    ("Shaker, Small/Mid_ShakerDouble_Up_rr1.wav", "Percussion/Small shaker up.wav"),
                    ("Shaker, Small/Mid_Shaker_Slap_rr1.wav", "Percussion/Small shaker slap.wav"),
                    ("Woodblock/wood_click3_vl2.wav", "Percussion/Woodblock.wav"),
                    ("Cajon/Cajon_hit1_fff_rr1.wav", "Percussion/Cajon bass.wav"),
                    ("Cajon/Cajon_hit3_f_rr1.wav", "Percussion/Cajon slap.wav"),
                    ("Cowbells/Cowbell2_Double_v3_rr1_Mid.wav", "Percussion/Cowbell double.wav"),
                    ("Clash Cymbals 1/cymbal_crash1_ff2.wav", "Cymbals/Orchestral clash.wav")]:
        out.append(entry(VC + i + src, to))
    out.append(entry(VC + "LICENSE", "Licences/VCSL - LICENSE.txt"))
    return out


def whole(base, prefix, names, dest, licence_to):
    out = [entry(base + n, dest + n[len(prefix):]) for n in names]
    out.append(entry(base + "LICENSE", licence_to))
    return out


if __name__ == "__main__":
    which = sys.argv[2]
    if which == "starter":
        files = starter()
    else:
        repo, base, prefix, dest = {
            "body": ("body_percussion", BP, "Samples/body/", "Body/"),
            "tx81z": ("VCSL", VC, "Electrophones/TX81Z/", "TX81Z/"),
            "vsco-perc": ("VSCO-2-CE", VS, "Percussion/", "Percussion/"),
        }[which]
        clone = os.path.join(HERE, repo)
        if not os.path.isdir(clone):
            # Names only: a blobless, shallow clone downloads no audio.
            subprocess.run(["git", "clone", "-q", "--filter=blob:none", "--no-checkout", "--depth", "1",
                            REPOS[repo], clone], check=True)
        names = subprocess.run(["git", "-C", clone, "ls-tree", "-r",
                                "--name-only", "HEAD"], capture_output=True, text=True).stdout.split("\n")
        names = [n for n in names if n.startswith(prefix) and n.lower().endswith((".wav", ".flac"))]
        files = whole(base, prefix, names, dest, "Licences/LICENSE.txt")
    json.dump(files, open(os.path.join(HERE, which + ".json"), "w"), indent=1)
    print(which, len(files), sum(f["size"] for f in files))
