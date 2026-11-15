"""The hybrid's pack format, CRS1 (cantus-dsp/src/tympanum_residual.rs says the layout).

Written here independently of the engine's encoder: the engine's test
`a_file_the_fit_tools_writer_made_reads` holds bytes this writer made.

    arts: [ {key, open (0..100 or 255), layers: [ {vel, rho, takes: [int16 array]} ]} ]
"""
import struct
import sys

import numpy as np

MAGIC = b"CRS1"
VERSION = 1


def fnv1a(data: bytes) -> int:
    h = 0x811C9DC5
    for b in data:
        h ^= b
        h = (h * 0x01000193) & 0xFFFFFFFF
    return h


def to_i16(x):
    """A float take as int16 and its scale (peak at full scale)."""
    x = np.asarray(x, dtype=np.float64)
    peak = float(np.max(np.abs(x))) if len(x) else 0.0
    if peak <= 0.0:
        return np.zeros(len(x), dtype=np.int16), 1.0 / 32768.0
    scale = peak / 32767.0
    q = np.clip(np.round(x / scale), -32767, 32767).astype(np.int16)
    return q, scale


def encode(rate: int, model: int, arts) -> bytes:
    head = bytearray()
    data = []
    at = 0
    head += MAGIC + struct.pack("<IIIHH", VERSION, rate, model, len(arts), 0)
    for a in arts:
        head += struct.pack("<BBBB", a["key"], a["open"], len(a["layers"]), 0)
        for l in a["layers"]:
            head += struct.pack("<BBHf", l["vel"], len(l["takes"]), 0, float(l["rho"]))
            for t in l["takes"]:
                if t.dtype == np.int16:
                    q, scale = t, l.get("scale", 1.0 / 32768.0)
                else:
                    q, scale = to_i16(t)
                head += struct.pack("<IIf", at, len(q), scale)
                data.append(q.astype("<i2").tobytes())
                at += len(q)
    head += struct.pack("<I", at)
    return bytes(head) + b"".join(data)


def write(path, rate, model, arts):
    b = encode(rate, model, arts)
    with open(path, "wb") as f:
        f.write(b)
    return len(b)


if __name__ == "__main__":
    # The engine test's bytes: key 36, one layer at 64 (rho 0.5), [1000, -2000, 0].
    b = encode(48000, 0xA1B2C3D4, [{"key": 36, "open": 255, "layers": [
        {"vel": 64, "rho": 0.5, "scale": 1.0 / 32768.0,
         "takes": [np.array([1000, -2000, 0], dtype=np.int16)]}]}])
    sys.stdout.write(", ".join("0x%02x" % x for x in b) + "\n")
