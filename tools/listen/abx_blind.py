#!/usr/bin/env python3
"""A blind ABX kit: two renders to compare, shuffled into trials the listener cannot tell apart.

    python -I tools/listen/abx_blind.py <out-dir> <trials> <name>=<file-one>,<file-two> ...

For each pair it writes <out-dir>/<name>/prueba-NN/{A,B,X}.wav — A and B are the two files in
a random order for that trial, X is one of them at random — plus <out-dir>/<name>/hoja.txt (a
sheet to write "X = A" or "X = B" on) and <out-dir>/clave/<name>.txt (which file A, B and X
were; look only after answering). Use loudness-matched files (the *-lufs23.wav copies), so the
level never gives the answer away. The odds of guessing 9 of 10 or better by chance are about
1 %. Used for Arcus against the recorded cello and double bass (round 42).
"""
import os
import random
import shutil
import sys


def main(argv):
    if len(argv) < 4:
        print(__doc__)
        return 2
    out, trials = argv[1], int(argv[2])
    rng = random.Random(os.urandom(16))
    key_dir = os.path.join(out, "clave")
    os.makedirs(key_dir, exist_ok=True)
    for spec in argv[3:]:
        name, files = spec.split("=", 1)
        one, two = files.split(",")
        for f in (one, two):
            if not os.path.isfile(f):
                print(f"missing: {f}")
                return 1
        base = os.path.join(out, name)
        shutil.rmtree(base, ignore_errors=True)
        key = [f"{name}: {os.path.basename(one)} vs {os.path.basename(two)}", ""]
        for t in range(1, trials + 1):
            d = os.path.join(base, f"prueba-{t:02d}")
            os.makedirs(d)
            a, b = (one, two) if rng.random() < 0.5 else (two, one)
            x = a if rng.random() < 0.5 else b
            shutil.copyfile(a, os.path.join(d, "A.wav"))
            shutil.copyfile(b, os.path.join(d, "B.wav"))
            shutil.copyfile(x, os.path.join(d, "X.wav"))
            key.append(
                f"prueba-{t:02d}: A = {os.path.basename(a)}, B = {os.path.basename(b)}, "
                f"X = {'A' if x == a else 'B'}"
            )
        with open(os.path.join(key_dir, f"{name}.txt"), "w", encoding="utf-8") as fh:
            fh.write("\n".join(key) + "\n")
        with open(os.path.join(base, "hoja.txt"), "w", encoding="utf-8") as fh:
            fh.write(
                f"ABX a ciegas — {name}. En cada prueba escucha A, B y X (las veces que quieras)\n"
                "y escribe si X es A o B. Despues anota cual de los dos te sono mas real.\n\n"
            )
            for t in range(1, trials + 1):
                fh.write(f"prueba-{t:02d}:  X = ___    mas real: ___\n")
        print(f"{name}: {trials} trials in {base}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
