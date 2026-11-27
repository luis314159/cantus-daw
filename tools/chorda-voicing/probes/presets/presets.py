"""The character presets as the editor ships them (round 42, chorda-presets), read from
`crates/cantus-ui/src/views/inst/chorda.rs`, and their reference pianos.

    python -I -X utf8 presets.py sources [<name> …]   chorda_compare's source arguments
    python -I -X utf8 presets.py list                  the presets' values
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(HERE))))
EDITOR = os.path.join(REPO, 'crates', 'cantus-ui', 'src', 'views', 'inst', 'chorda.rs')

# The recorded pianos (chorda_compare's `pack:` sources) each preset is fitted against.
REFS = {
    'salamander': 'salamander',
    'bk_stage': 'bk_stage=pack:bitklavier-grand-stage-lid-ortf:grand',
    'bk_bar': 'bk_bar=pack:bitklavier-grand-piano-bar:grand',
    'kw': 'kw=pack:upright-piano-kw:upright',
    'fb': 'fb=pack:old-piano-fb:old-piano',
}
KIND = {
    'v_dark': [],
    'v_imposing': ['salamander', 'bk_stage'],
    'v_bright': ['salamander'],
    'v_intimate': ['bk_bar'],
    'v_upright': ['kw'],
    'v_honky': ['fb'],
    'v_jazz': ['bk_bar'],
    'v_cinematic': [],
}


def presets():
    src = open(EDITOR, encoding='utf-8').read()
    out = {}
    for m in re.finditer(r'preset!\("chorda\.preset\.(v_[a-z]+)",.*?\[(.*?)\]\),', src):
        name, body = m.group(1), m.group(2)
        vals = re.findall(r'"([a-z_.]+)" => (-?[0-9_.]+)', body)
        out[name] = [(k, float(v.replace('_', ''))) for k, v in vals]
    return out


def main(argv):
    p = presets()
    if argv[0] == 'list':
        for k, v in p.items():
            print(k, ','.join(f'{a}={b:g}' for a, b in v))
        return
    want = argv[1:] or list(p) + list(REFS)
    for w in want:
        if w in p:
            # The whole preset, gain included: what the browser applies.
            print(f'{w}=2:' + ','.join(f'{a}={b:g}' for a, b in p[w]))
        elif w in REFS:
            print(REFS[w])
        elif w == 'shipped':
            print('shipped=2:')
        else:
            sys.exit(f'unknown: {w}')


if __name__ == '__main__':
    main(sys.argv[1:])
