# Sectio marker files written by libsndfile (round 37, ADR 0099)

Written by **libsndfile 1.2.2** (LGPL-2.1; the `libsndfile_x64.dll` of the `soundfile` 0.14.0
wheel from PyPI), driven through Python ctypes by the script below on 2026-11-05: cue points
with `SFC_SET_CUE`, sampler loops with `SFC_SET_INSTRUMENT`. libsndfile read each back
(`SFC_GET_CUE_COUNT`: 4, 0, 4; `SFC_GET_INSTRUMENT`: 0, 2, 1 loops). The audio is a decaying
330 Hz tone at each hit. The bytes are libsndfile's own: `tests/sectio_markers.rs` reads
them with Sectio's parser.

| File | SHA-256 |
|---|---|
| `libsndfile-cues-mono16.wav` | `a9ba7886890dc999a579201a582c718db3642d717ff5697fb54cdbebd836a4c6` |
| `libsndfile-cues-smpl-float.wav` | `34648b7827345f5d81f9e2b9d66734016c9b25e2f93254e853d3b60f09f885db` |
| `libsndfile-smpl-stereo24.wav` | `2b3a0f513969c7e59e43b4f99daccd722437715e506500a86f1d372532aea930` |

```python
"""Write WAV files with cue points and smpl loops through libsndfile (an outside writer),
for Sectio's marker tests. Usage: python -I sf_cues.py <libsndfile.dll> <out dir>"""
import ctypes as C
import math
import sys

dll = C.CDLL(sys.argv[1])
out = sys.argv[2]


class Info(C.Structure):
    _fields_ = [("frames", C.c_int64), ("samplerate", C.c_int), ("channels", C.c_int),
                ("format", C.c_int), ("sections", C.c_int), ("seekable", C.c_int)]


class Cue(C.Structure):
    _fields_ = [("indx", C.c_int32), ("position", C.c_uint32), ("fcc_chunk", C.c_int32),
                ("chunk_start", C.c_int32), ("block_start", C.c_int32),
                ("sample_offset", C.c_uint32), ("name", C.c_char * 256)]


def cues_struct(n):
    class Cues(C.Structure):
        _fields_ = [("cue_count", C.c_uint32), ("cue_points", Cue * n)]
    return Cues


class Loop(C.Structure):
    _fields_ = [("mode", C.c_int), ("start", C.c_uint32), ("end", C.c_uint32),
                ("count", C.c_uint32)]


class Instrument(C.Structure):
    _fields_ = [("gain", C.c_int), ("basenote", C.c_char), ("detune", C.c_char),
                ("velocity_lo", C.c_char), ("velocity_hi", C.c_char), ("key_lo", C.c_char),
                ("key_hi", C.c_char), ("loop_count", C.c_int), ("loops", Loop * 16)]


SFM_WRITE, SFM_READ = 0x20, 0x10
SFC_GET_CUE_COUNT, SFC_GET_CUE, SFC_SET_CUE = 0x10CD, 0x10CE, 0x10CF
SFC_GET_INSTRUMENT, SFC_SET_INSTRUMENT = 0x10D0, 0x10D1
SF_LOOP_FORWARD = 801
WAV, PCM16, PCM24, FLOAT = 0x010000, 0x0002, 0x0003, 0x0006
dll.sf_open.restype = C.c_void_p
dll.sf_open.argtypes = [C.c_char_p, C.c_int, C.POINTER(Info)]
dll.sf_command.argtypes = [C.c_void_p, C.c_int, C.c_void_p, C.c_int]
dll.sf_writef_float.argtypes = [C.c_void_p, C.POINTER(C.c_float), C.c_int64]
dll.sf_writef_float.restype = C.c_int64
dll.sf_close.argtypes = [C.c_void_p]
dll.sf_version_string.restype = C.c_char_p


def write(name, rate, channels, fmt, frames, hits, cues=(), loops=()):
    info = Info(0, rate, channels, WAV | fmt, 0, 0)
    sf = dll.sf_open((out + "/" + name).encode(), SFM_WRITE, C.byref(info))
    assert sf, dll.sf_strerror(None)
    if cues:
        # libsndfile takes exactly an SF_CUES: room for 100 points (sndfile.h's SF_CUES_VAR(100)).
        cs = cues_struct(100)()
        cs.cue_count = len(cues)
        for i, (indx, at) in enumerate(cues):
            p = cs.cue_points[i]
            p.indx, p.position, p.fcc_chunk = indx, at, 0x61746164  # 'data'
            p.chunk_start, p.block_start, p.sample_offset = 0, 0, at
            p.name = b"Cue %d" % indx
        ok = dll.sf_command(sf, SFC_SET_CUE, C.byref(cs), C.sizeof(cs))
        assert ok == 1, f"SFC_SET_CUE refused ({ok})"
    if loops:
        ins = Instrument()
        ins.gain, ins.basenote, ins.velocity_hi, ins.key_hi = 1, b"\x3c", b"\x7f", b"\x7f"
        ins.loop_count = len(loops)
        for i, (a, b) in enumerate(loops):
            ins.loops[i] = Loop(SF_LOOP_FORWARD, a, b, 0)
        ok = dll.sf_command(sf, SFC_SET_INSTRUMENT, C.byref(ins), C.sizeof(ins))
        assert ok == 1, f"SFC_SET_INSTRUMENT refused ({ok})"
    buf = (C.c_float * (frames * channels))()
    for h in hits:
        for k in range(min(frames - h, rate // 8)):
            v = 0.6 * math.exp(-k / (rate * 0.02)) * math.sin(2 * math.pi * 330 * k / rate)
            for c in range(channels):
                buf[(h + k) * channels + c] += v
    assert dll.sf_writef_float(sf, buf, frames) == frames
    dll.sf_close(sf)
    # Read the markers back through libsndfile itself.
    info = Info()
    sf = dll.sf_open((out + "/" + name).encode(), SFM_READ, C.byref(info))
    n = C.c_uint32(0)
    dll.sf_command(sf, SFC_GET_CUE_COUNT, C.byref(n), C.sizeof(n))
    ins = Instrument()
    got = dll.sf_command(sf, SFC_GET_INSTRUMENT, C.byref(ins), C.sizeof(ins))
    print(name, "frames", info.frames, "cues", n.value, "instrument", got, ins.loop_count)
    dll.sf_close(sf)


print(dll.sf_version_string().decode())
# Mono, 16-bit, 22.05 kHz: four cues written in the order 3, 1, 4, 2.
write("libsndfile-cues-mono16.wav", 22050, 1, PCM16, 11025, [0, 2756, 5512, 8268],
      cues=[(3, 5512), (1, 0), (4, 8268), (2, 2756)])
# Stereo, 24-bit, 44.1 kHz: no cues, two forward loops in a smpl chunk.
write("libsndfile-smpl-stereo24.wav", 44100, 2, PCM24, 4410, [0, 2205],
      loops=[(2205, 4409), (0, 2204)])
# Stereo float, 48 kHz: cues and a loop â€” the cues win.
write("libsndfile-cues-smpl-float.wav", 48000, 2, FLOAT, 4800, [0, 1200, 2400, 3600],
      cues=[(1, 0), (2, 1200), (3, 2400), (4, 3600)], loops=[(0, 4799)])
```
