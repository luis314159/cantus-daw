"""crop.py <src.png> <out.png> x y w h [scale] — cut a region and enlarge it with
nearest-neighbour, so the real pixels are judged (no resampling hides a soft edge)."""
import sys
from PIL import Image
a = sys.argv
im = Image.open(a[1]).convert('RGB')
x, y, w, h = map(int, a[3:7])
s = int(a[7]) if len(a) > 7 else 4
c = im.crop((x, y, x + w, y + h))
c.resize((w * s, h * s), Image.NEAREST).save(a[2])
print(a[2], c.size)
