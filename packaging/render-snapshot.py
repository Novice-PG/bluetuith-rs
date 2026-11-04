#!/usr/bin/env python3
"""Turn the JSON cell dump produced by `examples/snapshot.rs` into a PNG.

Usage: python3 packaging/render-snapshot.py frame.json frame.png
"""
import json
import sys
from PIL import Image, ImageDraw, ImageFont

REG = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"
BOLD = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"

def main():
    src, dst = sys.argv[1], sys.argv[2]
    data = json.load(open(src, encoding="utf-8"))
    w, h = data["width"], data["height"]
    cells = data["cells"]
    size = 15
    font = ImageFont.truetype(REG, size)
    bold = ImageFont.truetype(BOLD, size)
    cw = round(font.getlength("M"))
    box = font.getbbox("Hg")
    ch = box[3] - box[1] + 4
    img = Image.new("RGB", (w * cw, h * ch), (13, 17, 27))
    draw = ImageDraw.Draw(img)
    for y in range(h):
        for x in range(w):
            sym, fg, bg, is_bold = cells[y * w + x]
            px, py = x * cw, y * ch
            draw.rectangle([px, py, px + cw, py + ch], fill=tuple(bg))
            if sym and sym != " ":
                f = bold if is_bold else font
                draw.text((px, py + 1), sym, font=f, fill=tuple(fg))
    img.save(dst)
    print(f"{dst}: {img.width}x{img.height}")

if __name__ == "__main__":
    main()
