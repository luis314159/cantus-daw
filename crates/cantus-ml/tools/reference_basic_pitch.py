"""Basic Pitch's own answers, for Cantus's port to be held to (round 34, ADR 0086).

Spotify's `basic_pitch` 0.4.0 (Apache-2.0) with its ONNX model, in a Python environment
under F:\\projects\\daw-r34\\models\\ (never in the repository):

    python -I reference_basic_pitch.py predict <nmp.onnx> <in.wav> <out.json>
        The notes `basic_pitch.inference.predict` makes from a WAV at 22 050 Hz with its
        defaults (start, end, pitch, amplitude, bends), and the posteriorgrams it made them
        from (`<out>.note.f32`, `.onset.f32`, `.contour.f32`, row-major little-endian).

    python -I reference_basic_pitch.py fixture <nmp.onnx> <out_dir>
        The CI fixture: a few seconds of tones made here (chords, a glide with vibrato, a
        bass line, a note at the very end), their posteriorgrams quantised to 16 bits
        (`posteriors.u16.gz`: frames, then note, onset, contour) and, for several settings,
        the notes Basic Pitch's note creation makes from exactly those quantised values
        (`notes.json`) — in Basic Pitch's own order.

    python -I reference_basic_pitch.py notes <posteriors.bin> <frames> <out.json>
        The notes from posteriorgrams Cantus wrote (f32: note, onset, contour).

Deterministic: the same files twice.
"""

import gzip
import json
import struct
import sys

import numpy as np

from basic_pitch import inference, note_creation
from basic_pitch.constants import AUDIO_SAMPLE_RATE, FFT_HOP

SETTINGS = {
    "default": dict(onset_thresh=0.5, frame_thresh=0.3, min_note_len=11),
    "no_melodia": dict(onset_thresh=0.5, frame_thresh=0.3, min_note_len=11, melodia_trick=False),
    "no_infer": dict(onset_thresh=0.5, frame_thresh=0.3, min_note_len=11, infer_onsets=False),
    "sensitive": dict(onset_thresh=0.3, frame_thresh=0.15, min_note_len=5),
    "strict": dict(onset_thresh=0.8, frame_thresh=0.5, min_note_len=20),
    "band": dict(onset_thresh=0.5, frame_thresh=0.3, min_note_len=11, min_freq=100.0, max_freq=600.0),
}


def events(output, **kw):
    out = {k: v.copy() for k, v in output.items()}
    _, ev = note_creation.model_output_to_notes(out, **kw)
    return [
        {
            "start": float(s),
            "end": float(e),
            "pitch": int(p),
            "amplitude": float(a),
            "bends": [int(b) for b in bends] if bends is not None else [],
        }
        for (s, e, p, a, bends) in ev
    ]


def predict(model, wav, out):
    output = inference.run_inference(wav, model)
    notes = events(output, onset_thresh=0.5, frame_thresh=0.3, min_note_len=11)
    with open(out, "w") as f:
        json.dump({"frames": int(output["note"].shape[0]), "notes": notes}, f)
    for k in ("note", "onset", "contour"):
        output[k].astype("<f4").tofile(out + "." + k + ".f32")


def tones():
    sr = AUDIO_SAMPLE_RATE
    n = int(4.2 * sr)
    t = np.arange(n) / sr
    x = np.zeros(n)

    def note(midi, a, b, level, vibrato=0.0, glide_to=None):
        hz0 = 440.0 * 2 ** ((midi - 69) / 12)
        seg = (t >= a) & (t < b)
        tt = t[seg] - a
        if glide_to is None:
            hz = hz0 * 2 ** (vibrato * np.sin(2 * np.pi * 5.5 * tt) / 12)
        else:
            hz1 = 440.0 * 2 ** ((glide_to - 69) / 12)
            hz = hz0 * (hz1 / hz0) ** np.clip(tt / (b - a), 0, 1)
        ph = 2 * np.pi * np.cumsum(hz) / sr
        env = np.minimum(1, tt / 0.01) * np.exp(-tt / 1.5)
        for h, g in ((1, 1.0), (2, 0.5), (3, 0.25), (4, 0.12)):
            x[seg] += level * g * env * np.sin(h * ph)

    for m in (60, 64, 67):
        note(m, 0.1, 1.4, 0.15)
    note(48, 0.1, 2.0, 0.2)
    note(72, 1.5, 2.2, 0.25, vibrato=0.4)
    note(74, 2.2, 2.9, 0.25, glide_to=77)
    note(41, 2.4, 3.6, 0.2)
    for i, m in enumerate((62, 65, 69)):
        note(m, 3.0 + 0.15 * i, 3.9, 0.12)
    note(84, 4.0, 4.2, 0.2)
    return x.astype(np.float32)


def fixture(model, out_dir):
    import os
    import soundfile as sf

    os.makedirs(out_dir, exist_ok=True)
    wav = os.path.join(out_dir, "tones.wav")
    sf.write(wav, tones(), AUDIO_SAMPLE_RATE, subtype="FLOAT")
    output = inference.run_inference(wav, model)
    frames = output["note"].shape[0]
    blob = struct.pack("<I", frames)
    q = {}
    for k in ("note", "onset", "contour"):
        v = np.clip(output[k], 0, 1)
        # The network rests near 0.1; below 0.12 nothing any setting here reads (it keeps the
        # file small): zero.
        v = np.where(v < 0.12, 0, v)
        u = np.round(v * 65535).astype("<u2")
        blob += u.tobytes()
        q[k] = (u.astype(np.float32) / np.float32(65535)).astype(np.float32)
    with gzip.GzipFile(os.path.join(out_dir, "posteriors.u16.gz"), "wb", mtime=0) as f:
        f.write(blob)
    notes = {name: events(q, **kw) for name, kw in SETTINGS.items()}
    with open(os.path.join(out_dir, "notes.json"), "w") as f:
        json.dump({"frames": int(frames), "settings": {k: {kk: v for kk, v in kw.items()} for k, kw in SETTINGS.items()}, "notes": notes}, f, indent=0)
    os.remove(wav)


def notes_of(path, frames, out):
    a = np.fromfile(path, dtype="<f4")
    n88, n264 = frames * 88, frames * 264
    output = {
        "note": a[:n88].reshape(frames, 88),
        "onset": a[n88 : 2 * n88].reshape(frames, 88),
        "contour": a[2 * n88 : 2 * n88 + n264].reshape(frames, 264),
    }
    with open(out, "w") as f:
        json.dump({"notes": events(output, onset_thresh=0.5, frame_thresh=0.3, min_note_len=11)}, f)


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "predict":
        predict(sys.argv[2], sys.argv[3], sys.argv[4])
    elif cmd == "fixture":
        fixture(sys.argv[2], sys.argv[3])
    elif cmd == "notes":
        notes_of(sys.argv[2], int(sys.argv[3]), sys.argv[4])
    else:
        raise SystemExit(__doc__)
