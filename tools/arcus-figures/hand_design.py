"""r42:arcus-upper — designing the bow hold away from the app (sheet v7): the right hand of
``woman.fig`` / ``man.fig`` posed on a bow's stick as ``views/inst/arcus_draw_arm.rs::
bow_hand`` and ``scene/bowed_human.rs::arm_aims`` pose it (``hand_rig.py`` mirrors them),
rendered from the hall's side, the player's and above, so the grip can be compared with
photographs at a glance and tuned in seconds (a build of the app takes minutes).

    python tools/arcus-figures/hand_design.py woman|man cello|violin out.png '{"k": 1.27}'

The JSON overrides the design's numbers (cm in the stick's frame: along it toward the
tip, over it, to its far side); ``k`` the hand's size against the woman's.
"""
import os
import sys
import json
import numpy as np
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hand_rig as hr
import preview

who = sys.argv[1] if len(sys.argv) > 1 else 'woman'
held = (sys.argv[2] == 'violin') if len(sys.argv) > 2 else False
out = sys.argv[3] if len(sys.argv) > 3 else 'bowhold.png'
P = json.loads(sys.argv[4]) if len(sys.argv) > 4 else {}
fig = hr.read_fig(os.path.join(os.environ.get('CANTUS_FIGURE_DIR') or os.path.join(
    os.path.dirname(os.path.abspath(__file__)), '..', '..', 'crates', 'cantus-ui', 'assets',
    'figures'), f'{who}.fig'))  # (r42:arcus-hands: CANTUS_FIGURE_DIR as the app reads it)
s = 1
D = np.array([1.0, 0, 0])
U = np.array([0, 1.0, 0])
F = np.array([0, 0, 1.0])
k = P.get('k', 1.0)


def at(u, o, f):
    return (D * u + U * o + F * f) * k


def g(name, d):
    return P.get(name, d)


kc = at(*g('kc', [0.8, 4.0, -1.6]))  # (r42:arcus-hands: grip.rs's)
hd = hr.norm(F * g('hdf', 0.8) - U * g('hdu', 0.45) + D * g('hdd', 0.2))
kx = hr.norm(D - U * g('kxu', 0.3))
wrist = kc - hd * 8.6 * k
along = g('along', [2.0, 0.5, -1.1, -2.6])
fingers = []
for j in range(4):
    knuckle = kc + kx * ((1.5 - j) * 1.9 * k)
    u = along[j]
    if j == 3 and held:
        mid, tip = at(*g('vm3', [-2.2, 2.8, -0.6])), at(*g('vt3', [-2.9, 0.55, 0.0]))
    elif j == 3:
        mid, tip = at(*g('m3', [u, 1.3, 0.55])), at(*g('t3', [u - 0.2, -0.2, 0.95]))
    else:
        hang = g('hang', [0.3, 0.5, 0.5])[j]
        mid, tip = at(u, *g('mid', [1.9, 1.0])), at(u + g('tipu', [1.0, 0.6, 0.2])[j], -hang, g('tipf', 0.2))
    fingers.append([knuckle, mid, mid * 0.45 + tip * 0.55, tip])
palm = kc - hd * 3.6 * k
t3 = at(*g('t3t', [0.3, -0.35, -0.45]))
t2 = at(*g('t2t', [-0.5, -1.0, -1.45]))
t1 = palm * 0.55 + t2 * 0.45 - F * 0.6 - U * 0.4
t0 = wrist * 0.65 + kc * 0.35 - F * 1.0 - U * 0.6
thumb = [t0, t1, t2, t3]
xf = hr.pose_joints(fig, s, wrist, fingers, thumb)
pos, tot = hr.skin_hand(fig, s, xf)
tris = hr.hand_tris(fig, s, 0.5)
# colours
det = fig['det']
skin = np.array([218, 166, 136]) / 255.0
col = np.clip(skin[None] * det[:, :1] * (1 + det[:, 1:2] * np.array([0.22, -0.12, -0.18])), 0, 1)
# stick: a cylinder from frog-4 to 12 along D at origin, radius 0.45 + the frog box
cyl_p, cyl_t = [], []
for i in range(2):
    for a in range(12):
        ang = 2 * np.pi * a / 12
        cyl_p.append(D * (-6 + 20 * i) + (U * np.cos(ang) + F * np.sin(ang)) * 0.45)
for a in range(12):
    b = (a + 1) % 12
    cyl_t += [[a, 12 + a, 12 + b], [a, 12 + b, b]]
cyl_p = np.array(cyl_p)
cyl_t = np.array(cyl_t)
# frog: a box under the stick from u=-4.3..0.4, from the stick down 2 cm
fb = []
for u in (-4.3, 0.4):
    for o in (-0.3, -2.0):
        for f in (-0.6, 0.6):
            fb.append(D * u + U * o + F * f)
fb = np.array(fb)
ft = np.array([[0, 1, 3], [0, 3, 2], [4, 6, 7], [4, 7, 5], [0, 4, 5], [0, 5, 1], [2, 3, 7], [2, 7, 6],
               [0, 2, 6], [0, 6, 4], [1, 5, 7], [1, 7, 3]])
allp = np.vstack([pos, cyl_p, fb])
n0 = len(pos)
allt = np.vstack([tris, cyl_t + n0, ft + n0 + len(cyl_p)])
allc = np.vstack([col, np.tile([[0.35, 0.18, 0.1]], (len(cyl_p), 1)), np.tile([[0.08, 0.06, 0.05]], (len(fb), 1))])
flat = allt.ravel()
nrm = preview.normals(allp, allt)[flat]
pp = allp[flat]
cc = allc[flat]
ti = np.arange(len(flat)).reshape(-1, 3)
c = kc * 0.5
ims = []
views = [((c + np.array([-4, 9, 32])), 'front'), ((c + np.array([-25, 14, 14])), 'player'),
         ((c + np.array([12, 22, 18])), 'above')]
for eye, nm in views:
    o = out.replace('.png', f'-{nm}.png')
    preview.render(pp, ti, cc, cam=(eye, c), size=(420, 420), fov=0.55, nrm=nrm, out=o, key=(-0.4, 0.8, 0.6))
    ims.append(Image.open(o))
im = Image.new('RGB', (1260, 420))
for i, x in enumerate(ims):
    im.paste(x, (420 * i, 0))
im.save(out)
