"""Inspect a render: crop a region of a PNG and magnify it with pixelated scaling.
python -I crop.py <png> x y w h zoom <out.png>"""
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import render, write


def crop(src, x, y, w, h, z, out, smooth=False):
    url = "file:///" + os.path.abspath(src).replace("\\", "/")
    ir = "auto" if smooth else "pixelated"
    import struct
    with open(src, "rb") as f:
        pw, ph = struct.unpack(">II", f.read(24)[16:24])
    html = (f"<!doctype html><html><body style='margin:0;background:#000;overflow:hidden'>"
            f"<div style='width:{w * z}px;height:{h * z}px;overflow:hidden;position:relative'>"
            f"<img src='{url}' style='position:absolute;left:{-x * z}px;top:{-y * z}px;"
            f"width:{pw * z}px;height:{ph * z}px;image-rendering:{ir}'></div></body></html>")
    tmp = out + ".html"
    write(tmp, html)
    render(tmp, out, w * z, h * z, 1)
    os.remove(tmp)


if __name__ == "__main__":
    a = sys.argv[1:]
    crop(a[0], int(a[1]), int(a[2]), int(a[3]), int(a[4]), int(a[5]), a[6])
