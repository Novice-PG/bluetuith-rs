#!/usr/bin/env python3
"""Turn the JSON cell dump produced by `examples/snapshot.rs` into a PNG.

Usage: python3 packaging/render-snapshot.py frame.json frame.png

Latin text and box-drawing glyphs use DejaVu Mono (1 cell wide). East-Asian
wide characters (CJK) use Noto Sans Mono CJK SC at 2x size so each ideograph
fills exactly two cells, matching ratatui's cell grid. English screenshots are
therefore rendered byte-for-byte as before.
"""
import json
import sys
import unicodedata
from PIL import Image, ImageDraw, ImageFont

REG = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"
BOLD = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"
# TTC index 7 = "Noto Sans Mono CJK SC" (Simplified Chinese, monospace).
CJK_REG = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"
CJK_BOLD = "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc"
CJK_INDEX = 7

def is_wide(sym):
    return bool(sym) and unicodedata.east_asian_width(sym) in ("W", "F")

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
    # Each ideograph must occupy exactly two cells: size it so its advance is
    # 2*cw (the cell width), not size*2 (which overflows into adjacent rows).
    cjk_size = 2 * cw
    cjk_reg = ImageFont.truetype(CJK_REG, cjk_size, index=CJK_INDEX)
    cjk_bold = ImageFont.truetype(CJK_BOLD, cjk_size, index=CJK_INDEX)
    img = Image.new("RGB", (w * cw, h * ch), (13, 17, 27))
    draw = ImageDraw.Draw(img)

    def paint(px, py, sym, fg, is_bold):
        if not sym or sym == " ":
            return
        if is_wide(sym):
            # The next cell is ratatui's blank "spacing" half of the wide
            # glyph; drawing here with a 2-cell font fills both halves.
            f = cjk_bold if is_bold else cjk_reg
            # Fixed vertical anchor for every CJK glyph. Per-glyph textbbox
            # varies by a pixel or two across ideographs (and between regular
            # and bold), which makes a column of labels look ragged/touching.
            # Align the font's top to py so all glyphs share one baseline.
            draw.text((px, py), sym, font=f, fill=tuple(fg), anchor="lt")
        else:
            f = bold if is_bold else font
            draw.text((px, py + 1), sym, font=f, fill=tuple(fg))

    has_wide = any(is_wide(c[0]) for c in cells)
    if has_wide:
        # CJK frames: draw all backgrounds first, then all glyphs. A 2-cell
        # glyph would otherwise have its right half repainted by the following
        # blank "spacing" cell's background rectangle.
        for y in range(h):
            for x in range(w):
                px, py = x * cw, y * ch
                draw.rectangle([px, py, px + cw, py + ch], fill=tuple(cells[y * w + x][2]))
        for y in range(h):
            for x in range(w):
                sym, fg, _, is_bold = cells[y * w + x]
                paint(x * cw, y * ch, sym, fg, is_bold)
    else:
        # Latin-only frames: keep the original interleaved draw order byte-for-byte.
        for y in range(h):
            for x in range(w):
                sym, fg, bg, is_bold = cells[y * w + x]
                px, py = x * cw, y * ch
                draw.rectangle([px, py, px + cw, py + ch], fill=tuple(bg))
                paint(px, py, sym, fg, is_bold)
    img.save(dst)
    print(f"{dst}: {img.width}x{img.height}")

if __name__ == "__main__":
    main()
