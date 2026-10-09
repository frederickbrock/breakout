"""Build the Steelbreak graffiti-line-art LoRA dataset from art/concepts.

- chroma/white backgrounds are keyed out and replaced with the hangar void colour
- subjects are cropped tight and centred on an SDXL-friendly canvas
- captions are hand-written per group (trigger token + plain content description)
"""
import os, shutil
from PIL import Image, ImageDraw, ImageFilter, ImageChops

SRC = os.path.expanduser('~/Projects/breakout/art/concepts')
OUT = os.path.expanduser('~/Projects/lora/dataset/10_stlbrk')
VOID = (10, 14, 21)
TRIGGER = 'stlbrk_graffiti'

# (issue, round, variant, mode, caption)
#   mode: bg    = full-frame scene, keep as is (crop white margins)
#         key   = flood-key chroma from the corners, centre subject on void
#         keyg  = key every pixel near the corner colour (frames with a chroma interior)
S = []
def add(issue, picks, mode, caption):
    for p in picks:
        S.append((issue, *p.split('/'), mode, caption))

# --- backgrounds / planets / clouds (sim-rdl.10)
add('sim-rdl.10', ['r1/v3'], 'bg', 'a towering stylized cloud pillar with thick flowing wild-style contour lines, off-white, orange and teal bands, black space')
add('sim-rdl.10', ['r1/v5'], 'bg', 'a scattering of cartoon clouds with thick dark outlines and layered off-white, orange and teal bands, dark navy space')
add('sim-rdl.10', ['r1/v8'], 'bg', 'a round planet with swirling orange, teal and cream bands, thick dark outline, dark navy space')
add('sim-rdl.10', ['r1/v9'], 'bg', 'a round banded planet in orange, teal and cream with a bold dark keyline, dark navy space')
add('sim-rdl.10', ['r1/v10'], 'bg', 'a small striped planet with a cream outline ring, orange and teal stripes, dark navy space')
add('sim-rdl.10', ['r2/v1'], 'bg', 'a glowing magenta and cream sunburst surrounded by layered outlined clouds, a banded planet in the corner, dark space')
add('sim-rdl.10', ['r2/v2'], 'bg', 'a cream sun ringed by outlined clouds with white lightning bolts, a banded planet in the corner, dark space')
add('sim-rdl.10', ['r2/v4'], 'bg', 'a tall cloud pillar with thick offset contour lines in orange, teal and cream, white lightning bolts, black space')
add('sim-rdl.10', ['r2/v7'], 'bg', 'two sleek spaceships flying along glowing cyan and magenta ribbon lines in front of a banded planet, bold outlines')
add('sim-rdl.10', ['r2/v8'], 'bg', 'a banded planet framed by curved metal panels and flowing purple and teal ribbons, bold outlines')
add('sim-rdl.10', ['r2/v9', 'r2/v10'], 'bg', 'outlined clouds and a banded planet with a bright magenta energy burst, dark space')

# --- ball, paddle parts, powerups (sim-usq.2 / usq.6)
add('sim-usq.2', ['r1/v1'], 'key', 'a metallic sphere split into dark and light halves with thin orange contour lines, bold black outline')
add('sim-usq.2', ['r1/v2'], 'key', 'a dark metal sphere in a ringed socket with a glossy highlight and thin orange contour lines, bold black outline')
add('sim-usq.2', ['r1/v4'], 'key', 'a glossy grey steel ball with a cream wave line across it, hard white glint, thick black outline')
#add('sim-usq.6', ['r1/v1'], 'key', 'a glossy grey steel ball with a cream wave line across it, hard white glint, thick black outline')
add('sim-usq.6', ['r2/v2'], 'key', 'a steel ball with a cream wave line, a double off-white ring around it, hard white glint, thick black outline')
add('sim-usq.2', ['r2/v1'], 'key', 'a grey metal emitter prong with a round lens, bolted plates, thick black outline')
add('sim-usq.2', ['r2/v3', 'r2/v4'], 'key', 'a rounded metal bar filled with cyan wavy energy lines and white highlights, thick black outline')
add('sim-usq.2', ['r2/v6'], 'key', 'a purple faceted gem set in a square bolted steel frame, thick black outline')

# --- bricks (sim-usq.1)
add('sim-usq.1', ['r1/v1', 'r1/v2'], 'key', 'a rounded grey steel plate with corner bolts, offset double outline, white glint, thick black keyline')
add('sim-usq.1', ['r4/v1'], 'key', 'a rounded golden-yellow metal plate with bolts, horizontal seam, cream edge highlight, thick dark outline')
add('sim-usq.1', ['r4/v2'], 'key', 'a rounded red metal plate with yellow hazard stripes at the corners, glossy streaks, thick dark outline')
add('sim-usq.1', ['r4/v3'], 'key', 'a cracked grey steel plate with bolts and a jagged crack line, thick dark outline')
add('sim-usq.1', ['r4/v4'], 'key', 'a cracked terracotta tile plate with a broken corner, pale rim, thick dark outline')
add('sim-usq.1', ['r4/v6'], 'key', 'a cracked purple plate with a glowing violet seam line, thick dark outline')
add('sim-usq.1', ['r4/v8'], 'key', 'a rounded green metal plate with fine crack lines and a small hatch, cream rim, thick dark outline')
add('sim-usq.1', ['r4/v9'], 'key', 'a pale cyan glass panel with hexagon cracks in a steel frame, thick dark outline')
add('sim-usq.1', ['r4/v12'], 'key', 'a shattered purple plate with glowing violet cracks, thick dark outline')

# --- frames (sim-usq.3)
add('sim-usq.3', ['r1/v2'], 'keyg', 'a rounded rectangular frame in orange and cream with a black inner outline, empty centre')
add('sim-usq.3', ['r1/v5'], 'keyg', 'an oval frame with red and orange hazard stripes and cream edge lines, empty centre')
#add('sim-usq.3', ['r1/v9'], 'keyg', 'a rounded cyan frame with small rivets, cream inner line and white glints, empty centre')
add('sim-usq.3', ['r1/v10'], 'keyg', 'a round cyan porthole ring with four bolts and a white sparkle, empty centre')
add('sim-usq.3', ['r1/v12'], 'keyg', 'an oval purple frame with cream inner line and cyan diamond studs, empty centre')
add('sim-usq.3', ['r2/v2'], 'keyg', 'an orange rounded frame with white side tabs and corner marks, empty centre')
add('sim-usq.3', ['r3/v4'], 'keyg', 'a wide purple and orange rounded frame with a cream inner line, empty centre')

# --- station wall line art (sim-73n.3)
add('sim-73n.3', ['r4/v1', 'r4/v4'], 'bg', 'a dark navy wall panel with rounded capsule shapes, inlaid flowing contour lines in orange, teal and off-white')

BUCKETS = [(1024, 1024), (1344, 768), (768, 1344), (1216, 832), (832, 1216), (1536, 640)]

def bucket(w, h):
    ar = w / h
    return min(BUCKETS, key=lambda b: abs(b[0] / b[1] - ar))

def corner_colour(im):
    px = im.load(); w, h = im.size
    pts = [px[2, 2], px[w - 3, 2], px[2, h - 3], px[w - 3, h - 3]]
    return tuple(sorted(c[i] for c in pts)[1] for i in range(3))  # median-ish

def key_mask(im, mode, tol=55):
    """Return L mask: 255 where subject."""
    key = corner_colour(im)
    diff = ImageChops.difference(im, Image.new('RGB', im.size, key)).convert('L')
    near = diff.point(lambda v: 0 if v * 3 > tol * 1.7 else 255)  # 255 = close to key (background)
    # channel-wise max difference is stricter than the luma diff above
    r, g, b = [ImageChops.difference(c, Image.new('L', im.size, key[i])) for i, c in enumerate(im.split())]
    mx = ImageChops.lighter(ImageChops.lighter(r, g), b)
    bgish = mx.point(lambda v: 255 if v <= tol else 0)
    if mode == 'keyg':
        bg = bgish
    else:
        # keep only the part connected to the border
        flood = bgish.copy()
        w, h = im.size
        seeds = [(1, 1), (w - 2, 1), (1, h - 2), (w - 2, h - 2), (w // 2, 1), (w // 2, h - 2), (1, h // 2), (w - 2, h // 2)]
        for s in seeds:
            if flood.getpixel(s) == 255:
                ImageDraw.floodfill(flood, s, 128)
        bg = flood.point(lambda v: 255 if v == 128 else 0)
    bg = bg.filter(ImageFilter.MaxFilter(5)).filter(ImageFilter.GaussianBlur(0.8))  # eat the halo
    return ImageChops.invert(bg)

def crop_white(im, thresh=244):
    g = im.convert('L').point(lambda v: 255 if v < thresh else 0)
    bb = g.getbbox()
    return im.crop(bb) if bb else im

def fit_on_canvas(subj, mask, pad=0.07):
    bb = mask.getbbox()
    subj, mask = subj.crop(bb), mask.crop(bb)
    cw, ch = bucket(*subj.size)
    cw, ch = (cw, ch)
    scale = min(cw * (1 - 2 * pad) / subj.width, ch * (1 - 2 * pad) / subj.height)
    nw, nh = max(1, round(subj.width * scale)), max(1, round(subj.height * scale))
    subj = subj.resize((nw, nh), Image.LANCZOS); mask = mask.resize((nw, nh), Image.LANCZOS)
    canvas = Image.new('RGB', (cw, ch), VOID)
    canvas.paste(subj, ((cw - nw) // 2, (ch - nh) // 2), mask)
    return canvas

def scene(im):
    im = crop_white(im)
    cw, ch = bucket(*im.size)
    # cover-crop to the bucket aspect, then resize
    s = max(cw / im.width, ch / im.height)
    im = im.resize((round(im.width * s), round(im.height * s)), Image.LANCZOS)
    l, t = (im.width - cw) // 2, (im.height - ch) // 2
    return im.crop((l, t, l + cw, t + ch))

def main():
    shutil.rmtree(OUT, ignore_errors=True); os.makedirs(OUT)
    for n, (issue, rnd, var, mode, cap) in enumerate(S, 1):
        f = f'{SRC}/{issue}/round-{rnd[1:]}/{var}.png'
        im = Image.open(f).convert('RGB')
        out = scene(im) if mode == 'bg' else fit_on_canvas(im, key_mask(im, mode))
        name = f'{n:02d}_{issue}_{rnd}{var}'
        out.save(f'{OUT}/{name}.png')
        open(f'{OUT}/{name}.txt', 'w').write(f'{TRIGGER}, {cap}\n')
    print(len(S), 'images ->', OUT)

if __name__ == '__main__':
    main()
