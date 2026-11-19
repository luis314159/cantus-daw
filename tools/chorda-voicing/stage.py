"""Round 40: stage a voicing — find the output gain that keeps the fortissimo chord's peaks
under the ceiling's knee (rendered without a ceiling), then measure the staged triad, the ff
chord's crest and every phrase's balance against the Salamander.

    python stage.py <spec file> <tag> [knee] [ceiling] [margin dB]
writes <lab>/<tag>-staged.txt (the spec with output, knee and ceiling) and renders
<lab>/lab4/wav/<tag> (CHORDA_LAB, CHORDA_EXE: labpaths.py).
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '.'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import os, subprocess, sys
import numpy as np
import soundfile as sf
from heard import lufs

HERE = LAB  # where the renders and the staged spec go
ALL = 'middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top,triad'


def lab(name, spec, phrases):
    env = dict(os.environ, TMP=TMP, TEMP=TMP, CHORDA_PHRASES=phrases)
    subprocess.run([EXE, 'lab', os.path.join(HERE, 'lab4'), name, spec], env=env, check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return os.path.join(HERE, 'lab4', 'wav', name)


def kv(spec):
    return dict(x.split('=') for x in spec.split(',') if '=' in x)


def main():
    spec_file, tag = sys.argv[1], sys.argv[2]
    knee = float(sys.argv[3]) if len(sys.argv) > 3 else 0.89
    top = float(sys.argv[4]) if len(sys.argv) > 4 else 0.98
    margin = float(sys.argv[5]) if len(sys.argv) > 5 else 0.3
    lines = open(spec_file).read().split('\n')
    spec = [l for l in lines if '=' in l][0].strip()
    v = kv(spec)
    out0 = float(v.get('output', 0.0244))
    # Unbent: a knee far over any sample.
    v.update(knee='100', ceiling='101')
    d = lab(tag + '_free', ','.join(f'{k}={x}' for k, x in v.items()), 'ff_chord')
    x, _ = sf.read(os.path.join(d, 'ff_chord.wav'), always_2d=True)
    pk = 20 * np.log10(np.abs(x).max())
    want = 20 * np.log10(knee) - margin
    out = out0 * 10 ** ((want - pk) / 20)
    v.update(output=f'{out:.6g}', knee=f'{knee}', ceiling=f'{top}')
    staged = ','.join(f'{k}={x}' for k, x in v.items())
    open(os.path.join(HERE, f'{tag}-staged.txt'), 'w').write(staged + '\n')
    d = lab(tag, staged, ALL)
    t, _ = sf.read(os.path.join(d, 'triad.wav'), always_2d=True)
    f, _ = sf.read(os.path.join(d, 'ff_chord.wav'), always_2d=True)
    crest = 20 * np.log10(np.abs(f).max() / np.sqrt((f.mean(1) ** 2).mean()))
    # The lab is the instrument's own output; the engine's centred channel is 3 dB under it.
    print(f'{tag}: ff chord unbent peak {pk:.2f} dBFS at output {out0:.5g} -> output {out:.5g}')
    print(f'  staged triad {lufs(t) - 3.0:.1f} LUFS on a channel (Salamander -27.3, Chorda beta.14 -23.0);'
          f' ff chord crest {crest:.1f} dB (Salamander 25.4), peak {20 * np.log10(np.abs(f).max()):.2f} dBFS')


if __name__ == '__main__':
    main()
