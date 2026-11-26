"""Round 42 (chorda-c17): the difference analysis before the blind test — a candidate against
the Salamander and the bitKlavier (the two grands' own difference drawn as the natural
spread), on the seven phrases and on single notes. Writes PNGs, `data.json` and `index.html`
(the Spanish summary is `resumen.html` beside it, written by hand from data.json).

    python -I report17.py <out dir> <cand label> [<old label>]

Reads <lab>/eng40/wav/{salamander,bitklavier,<cand>,<old>} and <lab>/notes5/wav/...
"""
import json, os, sys
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from labpaths import LAB  # noqa: E402
import ana17 as A  # noqa: E402
from sig import onsets  # noqa: E402
import between  # noqa: E402

END = {'middle_c': 3.0, 'soft_chords': 9.2, 'fast_run': 3.615, 'ballad': 14.33, 'ff_chord': 5.0,
       'bass': 14.5, 'top': 5.835}
COL = {'sal': '#1f6fb4', 'bk': '#7a7a7a', 'cand': '#d1495b', 'old': '#e8a33d'}
REG = {'graves A0-A2': (21, 45), 'medio C3-C5': (48, 72), 'agudos D#5-C8': (75, 108)}
NOTE_KEYS = [21, 36, 48, 60, 72, 84, 96]
NAMES = {21: 'A0', 36: 'C2', 48: 'C3', 60: 'C4', 72: 'C5', 84: 'C6', 96: 'C7'}


def fig_ltas(src, out, data):
    fig, axs = plt.subplots(4, 2, figsize=(13, 13), sharex=True)
    for ax, ph in zip(axs.flat, A.PHRASES):
        L = {k: A.ltas(A.load_phrase(d, ph)[0]) for k, d in src.items() if not k.startswith('n_')}
        spread = L['bk'] - L['sal']
        ax.fill_between(A.SIXTHS, np.minimum(spread, 0), np.maximum(spread, 0), color='#bbbbbb', alpha=.5,
                        label='bitKlavier − Salamander (lo natural)')
        ax.plot(A.SIXTHS, L['cand'] - L['sal'], color=COL['cand'], lw=1.8, label='c17 − Salamander')
        ax.plot(A.SIXTHS, L['cand'] - L['bk'], color=COL['cand'], lw=1, ls='--', label='c17 − bitKlavier')
        if 'old' in L:
            ax.plot(A.SIXTHS, L['old'] - L['sal'], color=COL['old'], lw=1, label='c16 − Salamander')
        ax.axhline(0, color='k', lw=.5)
        ax.set_xscale('log'); ax.set_ylim(-20, 20); ax.set_title(ph); ax.grid(alpha=.3)
        data.setdefault('ltas', {})[ph] = {k: v.tolist() for k, v in L.items()}
    axs.flat[-1].axis('off')
    axs.flat[0].legend(fontsize=8)
    for ax in axs[-1]:
        ax.set_xlabel('Hz (1/6 de octava, -20 LUFS)')
    fig.tight_layout(); fig.savefig(os.path.join(out, '1_ltas.png'), dpi=90); plt.close(fig)


def fig_maps(src, out, data):
    for ph in A.PHRASES:
        M = {}
        for k in ('sal', 'bk', 'cand'):
            x, on = A.load_phrase(src[k], ph)
            M[k], cnt = A.since_onset_map(x, on, onsets(ph))
        fig, axs = plt.subplots(1, 2, figsize=(13, 4.5))
        for ax, (ref, name) in zip(axs, (('sal', 'Salamander'), ('bk', 'bitKlavier'))):
            D = np.clip(M['cand'] - M[ref], -15, 15)
            D[:, cnt == 0] = np.nan
            ye = [c / 2 ** (1 / 12) for c in A.SIXTHS] + [A.SIXTHS[-1] * 2 ** (1 / 12)]
            im = ax.pcolormesh(np.arange(len(A.TSO)), ye, D, cmap='RdBu_r', vmin=-15, vmax=15, shading='flat')
            ax.set_yscale('log'); ax.set_xticks(np.arange(len(A.TSO))[::2]); ax.set_xticklabels([f'{t:g}' for t in A.TSO[::2]])
            ax.set_xlabel('s desde el último ataque'); ax.set_title(f'{ph}: c17 − {name} (dB)')
        fig.colorbar(im, ax=axs, shrink=.8)
        fig.savefig(os.path.join(out, f'2_map_{ph}.png'), dpi=80); plt.close(fig)
        data.setdefault('maps', {})[ph] = {k: v.tolist() for k, v in M.items()}


def fig_partials(src, out, data):
    rows = []
    for k in NOTE_KEYS:
        f0 = 440 * 2 ** ((k - 69) / 12)
        t = {}
        for s in ('sal', 'bk', 'cand'):
            m, _ = A.note_of(src['n_' + s], 'mf', k)
            t[s] = A.partial_table(m.mean(1), f0)
        rows.append((k, t))
        data.setdefault('partials', {})[NAMES[k]] = {s: {'B': v[1], 'rows': v[0]} for s, v in t.items()}
    fig, axs = plt.subplots(len(NOTE_KEYS), 3, figsize=(14, 3 * len(NOTE_KEYS)))
    for i, (k, t) in enumerate(rows):
        for j, (field, lab) in enumerate((('peak', 'nivel en el ataque (dB re el mayor)'),
                                          ('early', 'caída temprana 0.05–0.5 s (dB/s)'),
                                          ('late', 'caída tardía 1–3 s (dB/s)'))):
            ax = axs[i, j]
            for s, c in (('sal', COL['sal']), ('bk', COL['bk']), ('cand', COL['cand'])):
                r = t[s][0]
                ax.plot([q['n'] for q in r], [q[field] for q in r], 'o-', color=c, ms=3, lw=1,
                        label={'sal': 'Salamander', 'bk': 'bitKlavier', 'cand': 'c17'}[s])
            ax.set_title(f'{NAMES[k]} mf: {lab}', fontsize=9); ax.grid(alpha=.3)
    axs[0, 0].legend(fontsize=8)
    fig.tight_layout(); fig.savefig(os.path.join(out, '3_partials.png'), dpi=75); plt.close(fig)


def fig_noise_attack(src, out, data):
    fig, axs = plt.subplots(2, 3, figsize=(14, 8))
    for j, (rn, (lo, hi)) in enumerate(REG.items()):
        keys = [k for k in A.KEYS if lo <= k <= hi]
        ax = axs[0, j]
        for s, c in (('sal', COL['sal']), ('bk', COL['bk']), ('cand', COL['cand'])):
            R, _ = between.measure(src['n_' + s], 'mf', keys)
            for w, ls in zip(range(3), ('-', '--', ':')):
                ax.plot(between.FC, R[w], ls, color=c, lw=1.2)
            data.setdefault('between', {}).setdefault(rn, {})[s] = R.tolist()
        ax.set_xscale('log'); ax.set_title(f'ruido entre parciales, {rn}, mf\n(— 0.02–0.12 s, -- 0.12–0.32, ·· 0.32–0.82)', fontsize=9)
        ax.grid(alpha=.3); ax.set_ylabel('dB re la nota')
        ax = axs[1, j]
        for s, c in (('sal', COL['sal']), ('bk', COL['bk']), ('cand', COL['cand'])):
            for dyn, ls in (('pp', ':'), ('mf', '-'), ('ff', '--')):
                rise, peak, cs, spec = A.attack(src['n_' + s], dyn, keys)
                ax.plot(cs, spec, ls, color=c, lw=1.2)
                data.setdefault('attack', {}).setdefault(rn, {}).setdefault(s, {})[dyn] = dict(rise_ms=rise, peak_ms=peak, spec=spec.tolist())
        ax.set_xscale('log'); ax.set_title(f'ataque 0–30 ms re 30–300 ms, {rn}\n(·· pp, — mf, -- ff)', fontsize=9); ax.grid(alpha=.3)
    fig.tight_layout(); fig.savefig(os.path.join(out, '4_noise_attack.png'), dpi=80); plt.close(fig)


def fig_stereo_tail(src, out, data):
    fig, axs = plt.subplots(2, 7, figsize=(22, 6))
    for j, ph in enumerate(A.PHRASES):
        for s, c in (('sal', COL['sal']), ('bk', COL['bk']), ('cand', COL['cand'])):
            x, on = A.load_phrase(src[s], ph)
            cs, st = A.stereo_bands(x)
            axs[0, j].plot(cs, st[:, 0], color=c, lw=1.2)
            tail = A.tail_decay(x, on / A.SR + END[ph])
            axs[1, j].plot(np.arange(0, 1.5, 0.05), tail.mean(0), color=c, lw=1.2)
            data.setdefault('stereo', {}).setdefault(ph, {})[s] = st.tolist()
            data.setdefault('tail', {}).setdefault(ph, {})[s] = tail.tolist()
        axs[0, j].set_xscale('log'); axs[0, j].set_ylim(-0.5, 1); axs[0, j].set_title(f'{ph}: correlación L/R', fontsize=9)
        axs[1, j].set_title(f'{ph}: cola tras soltar (dB)', fontsize=9); axs[1, j].set_ylim(-60, 5)
        for a in axs[:, j]:
            a.grid(alpha=.3)
    fig.tight_layout(); fig.savefig(os.path.join(out, '5_stereo_tail.png'), dpi=70); plt.close(fig)


def fig_release(src, out, data):
    fig, axs = plt.subplots(1, 3, figsize=(14, 4))
    for j, (rn, (lo, hi)) in enumerate(REG.items()):
        keys = [k for k in A.KEYS if lo <= k <= hi]
        for s, c in (('sal', COL['sal']), ('bk', COL['bk']), ('cand', COL['cand'])):
            cs, R = A.release(src['n_' + s], 'mf', keys)
            for i, ls in enumerate(('-', '--', ':')):
                axs[j].plot(cs, R[i], ls, color=c, lw=1.2)
            data.setdefault('release', {}).setdefault(rn, {})[s] = R.tolist()
        axs[j].set_xscale('log'); axs[j].grid(alpha=.3)
        axs[j].set_title(f'al soltar, {rn}, mf: dB re antes\n(— 0–50 ms, -- 50–200, ·· 200–800)', fontsize=9)
    fig.tight_layout(); fig.savefig(os.path.join(out, '6_release.png'), dpi=80); plt.close(fig)


def main():
    out, cand = sys.argv[1], sys.argv[2]
    old = sys.argv[3] if len(sys.argv) > 3 else None
    os.makedirs(out, exist_ok=True)
    E, N = os.path.join(LAB, 'eng40', 'wav'), os.path.join(LAB, 'notes5', 'wav')
    src = {'sal': os.path.join(E, 'salamander'), 'bk': os.path.join(E, 'bitklavier'), 'cand': os.path.join(E, cand),
           'n_sal': os.path.join(N, 'salamander'), 'n_bk': os.path.join(N, 'bitklavier'), 'n_cand': os.path.join(N, cand)}
    if old:
        src['old'] = os.path.join(E, old)
    data = {}
    for f in (fig_ltas, fig_maps, fig_partials, fig_noise_attack, fig_stereo_tail, fig_release):
        f({k: v for k, v in src.items() if k != 'old' or f is fig_ltas}, out, data)
        print(f.__name__, 'done', flush=True)
    json.dump(data, open(os.path.join(out, 'data.json'), 'w'))
    imgs = sorted(p for p in os.listdir(out) if p.endswith('.png'))
    html = ['<!doctype html><html lang="es"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">',
            '<title>Chorda c17: análisis de diferencias</title><style>body{font:15px system-ui;max-width:1400px;margin:auto;padding:16px;background:#fafafa;color:#222}img{max-width:100%;border:1px solid #ddd;margin:8px 0;background:#fff}</style>',
            '<h1>Chorda candidato 17 frente al Salamander y al bitKlavier</h1>',
            '<p><a href="resumen.html">Resumen en español: las cinco diferencias mayores</a></p>']
    for p in imgs:
        html.append(f'<h2>{p}</h2><img src="{p}" loading="lazy">')
    open(os.path.join(out, 'index.html'), 'w', encoding='utf-8').write('\n'.join(html))


if __name__ == '__main__':
    main()
