# Adapted from demucs/htdemucs.py, demucs/spec.py and demucs/apply.py of
# https://github.com/facebookresearch/demucs @ e976d93e (MIT, Copyright (c) Meta
# Platforms, Inc. and affiliates; the licence: crates/cantus-content/models/LICENSE).
"""Export HT-Demucs (Meta's MIT weights) to the ONNX file Cantus runs (ADR 0085).

The network only: the spectrogram (STFT), its inverse and the sum of the two branches are
computed by Cantus (`crates/cantus-ml/src/demucs.rs`), as the original code computes them
around the network. What crosses the graph's edge:

    inputs   mix  [1, 2, L]           the segment, L = 343 980 samples (7.8 s at 44.1 kHz)
             mag  [1, 4, 2048, 336]   its spectrogram, real and imaginary as channels
    outputs  spec [1, S, 4, 2048, 336] each source's spectrogram (complex as channels)
             wave [1, S, 2, L]         each source's time-branch waveform

Usage (a Python environment with torch, demucs 4, onnx, onnxscript, onnxruntime):

    python -I export_htdemucs.py <weights.th> <out_dir> <name>

writes `<name>.onnx` (the graph, no weights: every weight of the state is *external
data* in `<name>.weights`, in half precision as the `.th` stores it, cast to float32
inside the graph), `<name>.weights` and `<name>.map.json` — where each weight of the
`.th` goes in `<name>.weights`, and the hashes. Cantus ships the graph and the map and
rebuilds `<name>.weights` from Meta's own `.th` at install, so it never hosts the
weights; the hash of the rebuilt file is checked against the map's.
"""
import argparse
import math
import sys

import numpy as np
import torch
from torch import nn


def core_of(model):
    """The part of `HTDemucs.forward` between the spectrogram and the masking."""

    class Core(nn.Module):
        def __init__(self, m):
            super().__init__()
            self.m = m

        def forward(self, mix, mag):
            m = self.m
            x = mag
            B, C, Fq, T = x.shape
            mean = x.mean(dim=(1, 2, 3), keepdim=True)
            std = x.std(dim=(1, 2, 3), keepdim=True)
            x = (x - mean) / (1e-5 + std)
            xt = mix
            meant = xt.mean(dim=(1, 2), keepdim=True)
            stdt = xt.std(dim=(1, 2), keepdim=True)
            xt = (xt - meant) / (1e-5 + stdt)
            saved, saved_t, lengths, lengths_t = [], [], [], []
            for idx, encode in enumerate(m.encoder):
                lengths.append(x.shape[-1])
                inject = None
                if idx < len(m.tencoder):
                    lengths_t.append(xt.shape[-1])
                    tenc = m.tencoder[idx]
                    xt = tenc(xt)
                    if not tenc.empty:
                        saved_t.append(xt)
                    else:
                        inject = xt
                x = encode(x, inject)
                if idx == 0 and m.freq_emb is not None:
                    frs = torch.arange(x.shape[-2], device=x.device)
                    emb = m.freq_emb(frs).t()[None, :, :, None].expand_as(x)
                    x = x + m.freq_emb_scale * emb
                saved.append(x)
            if m.crosstransformer:
                if m.bottom_channels:
                    b, c, f, t = x.shape
                    x = x.reshape(b, c, f * t)
                    x = m.channel_upsampler(x)
                    x = x.reshape(b, x.shape[1], f, t)
                    xt = m.channel_upsampler_t(xt)
                x, xt = m.crosstransformer(x, xt)
                if m.bottom_channels:
                    b, c, f, t = x.shape
                    x = x.reshape(b, c, f * t)
                    x = m.channel_downsampler(x)
                    x = x.reshape(b, x.shape[1], f, t)
                    xt = m.channel_downsampler_t(xt)
            for idx, decode in enumerate(m.decoder):
                skip = saved.pop(-1)
                x, pre = decode(x, skip, lengths.pop(-1))
                offset = m.depth - len(m.tdecoder)
                if idx >= offset:
                    tdec = m.tdecoder[idx - offset]
                    length_t = lengths_t.pop(-1)
                    if tdec.empty:
                        pre = pre[:, :, 0]
                        xt, _ = tdec(pre, None, length_t)
                    else:
                        skip = saved_t.pop(-1)
                        xt, _ = tdec(xt, skip, length_t)
            S = len(m.sources)
            x = x.view(B, S, -1, Fq, T)
            x = x * std[:, None] + mean[:, None]
            xt = xt.view(B, S, -1, mix.shape[-1])
            xt = xt * stdt[:, None] + meant[:, None]
            return x, xt

    return Core(model)


def host_forward(model, core_run, mix):
    """The whole model with the network replaced by `core_run`: what Cantus does."""
    z = model._spec(mix)
    mag = model._magnitude(z)
    spec, wave = core_run(mix, mag)
    zout = model._mask(z, torch.as_tensor(spec))
    x = model._ispec(zout, mix.shape[-1])
    return torch.as_tensor(wave) + x


def test_signal(length, seed=7):
    """A deterministic, music-like stereo signal (tones, a kick, noise)."""
    rng = np.random.default_rng(seed)
    t = np.arange(length) / 44100.0
    tone = 0.2 * np.sin(2 * math.pi * 220 * t) + 0.1 * np.sin(2 * math.pi * 330 * t)
    beat = t % 0.5
    kick = 0.5 * np.sin(2 * math.pi * 55 * beat) * np.exp(-beat * 30)
    noise = rng.standard_normal((2, length)) * 0.02
    x = np.stack([tone + kick, 0.8 * tone + kick]) + noise
    return torch.tensor(x[None], dtype=torch.float32)


ALIGN = 64


def externalise(path_in, out_dir, name, state, source_sha256):
    """Move every weight of the original state out of the graph into `<name>.weights`.

    That file is the state's tensors in the state's order, each in half precision as the
    `.th` stores it, each at a multiple of 64 bytes: nothing in it depends on the graph,
    so Cantus rebuilds it from the `.th` alone (`cantus_content::models`). In the graph,
    each weight the exporter kept — as it is, or transposed (a `MatMul`'s) — becomes a
    half-precision external tensor, a `Cast` to float32 and, if need be, a `Transpose`,
    which ONNX Runtime folds at load. What is left inline is the exporter's own small
    constants (shapes, positions computed with sines), no weight. Every match is by value,
    exactly, so a weight the graph holds is the state's to the bit or the export fails."""
    import hashlib
    import json
    import os

    import onnx
    from onnx import TensorProto, helper, numpy_helper

    blob = bytearray()
    tensors = []
    offsets = {}
    by_value = {}
    for key, v in state.items():
        if v.dtype != torch.float16 or not v.is_contiguous():
            sys.exit(f"{key}: {v.dtype}, contiguous {v.is_contiguous()}: not handled")
        while len(blob) % ALIGN:
            blob.append(0)
        raw = v.numpy().astype("<f2").tobytes()
        offsets[key] = (len(blob), len(raw))
        tensors.append({"key": key, "offset": len(blob), "dtype": "f16",
                        "shape": list(v.shape)})
        blob += raw
        by_value.setdefault(v.float().numpy().tobytes(), key)

    m = onnx.load(path_in)
    g = m.graph
    nodes = []
    used = set()
    inline = 0
    for t in list(g.initializer):
        a = numpy_helper.to_array(t)
        key, transposed = None, False
        if a.dtype == np.float32:
            key = by_value.get(np.ascontiguousarray(a).tobytes())
            if key is None and a.ndim == 2:
                key = by_value.get(np.ascontiguousarray(a.T).tobytes())
                transposed = key is not None
        if key is None:
            inline += a.size
            continue
        used.add(key)
        offset, length = offsets[key]
        ext = TensorProto()
        ext.name = t.name + "__f16"
        ext.data_type = TensorProto.FLOAT16
        ext.dims.extend(a.T.shape if transposed else a.shape)
        ext.data_location = TensorProto.EXTERNAL
        for k, val in (("location", name + ".weights"), ("offset", str(offset)),
                       ("length", str(length))):
            e = ext.external_data.add()
            e.key, e.value = k, val
        g.initializer.remove(t)
        g.initializer.append(ext)
        cast_out = t.name + "__f32" if transposed else t.name
        nodes.append(helper.make_node("Cast", [ext.name], [cast_out], to=TensorProto.FLOAT,
                                      name=t.name + "__cast"))
        if transposed:
            nodes.append(helper.make_node("Transpose", [cast_out], [t.name], perm=[1, 0],
                                          name=t.name + "__transpose"))
    for n in reversed(nodes):
        g.node.insert(0, n)
    # The exporter's notes on every node (source lines, stack traces): two thirds of the
    # file, no use at run time.
    for n in g.node:
        del n.metadata_props[:]
        n.doc_string = ""
    g.doc_string = ""
    del m.metadata_props[:]
    graph_path = os.path.join(out_dir, name + ".onnx")
    weights_path = os.path.join(out_dir, name + ".weights")
    with open(weights_path, "wb") as f:
        f.write(blob)
    onnx.save(m, graph_path)
    onnx.checker.check_model(graph_path)
    sha = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest()
    mp = {
        "format": 1,
        "source_sha256": source_sha256,
        "graph_sha256": sha(graph_path),
        "weights_size": len(blob),
        "weights_sha256": sha(weights_path),
        "tensors": tensors,
    }
    with open(os.path.join(out_dir, name + ".map.json"), "w", newline="\n") as f:
        json.dump(mp, f, indent=1)
        f.write("\n")
    print(f"inline constants: {inline} values", file=sys.stderr)
    return graph_path, len(used), len(state)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("weights", help="Meta's .th file")
    p.add_argument("out_dir")
    p.add_argument("name", help="the files' stem: htdemucs, htdemucs_6s")
    a = p.parse_args()

    import hashlib
    import os

    import onnx
    import onnxruntime as ort
    from demucs.states import load_model

    torch.backends.mha.set_fastpath_enabled(False)
    package = torch.load(a.weights, map_location="cpu", weights_only=False)
    model = load_model(a.weights)
    model.eval()
    model.segment = float(model.segment)
    L = int(model.segment * model.samplerate)
    core = core_of(model).eval()
    mix = test_signal(L)
    os.makedirs(a.out_dir, exist_ok=True)
    source_sha = hashlib.sha256(open(a.weights, "rb").read()).hexdigest()
    with torch.no_grad():
        mag = model._magnitude(model._spec(mix))
        print("sources", model.sources, "segment", L, "mag", tuple(mag.shape), file=sys.stderr)
        tmp = os.path.join(a.out_dir, a.name + ".inline.onnx")
        torch.onnx.export(core, (mix, mag), tmp, input_names=["mix", "mag"],
                          output_names=["spec", "wave"], opset_version=18, dynamo=True,
                          external_data=False)
        graph, moved, total = externalise(tmp, a.out_dir, a.name, package["state"], source_sha)
        os.remove(tmp)
        g = onnx.load(graph, load_external_data=False).graph
        print(f"nodes {len(g.node)}, weights moved {moved} of {total}, graph "
              f"{os.path.getsize(graph)} bytes", file=sys.stderr)
        if moved != total:
            sys.exit("some weights of the state are not in the graph by name")

        # The check: the whole model as Cantus runs it (network in ONNX Runtime, the rest
        # around it) against the original forward, on the same segment.
        ref = model(mix).numpy()
        sess = ort.InferenceSession(graph, providers=["CPUExecutionProvider"])

        def run(mx, mg):
            s, w = sess.run(["spec", "wave"], {"mix": mx.numpy(), "mag": mg.numpy()})
            return s, w

        got = host_forward(model, run, mix).numpy()
        err = np.abs(got - ref).max()
        snr = 10 * np.log10((ref ** 2).sum() / ((got - ref) ** 2).sum())
        print(f"against PyTorch: max abs diff {err:.3e}, SNR {snr:.1f} dB", file=sys.stderr)
        if err > 1e-3:
            sys.exit("the export does not match the original model")


if __name__ == "__main__":
    main()
