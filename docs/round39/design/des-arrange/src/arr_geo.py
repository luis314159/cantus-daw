"""des-arrange — the arrangement's measures (logical px; every layout number even, so 125 %
and 150 % land on whole or half pixels) and the view a drawing is made for."""

# the header column
HEAD_W = {'narrow': 136, 'normal': 200, 'wide': 280}
HEAD_MIN, HEAD_MAX = 120, 360
# track heights, by name (the user drags any height between 24 and 240; these are the steps
# the height menu and Alt-wheel go through)
HEIGHTS = {'small': 24, 'compact': 32, 'normal': 44, 'large': 64, 'tall': 96}
LANE_H = 36            # an automation lane's default height
FOLDER_H = 30          # a folder's own row when it is open (its summary)
ADD_H = 28             # the "Add track" row under the last track

# the top lanes, top to bottom
RULER_H = 24           # bars and beats, the loop band, the playhead's cap
SECT_H = 20            # sections and markers
TEMPO_H = 40           # the tempo lane, when shown
SCROLL_H = 14          # the song map that is also the horizontal scrollbar

# the header's columns
STRIPE = 3             # the user's colour, at the left edge
NUM_R = 24             # the number, right-aligned here
NAME_X = 32            # the name and the kind line start here
KEY_W, KEY_H = 18, 16  # mute / solo / arm / automation, joined
METER_W = 3            # the header's own meter, at the right edge
INDENT = 12            # one level of a folder


def width_mode(w):
    return 'narrow' if w < 168 else 'normal' if w < 240 else 'wide'


class View:
    """What one drawing of the arrangement shows: its box, zoom, scroll and state."""

    def __init__(self, x, y, w, h, tracks, head_w=200, bar0=33.0, ppbar=44.0, head=41.0, loop=(41, 49),
                 loop_on=True, tempo=False, sections=True, playing=False, sel_range=None, scroll_y=0,
                 edited=None, sel_clips=(), hover=None, hover_track=None, song_bars=64, add_row=True,
                 overview=True, uid='a'):
        self.x, self.y, self.w, self.h = x, y, w, h
        self.tracks, self.head_w = tracks, head_w
        self.bar0, self.ppbar = bar0, ppbar
        self.head, self.loop, self.loop_on = head, loop, loop_on
        self.tempo, self.sections, self.playing = tempo, sections, playing
        self.sel_range, self.scroll_y = sel_range, scroll_y
        self.edited, self.sel_clips = edited, set(sel_clips)
        self.hover, self.hover_track = hover or {}, hover_track
        self.song_bars, self.add_row, self.overview, self.uid = song_bars, add_row, overview, uid

    # x of a bar (1-based, may be fractional)
    def bx(self, bar):
        return self.x + self.head_w + (bar - self.bar0) * self.ppbar

    @property
    def ppb(self):           # px a beat
        return self.ppbar / 4

    @property
    def cx(self):            # the canvas's left edge
        return self.x + self.head_w

    @property
    def cw(self):
        return self.w - self.head_w

    @property
    def top_h(self):
        return RULER_H + (SECT_H if self.sections else 0) + (TEMPO_H if self.tempo else 0)

    @property
    def lanes_y(self):
        return self.y + self.top_h

    @property
    def lanes_h(self):
        return self.h - self.top_h - (SCROLL_H if self.overview else 0)

    def last_bar(self):
        return self.bar0 + self.cw / self.ppbar

    def rows(self):
        """(kind, track, lane or None, y, h) for every row, scrolled; kind = 'track' | 'lane'."""
        out = []
        y = self.lanes_y - self.scroll_y
        hidden_under = set()
        for tr in self.tracks:
            if tr.get('parent') in hidden_under:
                continue
            if tr['kind'] == 'folder' and not tr.get('open', True):
                hidden_under.add(tr['name'])
            out.append(('track', tr, None, y, tr['h']))
            y += tr['h']
            for ln in tr.get('lanes', []):
                out.append(('lane', tr, ln, y, ln['h']))
                y += ln['h']
        return out, y

    def band(self):
        p = self.ppb
        return 'far' if p < 6 else 'default' if p < 20 else 'near' if p < 60 else 'close'
