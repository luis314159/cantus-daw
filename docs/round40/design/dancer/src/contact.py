"""A contact sheet of a GIF's frames (to check motion by eye): contact.py in.gif out.png step [scale] [cols]"""
import sys
from PIL import Image, ImageSequence, ImageDraw
src, out = sys.argv[1], sys.argv[2]
step = int(sys.argv[3]) if len(sys.argv) > 3 else 3
scale = float(sys.argv[4]) if len(sys.argv) > 4 else 0.5
cols = int(sys.argv[5]) if len(sys.argv) > 5 else 4
fr = [im.convert("RGB") for im in ImageSequence.Iterator(Image.open(src))][::step]
w, h = int(fr[0].width * scale), int(fr[0].height * scale)
rows = (len(fr) + cols - 1) // cols
sheet = Image.new("RGB", (w * cols, h * rows), "white")
for i, im in enumerate(fr):
    sheet.paste(im.resize((w, h), Image.LANCZOS), ((i % cols) * w, (i // cols) * h))
    ImageDraw.Draw(sheet).text(((i % cols) * w + 4, (i // cols) * h + h - 14), str(i * step), fill=(255, 80, 80))
sheet.save(out)
print(out, len(fr))
