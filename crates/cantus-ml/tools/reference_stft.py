# Adapted from demucs/htdemucs.py, demucs/spec.py and demucs/apply.py of
# https://github.com/facebookresearch/demucs @ e976d93e (MIT, Copyright (c) Meta
# Platforms, Inc. and affiliates; the licence: crates/cantus-content/models/LICENSE).
"""PyTorch's numbers for HT-Demucs's spectrogram and its inverse, for `tests/stft_reference.rs`.

    python -I reference_stft.py <out.json>

Runs `HTDemucs._spec` and `_ispec` (the network's own, no weights needed) on a signal both
sides compute from the same formula, and writes values sampled across the result — the
edges included, where the spectrogram's padding and the two dropped frames make the
inverse differ from the input — so Cantus's port is checked against the original, not
against itself.
"""
import json
import math
import sys

import numpy as np
import torch


def signal(n):
    """Both channels: a sum of sines in float64, then float32 (Rust computes the same)."""
    i = np.arange(n, dtype=np.float64)
    left = 0.5 * np.sin(i * 0.01) + 0.25 * np.sin(i * 0.173 + 1.0) + 0.1 * np.sin(i * 1.9)
    right = 0.4 * np.cos(i * 0.0037) + 0.2 * np.sin(i * 0.61 + 0.5) + 0.05 * np.sin(i * 2.9)
    return np.stack([left, right]).astype(np.float32)


def main():
    from demucs.htdemucs import HTDemucs

    out = sys.argv[1]
    m = HTDemucs(sources=["drums", "bass", "other", "vocals"])
    n = 16_507
    x = torch.tensor(signal(n))[None]
    z = m._spec(x)  # [1, 2, 2048, T] complex
    mag = m._magnitude(z)[0].numpy()  # [4, 2048, T]
    T = mag.shape[-1]
    rng = np.random.default_rng(1)
    spec_points = [(int(c), int(f), int(t)) for c, f, t in zip(
        rng.integers(0, 4, 300), rng.integers(0, 2048, 300), rng.integers(0, T, 300))]
    spec_points += [(c, f, t) for c in range(4) for f in (0, 1, 2047) for t in (0, T - 1)]
    # The inverse of the spectrogram changed bin by bin, so it is no round trip.
    g = torch.tensor(1.0 + 0.5 * np.sin(np.arange(2048) * 0.01), dtype=torch.float32)
    y = m._ispec(z * g[None, None, :, None], n)[0].numpy()  # [2, n]
    idx = sorted(set(list(range(0, 40)) + list(range(n - 40, n)) +
                     [int(v) for v in rng.integers(0, n, 400)]))
    data = {
        "n": n,
        "frames": T,
        "spec": [[c, f, t, float(mag[c, f, t])] for c, f, t in spec_points],
        "ispec_gain": "1 + 0.5 sin(0.01 f)",
        "ispec": [[c, i, float(y[c, i])] for c in range(2) for i in idx],
        "torch": torch.__version__,
    }
    with open(out, "w", newline="\n") as f:
        json.dump(data, f)
        f.write("\n")
    print(f"{len(data['spec'])} spectrogram values, {len(data['ispec'])} samples, T={T}")


if __name__ == "__main__":
    main()
