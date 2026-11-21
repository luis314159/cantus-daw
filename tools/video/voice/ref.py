"""Check the owner's reference recording and cut the clip the voice is cloned from.

    python -I tools/video/voice/ref.py F:/projects/video-work/voice/ref/luis.wav \
        --out F:/projects/video-work/voice/ref/luis-10s.wav

The recording is his own voice only, by his consent: a clean 30-60 s, a quiet room, no
music. This prints what a clone depends on (length, sample rate, peak and clipping, the
noise floor in the pauses, the speech's share), then writes the 10 s with the most speech
and the least noise as a 24 kHz mono WAV, the model's rate. Nothing here judges how the
voice sounds; the owner's ear does.
"""

import argparse

import numpy as np


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("wav")
    ap.add_argument("--out", required=True)
    ap.add_argument("--seconds", type=float, default=10.0)
    a = ap.parse_args()

    import soundfile as sf
    from scipy.signal import resample_poly

    x, sr = sf.read(a.wav, dtype="float32", always_2d=True)
    channels = x.shape[1]
    x = x.mean(axis=1)
    peak = float(np.abs(x).max())
    clipped = int((np.abs(x) >= 0.999).sum())
    # Loudness in 50 ms frames: the quietest tenth is the noise floor, frames 30 dB above
    # it are speech.
    hop = int(sr * 0.05)
    frames = len(x) // hop
    rms = np.sqrt(np.mean(x[: frames * hop].reshape(frames, hop) ** 2, axis=1) + 1e-12)
    db = 20 * np.log10(rms)
    floor = float(np.percentile(db, 10))
    speech = db > floor + 30
    print(f"length      {len(x) / sr:6.1f} s   (want 30-60)")
    print(f"rate        {sr} Hz, {channels} channel(s)")
    print(f"peak        {20 * np.log10(max(peak, 1e-9)):6.1f} dBFS, {clipped} clipped samples (want 0)")
    print(f"noise floor {floor:6.1f} dBFS (want below -60)")
    print(f"speech      {100 * speech.mean():5.1f} % of the frames")

    # The window with the most speech, then the lowest floor.
    win = int(a.seconds / 0.05)
    if frames <= win:
        start = 0
    else:
        best = None
        for s in range(0, frames - win, 4):
            share = speech[s : s + win].mean()
            quiet = np.percentile(db[s : s + win], 10)
            key = (round(share, 2), -quiet)
            if best is None or key > best[0]:
                best = (key, s)
        start = best[1]
    clip = x[start * hop : (start + win) * hop]
    clip = resample_poly(clip, 24000, sr) if sr != 24000 else clip
    clip = clip * (0.7 / max(float(np.abs(clip).max()), 1e-9))
    sf.write(a.out, clip.astype(np.float32), 24000, subtype="FLOAT")
    print(f"clip        {start * 0.05:6.1f}-{start * 0.05 + a.seconds:.1f} s -> {a.out}")


if __name__ == "__main__":
    main()
