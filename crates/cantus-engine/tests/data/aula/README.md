# Aula's outside test files (ADR 0112)

`python-wave-true-stereo-24.wav` was written by CPython 3.13's standard `wave` module (not
by Cantus, not by `hound`): 4 channels, 24-bit PCM, 44 100 Hz, 2 000 frames. Channel `c`
(0 left→left, 1 left→right, 2 right→left, 3 right→right) holds `(c + 1) · 1 000 000` at
frame `10 + 100 · c` and `−(c + 1) · 12 345` at frame 1 500, zero elsewhere. The script:

```python
import wave, struct
data = bytearray()
for i in range(2000):
    for c in range(4):
        v = 0
        if i == 10 + 100 * c: v = (c + 1) * 1_000_000
        if i == 1500: v = -(c + 1) * 12345
        data += struct.pack('<i', v)[:3]
w = wave.open('python-wave-true-stereo-24.wav', 'wb')
w.setnchannels(4); w.setsampwidth(3); w.setframerate(44100)
w.writeframes(bytes(data)); w.close()
```

The mono 16-bit, stereo 24-bit and 32-bit float files Aula is also read against are
libsndfile's, already in `crates/cantus-dsp/tests/data/sectio/`.
