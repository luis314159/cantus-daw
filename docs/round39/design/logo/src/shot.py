"""Render an HTML file with Edge headless: python shot.py page.html out.png W H [scale]."""
import os
import subprocess
import sys

EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"


def shot(html, png, w, h, scale=1):
    html = os.path.abspath(html).replace("\\", "/")
    png = os.path.abspath(png)
    subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    "--force-device-scale-factor=%s" % scale, "--window-size=%d,%d" % (w, h),
                    "--default-background-color=00000000",
                    "--screenshot=%s" % png, "file:///" + html],
                   check=True, capture_output=True, timeout=120)


if __name__ == "__main__":
    a = sys.argv[1:]
    shot(a[0], a[1], int(a[2]), int(a[3]), a[4] if len(a) > 4 else 1)
