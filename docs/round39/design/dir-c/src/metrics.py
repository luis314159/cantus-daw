"""Exact advance widths from the font files (cmap format 4 + hmtx), so every label is laid
out and aligned by measurement, never by guess."""
import struct

FONTDIR = r"F:\projects\daw-r39\fonts\ibm-plex\\"
FILES = {
    ("PlexC", 200): "IBMPlexSansCondensed-ExtraLight.ttf",
    ("PlexC", 300): "IBMPlexSansCondensed-Light.ttf",
    ("PlexC", 400): "IBMPlexSansCondensed-Regular.ttf",
    ("PlexC", 500): "IBMPlexSansCondensed-Medium.ttf",
    ("PlexC", 600): "IBMPlexSansCondensed-SemiBold.ttf",
    ("PlexM", 300): "IBMPlexMono-Light.ttf",
    ("PlexM", 400): "IBMPlexMono-Regular.ttf",
    ("PlexM", 500): "IBMPlexMono-Medium.ttf",
}
_cache = {}


def _load(fam, w):
    key = (fam, w)
    if key in _cache:
        return _cache[key]
    d = open(FONTDIR + FILES[key], "rb").read()
    n = struct.unpack(">H", d[4:6])[0]
    tabs = {}
    for i in range(n):
        tag, _, off, ln = struct.unpack(">4sIII", d[12 + 16 * i:28 + 16 * i])
        tabs[tag.decode()] = off
    upm = struct.unpack(">H", d[tabs["head"] + 18:tabs["head"] + 20])[0]
    nhm = struct.unpack(">H", d[tabs["hhea"] + 34:tabs["hhea"] + 36])[0]
    hm = tabs["hmtx"]
    adv = [struct.unpack(">H", d[hm + 4 * i:hm + 4 * i + 2])[0] for i in range(nhm)]
    c = tabs["cmap"]
    nt = struct.unpack(">H", d[c + 2:c + 4])[0]
    sub = None
    for i in range(nt):
        pid, eid, off = struct.unpack(">HHI", d[c + 4 + 8 * i:c + 12 + 8 * i])
        if pid == 3 and eid == 1:
            sub = c + off
    seg = struct.unpack(">H", d[sub + 6:sub + 8])[0] // 2
    ends = struct.unpack(f">{seg}H", d[sub + 14:sub + 14 + 2 * seg])
    p = sub + 16 + 2 * seg
    starts = struct.unpack(f">{seg}H", d[p:p + 2 * seg])
    deltas = struct.unpack(f">{seg}h", d[p + 2 * seg:p + 4 * seg])
    ro_at = p + 4 * seg
    ros = struct.unpack(f">{seg}H", d[ro_at:ro_at + 2 * seg])
    cmap = {}
    for k in range(seg):
        for ch in range(starts[k], ends[k] + 1):
            if ch == 0xFFFF:
                continue
            if ros[k] == 0:
                gid = (ch + deltas[k]) & 0xFFFF
            else:
                a = ro_at + 2 * k + ros[k] + 2 * (ch - starts[k])
                gid = struct.unpack(">H", d[a:a + 2])[0]
                if gid:
                    gid = (gid + deltas[k]) & 0xFFFF
            cmap[ch] = gid
    f = (upm, adv, cmap)
    _cache[key] = f
    return f


def width(s, size, w=400, fam="PlexC", ls=0.0):
    upm, adv, cmap = _load(fam, w)
    tot = 0
    for ch in str(s):
        g = cmap.get(ord(ch), 0)
        tot += adv[min(g, len(adv) - 1)]
    return tot * size / upm + ls * len(str(s))


def fit(s, size, maxw, w=400, fam="PlexC"):
    """The text, ellipsised only if it must be (round 39: nothing truncated that fits)."""
    if width(s, size, w, fam) <= maxw:
        return s
    while s and width(s + "…", size, w, fam) > maxw:
        s = s[:-1]
    return s + "…"
