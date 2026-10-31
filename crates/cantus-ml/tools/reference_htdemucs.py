# Adapted from demucs/htdemucs.py, demucs/spec.py and demucs/apply.py of
# https://github.com/facebookresearch/demucs @ e976d93e (MIT, Copyright (c) Meta
# Platforms, Inc. and affiliates; the licence: crates/cantus-content/models/LICENSE).
"""The reference: Meta's own code separating a file, as Cantus's port must (ADR 0085).

    python -I reference_htdemucs.py <weights.th> <in.wav|mp3> <out_dir> [--seconds N]

Runs `demucs.apply.apply_model` exactly as `demucs.separate` does — the track normalised
by its mono mix's mean and deviation, split into the model's 7.8 s segments with a
quarter's overlap and triangular cross-fades — but with `shifts=0` (no random time
shift), so the result is deterministic and Cantus's output can be compared to it sample
for sample. Writes `<out_dir>/input.wav` (what was separated, 44.1 kHz float) and one
float WAV per source, and prints the time taken and the peak memory.
"""
import argparse
import os
import sys
import time

import numpy as np
import soundfile as sf
import torch


def peak_mb():
    if sys.platform == "win32":
        import ctypes
        from ctypes import wintypes

        class PMC(ctypes.Structure):
            _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
                        ("PeakWorkingSetSize", ctypes.c_size_t),
                        ("WorkingSetSize", ctypes.c_size_t),
                        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                        ("QuotaPagedPoolUsage", ctypes.c_size_t),
                        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                        ("PagefileUsage", ctypes.c_size_t),
                        ("PeakPagefileUsage", ctypes.c_size_t)]
        c = PMC()
        c.cb = ctypes.sizeof(PMC)
        k32, psapi = ctypes.windll.kernel32, ctypes.windll.psapi
        k32.GetCurrentProcess.restype = wintypes.HANDLE
        psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(PMC),
                                               wintypes.DWORD]
        psapi.GetProcessMemoryInfo(k32.GetCurrentProcess(), ctypes.byref(c), c.cb)
        return c.PeakWorkingSetSize / 1e6
    import resource
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1e3


def main():
    p = argparse.ArgumentParser()
    p.add_argument("weights")
    p.add_argument("input")
    p.add_argument("out_dir")
    p.add_argument("--seconds", type=float, default=0)
    p.add_argument("--threads", type=int, default=0)
    p.add_argument("--shifts", type=int, default=0,
                   help="demucs's shift trick, its offsets fixed at k/n of half a second "
                        "as Cantus fixes them (demucs draws them at random)")
    a = p.parse_args()

    from demucs.apply import apply_model
    from demucs.states import load_model

    if a.threads:
        torch.set_num_threads(a.threads)
    model = load_model(a.weights)
    model.eval()
    audio, sr = sf.read(a.input, dtype="float32", always_2d=True)
    if sr != model.samplerate:
        sys.exit(f"{a.input} is at {sr} Hz; the model wants {model.samplerate}")
    if a.seconds:
        audio = audio[: int(a.seconds * sr)]
    if audio.shape[1] == 1:
        audio = np.repeat(audio, 2, axis=1)
    os.makedirs(a.out_dir, exist_ok=True)
    sf.write(os.path.join(a.out_dir, "input.wav"), audio, sr, subtype="FLOAT")
    wav = torch.tensor(audio.T)
    t0 = time.perf_counter()
    with torch.no_grad():
        ref = wav.mean(0)
        wav = (wav - ref.mean()) / ref.std()
        if a.shifts:
            import demucs.apply as ap
            max_shift = int(0.5 * model.samplerate)
            offsets = iter([k * max_shift // a.shifts for k in range(a.shifts)])
            ap.random.randint = lambda lo, hi: next(offsets)
        sources = apply_model(model, wav[None], shifts=a.shifts, split=True, overlap=0.25,
                              progress=False)[0]
        sources = sources * ref.std() + ref.mean()
    took = time.perf_counter() - t0
    for name, s in zip(model.sources, sources):
        sf.write(os.path.join(a.out_dir, f"{name}.wav"), s.numpy().T, sr, subtype="FLOAT")
    secs = audio.shape[0] / sr
    print(f"PyTorch {torch.__version__}, {torch.get_num_threads()} threads: {secs:.1f} s of "
          f"audio in {took:.1f} s ({secs / took:.2f}x real time), peak {peak_mb():.0f} MB")


if __name__ == "__main__":
    main()
