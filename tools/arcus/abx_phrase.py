"""Writes tools/arcus/abx_phrase.mid: the blind ABX's notes (round 42), so Arcus and the
recorded strings (`strings-rec`: the Karoryfer Bigcat cello, the Smolken double bass) render
the same thing.

    python -I tools/arcus/abx_phrase.py [out.mid]

A format-1 MIDI file at 480 PPQ and 72 BPM, two tracks: "Cello" (channel 1) and "Double bass"
(channel 2, the same an octave lower). Each track:

1. a solo A3 (bass A2) at pp, mf, ff (velocities 33, 80, 115), 3 s each, 4 s apart;
2. from bar 5, the legato phrase in D minor at velocity 80, each note overlapping the next by
   30 ms: D3 F3 A3 D4 (half notes) | C4 Bb3 A3 (quarter, quarter, half) | G3 A3 Bb3 A3
   (quarters) | D3 (whole) — the four half notes fill two bars of 4/4, so the phrase is five
   bars long — with CC1 at 40 from its start, swelling to 110 across the C4 and G3 bars and
   back to 40 over the last;
3. a marker "pizzicato", then a bar of quarter notes D3 A3 F3 D3 (velocity 80, not
   overlapping): the renderer plays this bar plucked.
"""
import struct
import sys

PPQ = 480
BPM = 72
Q = PPQ


def vlq(n):
    out = [n & 0x7F]
    n >>= 7
    while n:
        out.append(0x80 | (n & 0x7F))
        n >>= 7
    return bytes(reversed(out))


def ticks_of(seconds):
    return round(seconds * BPM / 60.0 * PPQ)


def track(name, ch, shift):
    ev = []  # (tick, order, bytes)
    ev.append((0, 0, b"\xff\x03" + vlq(len(name)) + name.encode()))
    if ch == 0:
        ev.append((0, 0, b"\xff\x51\x03" + struct.pack(">I", round(60e6 / BPM))[1:]))
        ev.append((0, 0, b"\xff\x58\x04\x04\x02\x18\x08"))
    on = lambda t, k, v: ev.append((t, 2, bytes([0x90 | ch, k + shift, v])))
    off = lambda t, k: ev.append((t, 1, bytes([0x80 | ch, k + shift, 0])))
    cc1 = lambda t, v: ev.append((t, 1, bytes([0xB0 | ch, 1, v])))
    # 1. the solo A3 at three dynamics, 3 s each, 4 s apart.
    cc1(0, 64)
    for i, vel in enumerate((33, 80, 115)):
        t = ticks_of(4.0 * i)
        on(t, 57, vel)
        off(t + ticks_of(3.0), 57)
    # 2. the legato phrase from bar 5.
    bar = 4 * Q
    start = 4 * bar
    line = [(50, 2), (53, 2), (57, 2), (62, 2), (60, 1), (58, 1), (57, 2),
            (55, 1), (57, 1), (58, 1), (57, 1), (50, 4)]
    overlap = ticks_of(0.03)
    t = start
    for i, (k, beats) in enumerate(line):
        end = t + beats * Q
        on(t, k, 80)
        off(end + (overlap if i + 1 < len(line) else 0), k)
        t = end
    phrase_end = t
    # CC1: 40 from the start; up to 110 across the C4 bar and the G3 bar (bars 3 and 4 of
    # the phrase), back to 40 over the last.
    cc1(start, 40)
    up_from, up_to = start + 2 * bar, start + 4 * bar
    steps = 32
    for j in range(1, steps + 1):
        cc1(up_from + (up_to - up_from) * j // steps, 40 + (110 - 40) * j // steps)
    for j in range(1, steps + 1):
        cc1(up_to + bar * j // steps, 110 - (110 - 40) * j // steps)
    # 3. the pizzicato bar, a bar after the phrase.
    pz = phrase_end + bar
    ev.append((pz, 0, b"\xff\x06" + vlq(len("pizzicato")) + b"pizzicato"))
    for i, k in enumerate((50, 57, 53, 50)):
        on(pz + i * Q, k, 80)
        off(pz + i * Q + Q - 24, k)
    ev.sort(key=lambda e: (e[0], e[1]))
    data, last = b"", 0
    for tk, _, b in ev:
        data += vlq(tk - last) + b
        last = tk
    data += vlq(bar) + b"\xff\x2f\x00"
    return b"MTrk" + struct.pack(">I", len(data)) + data


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "tools/arcus/abx_phrase.mid"
    head = b"MThd" + struct.pack(">IHHH", 6, 1, 2, PPQ)
    with open(out, "wb") as f:
        f.write(head + track("Cello", 0, 0) + track("Double bass", 1, -12))
    print(out)


if __name__ == "__main__":
    main()
