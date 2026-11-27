"""r42:arcus-hands — ``shoot_arcus --cam=`` arguments for a zoomed look at the left hand from
the app's own camera: the dump's eye (``arcus_hand_table -- --ignored dump``) aimed at the
stop of ``string`` at ``semis``, a little toward the hand, ``half`` cm across. The app's
perspective is kept (the same eye), only narrowed: about 400 % of the 100 % picture at
``half`` 8 with ``--scales=2``.

    python tools/arcus-figures/hand_cam.py <dump.json> <who> <inst> <string> <semis> <half>
"""
import json
import sys

import numpy as np


def main():
    D = json.load(open(sys.argv[1]))
    who, inst, string, semis, half = (sys.argv[2], sys.argv[3], int(sys.argv[4]),
                                      float(sys.argv[5]), float(sys.argv[6]))
    F = next(F for F in D["figs"] if F["who"] == who and F["inst"] == inst)
    s = F["strings"][string]
    b, n = np.array(s["bridge"]), np.array(s["nut"])
    stop = b + (n - b) * 2 ** (-semis / 12)
    ex, ey, ez = np.array(F["ex"]), np.array(F["ey"]), np.array(F["ez"])
    at = stop + ex * 2.5 - ez * 1.0 + ey * 1.5
    print(",".join("%.3f" % v for v in list(F["eye"]) + list(at) + [half]))


if __name__ == "__main__":
    main()
