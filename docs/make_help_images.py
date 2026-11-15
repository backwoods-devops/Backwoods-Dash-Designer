#!/usr/bin/env python3
"""Crops and shrinks docs/raw/*.png into src/help_img/*.png for the manual. Run after make_shots.sh."""
import os
from PIL import Image, ImageDraw, ImageFont
here = os.path.dirname(os.path.abspath(__file__))
raw, out = os.path.join(here, 'raw'), os.path.join(here, '..', 'src', 'help_img')
os.makedirs(out, exist_ok=True)
MAXW = 900
def load(n): return Image.open(os.path.join(raw, n + '.png')).convert('RGB')
def save(img, name):
    if img.width > MAXW:
        img = img.resize((MAXW, round(img.height * MAXW / img.width)), Image.LANCZOS)
    img = img.quantize(colors=200, method=Image.MEDIANCUT, dither=Image.NONE)
    img.save(os.path.join(out, name + '.png'), optimize=True)
def crop(n, box, name=None): save(load(n).crop(box), name or n)

def badge(img, xy, label):
    d = ImageDraw.Draw(img)
    x, y = xy
    d.ellipse((x - 16, y - 16, x + 16, y + 16), fill=(255, 138, 30), outline=(20, 20, 20), width=2)
    try: f = ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf', 20)
    except Exception: f = ImageFont.load_default()
    d.text((x, y), label, fill=(20, 20, 20), font=f, anchor='mm')

crop('start', (0, 150, 1480, 900))
for n in ['wiz_start', 'wiz_wind', 'wiz_sections', 'wiz_finish']:
    crop(n, (88, 38, 1392, 900))
m = load('main')
for xy, l in [((1000, 25), '1'), ((140, 520), '2'), ((700, 330), '3'), ((1300, 300), '4'), ((700, 760), '5')]:
    badge(m, xy, l)
save(m, 'layout')
crop('main', (460, 80, 905, 124), 'viewbar')
crop('wallview', (270, 50, 1125, 570))
crop('walls', (270, 570, 1125, 860), 'walls_ed')
crop('main', (270, 570, 1125, 860), 'profile')
crop('plan', (270, 570, 1125, 860))
crop('height', (270, 570, 1125, 860))
crop('size', (270, 50, 1125, 570))
crop('shapes', (0, 50, 270, 860), 'shapes_tab')
for n in ['draw', 'trace', 'cut']: crop(n, (0, 50, 1480, 860))
crop('lighting', (560, 240, 920, 660))
crop('export', (495, 80, 985, 820))
crop('shape_combo', (270, 50, 1480, 860))
crop('shape_3d', (270, 50, 1480, 860))
crop('leave', (270, 50, 1480, 860))
