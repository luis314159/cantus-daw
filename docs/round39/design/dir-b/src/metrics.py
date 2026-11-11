"""Text widths from the fonts' own tables (cmap format 4 + hmtx, and HVAR ignored: a weight
factor measured against the bold instances approximates it). Used so nothing collides."""
import os
import struct
from core import FONT_DIR, UI, NARROW, CLOCK, WIDE

_FILES = {UI: "SofiaSansSemiCondensed[wght].ttf", NARROW: "SofiaSansCondensed[wght].ttf",
          CLOCK: "SofiaSansExtraCondensed[wght].ttf", WIDE: "SofiaSans[wght].ttf"}
_cache = {}


def _load(fam):
    if fam in _cache:
        return _cache[fam]
    d = open(os.path.join(FONT_DIR, _FILES[fam]), "rb").read()
    n = struct.unpack(">H", d[4:6])[0]
    tabs = {}
    for i in range(n):
        tag, _, off, ln = struct.unpack(">4sIII", d[12 + 16 * i:28 + 16 * i])
        tabs[tag] = (off, ln)
    upem = struct.unpack(">H", d[tabs[b"head"][0] + 18:tabs[b"head"][0] + 20])[0]
    nh = struct.unpack(">H", d[tabs[b"hhea"][0] + 34:tabs[b"hhea"][0] + 36])[0]
    ho = tabs[b"hmtx"][0]
    adv = [struct.unpack(">H", d[ho + 4 * i:ho + 4 * i + 2])[0] for i in range(nh)]
    # cmap: the Windows Unicode BMP subtable, format 4
    co = tabs[b"cmap"][0]
    nt = struct.unpack(">H", d[co + 2:co + 4])[0]
    cmap = {}
    for i in range(nt):
        pid, eid, so = struct.unpack(">HHI", d[co + 4 + 8 * i:co + 12 + 8 * i])
        st = co + so
        if pid == 3 and eid == 1 and struct.unpack(">H", d[st:st + 2])[0] == 4:
            seg = struct.unpack(">H", d[st + 6:st + 8])[0] // 2
            ends = struct.unpack(">%dH" % seg, d[st + 14:st + 14 + 2 * seg])
            p = st + 16 + 2 * seg
            starts = struct.unpack(">%dH" % seg, d[p:p + 2 * seg])
            deltas = struct.unpack(">%dh" % seg, d[p + 2 * seg:p + 4 * seg])
            ro = p + 4 * seg
            ranges = struct.unpack(">%dH" % seg, d[ro:ro + 2 * seg])
            for k in range(seg):
                for ch in range(starts[k], ends[k] + 1):
                    if ch == 0xFFFF:
                        continue
                    if ranges[k] == 0:
                        g = (ch + deltas[k]) & 0xFFFF
                    else:
                        a = ro + 2 * k + ranges[k] + 2 * (ch - starts[k])
                        g = struct.unpack(">H", d[a:a + 2])[0]
                        if g:
                            g = (g + deltas[k]) & 0xFFFF
                    cmap[ch] = g
            break
    _cache[fam] = (upem, adv, cmap)
    return _cache[fam]


def width(s, size, fam=UI, wt=500, ls=0.0, tnum=False):
    upem, adv, cmap = _load(fam)
    tot = 0
    for ch in s:
        g = cmap.get(ord(ch), 0)
        a = adv[g] if g < len(adv) else adv[-1]
        if tnum and ch.isdigit():
            a = adv[cmap.get(ord("0"), 0)]
        tot += a
    w = tot / upem * size
    w *= 1 + (wt - 400) / 1000 * 0.10
    return w + ls * len(s)
