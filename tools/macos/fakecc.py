"""A stand-in C compiler for `clippy --target *-apple-darwin` on Windows (xcheck.sh).

Build scripts of ring, ogg and LAME compile C for the target, which needs the macOS SDK.
clippy only type-checks Rust and links nothing, so an empty object file per C file is
enough: this writes one wherever the compiler was asked to (`-o x`, `-ox`, `-Fox`).
"""
import sys

args = sys.argv[1:]
for i, a in enumerate(args):
    if a == "-o" and i + 1 < len(args):
        open(args[i + 1], "wb").close()
    elif a.startswith("-Fo"):
        open(a[3:], "wb").close()
    elif a.startswith("-o") and len(a) > 2:
        open(a[2:], "wb").close()
sys.exit(0)
