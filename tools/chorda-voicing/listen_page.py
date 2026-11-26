"""Round 42 (chorda-c17): the owner's named listening page — after `preview.py` has written each
phrase from each source as FLAC (one loudness per phrase), this writes 16-bit WAV copies (for
the chat; each file well under 30 MB) and `escuchar.html`, a plain page in Spanish: per phrase,
what to listen for and one player per source, named (not a blind test).

    python -I listen_page.py <preview dir> <notes.json>

<notes.json>: {"title": "...", "intro": "...", "sources": ["label", ...],
               "phrases": {"<phrase>": "what to listen for", ...},
               "order": ["top", ...]}   (optional: the phrases' order, default ORDER)
"""
import html, json, os, sys
import soundfile as sf

ORDER = ['middle_c', 'fast_run', 'top', 'bass', 'soft_chords', 'ballad', 'ff_chord']
NAMES = {'middle_c': 'Do central', 'fast_run': 'Escala rápida', 'top': 'Octava aguda',
         'bass': 'Graves', 'soft_chords': 'Acordes suaves', 'ballad': 'Balada', 'ff_chord': 'Acorde ff'}


def main():
    d, notes = sys.argv[1], json.load(open(sys.argv[2], encoding='utf-8'))
    wav = os.path.join(d, 'wav16')
    os.makedirs(wav, exist_ok=True)
    rows = []
    for ph in notes.get('order', ORDER):
        players = []
        for src in notes['sources']:
            f = os.path.join(d, f'{ph} - {src}.flac')
            if not os.path.exists(f):
                continue
            x, sr = sf.read(f, dtype='float32', always_2d=True)
            w = os.path.join(wav, f'{ph} - {src}.wav')
            sf.write(w, x, sr, subtype='PCM_16')
            assert os.path.getsize(w) < 30e6, w
            rel = 'wav16/' + os.path.basename(w)
            players.append(f'<div class="src"><span>{html.escape(src)}</span>'
                           f'<audio controls preload="none" src="{html.escape(rel)}"></audio></div>')
        rows.append(f'<section><h2>{NAMES.get(ph, ph)}</h2><p>{html.escape(notes["phrases"].get(ph, ""))}</p>'
                    + ''.join(players) + '</section>')
    page = f"""<!doctype html>
<html lang="es"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{html.escape(notes['title'])}</title>
<style>
:root {{ --bg: #f6f5f2; --fg: #1d1d1f; --muted: #5b5b60; --card: #ffffff; --line: #d9d7d0; }}
@media (prefers-color-scheme: dark) {{ :root {{ --bg: #17181b; --fg: #ececef; --muted: #a3a3aa; --card: #22242a; --line: #34363d; }} }}
body {{ background: var(--bg); color: var(--fg); font: 16px/1.5 system-ui, sans-serif; margin: 0 auto; max-width: 860px; padding: 24px 16px; }}
h1 {{ font-size: 1.5rem; margin: 0 0 .5rem; }}
p {{ color: var(--muted); }}
section {{ background: var(--card); border: 1px solid var(--line); border-radius: 10px; padding: 12px 16px; margin: 14px 0; }}
h2 {{ font-size: 1.1rem; margin: .2rem 0; }}
.src {{ display: flex; align-items: center; gap: 12px; margin: 6px 0; flex-wrap: wrap; }}
.src span {{ min-width: 150px; font-weight: 600; }}
audio {{ flex: 1; min-width: 220px; max-width: 100%; }}
</style></head><body>
<h1>{html.escape(notes['title'])}</h1>
<p>{html.escape(notes['intro'])}</p>
{''.join(rows)}
</body></html>
"""
    open(os.path.join(d, 'escuchar.html'), 'w', encoding='utf-8').write(page)
    print(os.path.join(d, 'escuchar.html'))


if __name__ == '__main__':
    main()
