"""Chorda's character presets, the owner's named preview (round 42, chorda-presets): each preset
playing the same phrases, beside the recorded piano of its kind where a free one exists, every
file at one loudness (ITU-R BS.1770 integrated, -20 LUFS, lowered together if a peak would
pass -1 dBFS), 16-bit WAV, and `escuchar.html` in Spanish.

    python -I -X utf8 page.py <wav root> <out dir> <plan.json> [<shots dir>]

<plan.json>: {"title": …, "intro": …, "phrases": [ids], "presets": [{"name": "…", "source":
"<render dir name>", "what": "…", "refs": [{"name": "…", "source": "<dir>[@root]", "credit":
"…"}]}]}. Screenshots (PNG) in <shots dir> are copied beside the page and shown at its end.
"""
import html
import json
import os
import shutil
import sys

import numpy as np
from scipy import signal
import soundfile as sf

NAMES = {'soft_chords': 'Acordes suaves', 'middle_c': 'Do central', 'bass': 'Graves',
         'fast_run': 'Escala rápida', 'ballad': 'Balada'}
TARGET, CEILING = -20.0, -1.0


def lufs(x, sr):
    """ITU-R BS.1770-4 integrated loudness (K-weighting, 400 ms blocks, absolute and relative
    gates) of a stereo signal."""
    # The K-weighting's two stages at 48 kHz (BS.1770's coefficients).
    assert sr == 48_000
    b1, a1 = [1.53512485958697, -2.69169618940638, 1.19839281085285], [1.0, -1.69065929318241, 0.73248077421585]
    b2, a2 = [1.0, -2.0, 1.0], [1.0, -1.99004745483398, 0.99007225036621]
    y = signal.lfilter(b2, a2, signal.lfilter(b1, a1, x, axis=0), axis=0)
    n, hop = int(0.4 * sr), int(0.1 * sr)
    z = np.array([(y[i:i + n] ** 2).mean(axis=0).sum() for i in range(0, len(y) - n + 1, hop)])
    l = -0.691 + 10 * np.log10(z + 1e-30)
    z = z[l > -70]
    rel = -0.691 + 10 * np.log10(z.mean()) - 10
    l = -0.691 + 10 * np.log10(z + 1e-30)
    return float(-0.691 + 10 * np.log10(z[l > rel].mean()))


def load(root, src, ph):
    if '@' in src:
        src, root = src.split('@', 1)
    p = os.path.join(root, src, ph + '.wav')
    if not os.path.exists(p):
        return None
    x, sr = sf.read(p, always_2d=True, dtype='float64')
    return x, sr


def main(argv):
    root, out, plan = argv[0], argv[1], json.load(open(argv[2], encoding='utf-8'))
    shots = argv[3] if len(argv) > 3 else None
    wav = os.path.join(out, 'wav16')
    os.makedirs(wav, exist_ok=True)
    sections = []
    levels = []
    for pr in plan['presets']:
        rows = []
        for ph in plan['phrases']:
            items = [(pr['name'], pr['source'])] + [(r['name'], r['source']) for r in pr.get('refs', [])]
            got = []
            for name, src in items:
                a = load(root, src, ph)
                if a is not None:
                    got.append((name, a))
            if not got:
                continue
            # One gain per file to -20 LUFS, then the phrase's files lowered together if any
            # would pass the ceiling.
            gains = []
            for _, (x, sr) in got:
                lu = lufs(x, sr)
                gains.append(10 ** ((TARGET - lu) / 20))
            peak = max(np.abs(x).max() * g for (_, (x, _)), g in zip(got, gains))
            lower = min(1.0, 10 ** (CEILING / 20) / max(peak, 1e-9))
            players = []
            for (name, (x, sr)), g in zip(got, gains):
                fname = f"{pr['source']} - {ph} - {name}.wav".replace('/', '-')
                sf.write(os.path.join(wav, fname), x * g * lower, sr, subtype='PCM_16')
                levels.append((pr['source'], ph, name, 20 * np.log10(g * lower)))
                players.append(f'<div class="src"><span>{html.escape(name)}</span>'
                               f'<audio controls preload="none" src="wav16/{html.escape(fname)}"></audio></div>')
            rows.append(f'<h3>{NAMES.get(ph, ph)}</h3>' + ''.join(players))
        refs = pr.get('refs', [])
        credit = ''.join(f'<li>{html.escape(r["name"])}: {html.escape(r.get("credit", ""))}</li>' for r in refs)
        if not refs:
            credit = '<li>Sin piano grabado libre de este tipo (ver la nota).</li>'
        sections.append(f'<section><h2>{html.escape(pr["name"])}</h2><p>{html.escape(pr["what"])}</p>'
                        f'<ul class="refs">{credit}</ul>' + ''.join(rows) + '</section>')
    pics = ''
    if shots and os.path.isdir(shots):
        os.makedirs(os.path.join(out, 'capturas'), exist_ok=True)
        for f in sorted(os.listdir(shots)):
            if f.endswith('.png'):
                shutil.copy(os.path.join(shots, f), os.path.join(out, 'capturas', f))
                pics += f'<figure><img src="capturas/{html.escape(f)}" alt="{html.escape(f)}"><figcaption>{html.escape(f)}</figcaption></figure>'
        if pics:
            pics = '<section><h2>La vista «Entonación avanzada»</h2>' + pics + '</section>'
    page = f"""<!doctype html>
<html lang="es"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{html.escape(plan['title'])}</title>
<style>
:root {{ --bg: #f6f5f2; --fg: #1d1d1f; --muted: #5b5b60; --card: #ffffff; --line: #d9d7d0; }}
@media (prefers-color-scheme: dark) {{ :root {{ --bg: #17181b; --fg: #ececef; --muted: #a3a3aa; --card: #22242a; --line: #34363d; }} }}
body {{ background: var(--bg); color: var(--fg); font: 16px/1.5 system-ui, sans-serif; margin: 0 auto; max-width: 900px; padding: 24px 16px; }}
h1 {{ font-size: 1.5rem; margin: 0 0 .5rem; }}
p, .refs {{ color: var(--muted); }}
section {{ background: var(--card); border: 1px solid var(--line); border-radius: 10px; padding: 12px 16px; margin: 14px 0; }}
h2 {{ font-size: 1.2rem; margin: .2rem 0; }} h3 {{ font-size: 1rem; margin: .8rem 0 .2rem; }}
.src {{ display: flex; align-items: center; gap: 12px; margin: 4px 0; flex-wrap: wrap; }}
.src span {{ min-width: 190px; font-weight: 600; }}
audio {{ flex: 1; min-width: 220px; max-width: 100%; }}
img {{ max-width: 100%; border: 1px solid var(--line); border-radius: 6px; }}
</style></head><body>
<h1>{html.escape(plan['title'])}</h1>
<p>{html.escape(plan['intro'])}</p>
{''.join(sections)}
{pics}
</body></html>
"""
    open(os.path.join(out, 'escuchar.html'), 'w', encoding='utf-8').write(page)
    with open(os.path.join(out, 'niveles.csv'), 'w', encoding='utf-8') as f:
        f.write('preset,phrase,source,gain_db\n')
        for r in levels:
            f.write('%s,%s,%s,%.2f\n' % r)
    print('written', os.path.join(out, 'escuchar.html'))


if __name__ == '__main__':
    main(sys.argv[1:])
