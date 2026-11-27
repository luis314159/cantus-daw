"""r42 arcus-classical pass 2 — the Paganini comparison's note shapes by length (IOI bins) and bow (N a new bow, S a slur): start/end under the peak (dB) and rise (ms), each take; then the summary. python -I paganini_shapes.py <W dir>"""
import json, sys, numpy as np
d = json.load(open(sys.argv[1] + '/analisis/analisis.json'))
n = d['notes']
bins = [(0, 0.15), (0.15, 0.3), (0.3, 0.6), (0.6, 9)]
print(f"{'':38s}" + "".join(f"{f'{a}-{b} {nb}':>22s}" for a, b in bins for nb in ('N', 'S')))
for k, v in n.items():
    row = []
    for a, b in bins:
        for nb in (True, False):
            r = [x for x in v if a <= x['ioi'] < b and x['new_bow'] == nb and x['start_under'] is not None]
            f = lambda key: np.nanmedian([x[key] for x in r if x[key] is not None]) if r else float('nan')
            row.append(f"{f('start_under'):5.1f}/{f('end_under'):5.1f}/{f('rise_ms'):4.0f}")
    print(f"{k[:38]:38s}" + "".join(f"{c:>22s}" for c in row))
s = d['summary']
for k, v in s.items():
    if k == 'cross':
        continue
    g = lambda key: v.get(key, [float('nan')])[0] if isinstance(v.get(key), list) else v.get(key)
    print(f"{k[:38]:38s} start {g('start_under'):.1f} end {g('end_under'):.1f} endlong {g('end_under_long'):.1f} rise {g('rise_ms'):.0f} dipR {g('dip_rebow'):.1f} dipS {g('dip_slur'):.1f} dyn {v.get('dyn_range_db', 0):.1f}")
if 'cross' in d['crossings'] if isinstance(d.get('crossings'), dict) else False:
    pass
for k, v in d.get('crossings', {}).items():
    if isinstance(v, dict) and 'ring_db' in v:
        print(f"cross {k[:34]:34s} ring {v['ring_db']:.1f} bal {v['balance_db']:.1f} int {v['interval_ct']:.1f}")
