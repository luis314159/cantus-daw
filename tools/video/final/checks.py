"""The finished files checked before the owner sees them (the plan's step 7).

    python -I tools/video/final/checks.py --work F:/projects/video-work [--lang en es] [--ffmpeg <bin dir>]

For each language, and printed as one table, with a FAIL beside anything off its mark:

- **Files:** each deliverable that exists, read by ffprobe (size, length, picture, frame
  rate, sound), against its spec: the master 3840 x 2160 at 60; YouTube the same in H.264
  with AAC; the site's MP4 and WebM 1920 x 1080, each at most 12 MB; the vertical 1080 x 1920,
  30 s; the loop silent.
- **Loudness:** ffmpeg's ebur128 (true peak) on the master and every lossy file: −14 LUFS
  ± 0.5 on the full-length ones, true peak at most −1.0 dBTP everywhere.
- **Sync at three marks:** where the picture shows *With All Your Heart* (s1 at 3 s, s7 at
  55 s, s8 at 64 s), the song's place the picture shows (the scene's `start_seconds`) against
  the place the soundtrack plays there (its edit: bar 1 at 0:00, 91.03 s ahead after the
  crossfade), and measured: the soundtrack's music stem cross-correlated with the Heart
  render around that place. Both within one frame (1/60 s).
- **The carve:** each line's margin over the music (soundtrack json) at least 10 dB, and the
  music's full-band drop at each entry.
- **Captions:** at most 42 characters a line and two lines a caption, in time order, each
  inside the 80 s.
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

FRAME = 1 / 60
MARKS = [("s1-open", 3.0), ("s7-master", 55.0), ("s8-desk", 64.0)]


def probe(ffprobe: str, f: Path) -> dict:
    out = subprocess.run([ffprobe, "-v", "error", "-show_entries",
                          "format=duration,size:stream=codec_type,codec_name,width,height,r_frame_rate",
                          "-of", "json", str(f)], capture_output=True, text=True, check=True).stdout
    return json.loads(out)


def loudness(ffmpeg: str, f: Path) -> tuple:
    err = subprocess.run([ffmpeg, "-hide_banner", "-nostats", "-i", str(f), "-af", "ebur128=peak=true",
                          "-f", "null", "-"], capture_output=True, text=True).stderr
    i = re.findall(r"I:\s+(-?[\d.]+) LUFS", err)
    p = re.findall(r"Peak:\s+(-?[\d.]+) dBFS", err)
    return (float(i[-1]) if i else None, float(p[-1]) if p else None)


def song_heard(t: float, edit: dict) -> float:
    """Where in the song the soundtrack plays at film time t."""
    return t if t < edit["crossfade_centre_s"] else t + edit["song_offset_after_s"]


def xcorr_lag(stem: Path, heart: Path, t: float, song_t: float) -> float:
    """The lag (s) that best aligns 2 s of the music stem at t with the Heart render at song_t."""
    import numpy as np
    import soundfile as sf

    a, sr = sf.read(stem, dtype="float32", start=int(t * 48000), frames=96000, always_2d=True)
    b, _ = sf.read(heart, dtype="float32", start=int((song_t - 0.25) * 48000), frames=96000 + 24000, always_2d=True)
    a, b = a.mean(axis=1), b.mean(axis=1)
    best, lag = -1.0, 0
    for k in range(0, 24000, 8):  # ±0.25 s at 1/6 ms steps
        seg = b[k:k + len(a)]
        c = float(np.dot(a, seg) / (np.linalg.norm(a) * np.linalg.norm(seg) + 1e-12))
        if c > best:
            best, lag = c, k
    return (lag - 12000) / sr, best


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--work", required=True)
    ap.add_argument("--lang", nargs="+", default=["en", "es"])
    ap.add_argument("--ffmpeg", default=None, help="ffmpeg's bin folder")
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    W = Path(a.work)
    bin_dir = Path(a.ffmpeg) if a.ffmpeg else W / "tools/ffmpeg/ffmpeg-n8.1.3-34-ga35157abb9-win64-gpl-8.1/bin"
    ffmpeg, ffprobe = str(bin_dir / "ffmpeg.exe"), str(bin_dir / "ffprobe.exe")
    O, S, F = W / "final", W / "soundtrack3", W / "film4k"
    bad = 0

    def say(ok: bool, text: str) -> None:
        nonlocal bad
        bad += not ok
        print(("  ok    " if ok else "  FAIL  ") + text)

    specs = {
        "master.mkv": (3840, 2160, "60/1", None, True), "2160p.mp4": (3840, 2160, "60/1", None, True),
        "1080p.mp4": (1920, 1080, "30/1", 12, True), "1080p.webm": (1920, 1080, "30/1", 12, True),
        "vertical.mp4": (1080, 1920, "30/1", None, True),
    }
    for lang in a.lang:
        print(f"== {lang}")
        for suffix, (w, h, fps, mb, sound) in specs.items():
            f = O / f"cantus-{lang}-{suffix}"
            if not f.exists():
                print(f"  --    {f.name} not made yet")
                continue
            p = probe(ffprobe, f)
            v = next(s for s in p["streams"] if s["codec_type"] == "video")
            has_a = any(s["codec_type"] == "audio" for s in p["streams"])
            size = int(p["format"]["size"]) / 1e6
            dur = float(p["format"]["duration"])
            want = 30.0 if "vertical" in suffix else 80.0
            say(v["width"] == w and v["height"] == h and v["r_frame_rate"] == fps and has_a == sound
                and abs(dur - want) < 0.1 and (mb is None or size <= mb),
                f"{f.name}: {v['width']}x{v['height']} {v['r_frame_rate']} {v['codec_name']}, {dur:.2f} s, {size:.1f} MB")
            lufs, peak = loudness(ffmpeg, f)
            full = "vertical" not in suffix
            say((not full or abs(lufs - (-14.0)) <= 0.5) and peak is not None and peak <= -1.0,
                f"{f.name}: {lufs} LUFS, true peak {peak} dBTP")
        # sync at three marks
        report = json.loads((S / f"soundtrack-{lang}.json").read_text(encoding="utf-8"))
        edit = report["edit"]
        for scene, t in MARKS:
            sj = W / "film4k" / f"{scene}.json"
            sj = sj if sj.exists() else W / "film" / f"{scene}.json"
            sc = json.loads(sj.read_text(encoding="utf-8"))
            shown = sc["start_seconds"] + (t - sc["cut"][0])
            heard = song_heard(t, edit)
            lag, corr = xcorr_lag(S / f"soundtrack-{lang}-music.wav", S / "heart.wav", t, heard)
            say(abs(shown - heard) <= FRAME and abs(lag) <= FRAME,
                f"sync {scene} at {t:.1f} s: picture at song {shown:.3f} s, sound at {heard:.3f} s, measured lag {lag * 1000:+.1f} ms (r {corr:.2f})")
        # the carve
        for line in report["lines"]:
            m = round(line["margin_db"], 1)
            say(m is not None and m >= 10.0, f"line {line.get('line')}: voice {m} dB over the music; music drop at entry {round(line.get('full_band_drop_at_entry_db', float('nan')), 1)} dB")
        # captions
        caps = json.loads((O / f"captions-{lang}.json").read_text(encoding="utf-8"))
        longest = max(len(l) for c in caps for l in c["lines"])
        ordered = all(caps[i]["to"] <= caps[i + 1]["from"] + 1e-6 for i in range(len(caps) - 1))
        say(longest <= 42 and all(len(c["lines"]) <= 2 for c in caps) and ordered and caps[-1]["to"] <= 80.0,
            f"captions: {len(caps)}, longest line {longest} characters, in order, inside 80 s")
    if (O / "cantus-loop.mp4").exists():
        p = probe(ffprobe, O / "cantus-loop.mp4")
        say(not any(s["codec_type"] == "audio" for s in p["streams"]) and abs(float(p["format"]["duration"]) - 18.0) < 0.2,
            f"cantus-loop.mp4: {float(p['format']['duration']):.2f} s, silent")
    print("all checks passed" if bad == 0 else f"{bad} check(s) failed")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
