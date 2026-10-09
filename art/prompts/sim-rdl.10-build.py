#!/usr/bin/env python3
"""sim-rdl.10: build the parallax layers (option 2: procedural L0, hand-built L1/L2/planet/glow).
Run: uv run --with pillow --with numpy --with scipy python art/prompts/sim-rdl.10-build.py <concepts>/sim-rdl.10 <out_dir>
Sources: r1/v2 (L0 look), r2/v9 (L1 clouds + burst), r1/v6 (L2 wisps), r1/v9 (planet), r2/v5 (bolt style)."""
import sys, math, random
import numpy as np
from PIL import Image, ImageDraw, ImageFilter
from scipy import ndimage as ndi

SRC, OUT = sys.argv[1], sys.argv[2]
W, H = 1440, 2160
VOID = (10, 14, 21)
def load(p): return Image.open(f"{SRC}/{p}.png").convert("RGB")

# ---------- L0: procedural stars, wrap-seamless ----------
def quant(im, colors=96):
    """Palette-quantise the RGB of an RGBA layer (flat colour art), keep the 8-bit alpha, to stay under 1 MB."""
    rgb = im.convert("RGB").quantize(colors=colors, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE).convert("RGB")
    rgb.putalpha(im.getchannel("A")); return rgb

def build_l0():
    S = 2; rnd = random.Random(7)
    img = Image.new("RGB", (W*S, H*S), VOID); d = ImageDraw.Draw(img)
    pal = [(226,223,210), (95,143,143), (217,140,100), (169,220,255)]
    def at(x, y, fn):
        for dy in (-H, 0, H): fn(x*S, (y+dy)*S)
    for _ in range(420):  # dots
        x, y = rnd.uniform(0, W), rnd.uniform(0, H); r = rnd.choice([1,1,1.5,2]); c = rnd.choice(pal)
        c = tuple(int(VOID[i]+(c[i]-VOID[i])*rnd.uniform(.35,.8)) for i in range(3))
        at(x, y, lambda X, Y: d.ellipse([X-r*S/1.2, Y-r*S/1.2, X+r*S/1.2, Y+r*S/1.2], fill=c))
    for _ in range(70):  # crosses, x, rings
        x, y = rnd.uniform(0, W), rnd.uniform(0, H); k = rnd.choice("+x o".replace(" ", "o")); s = rnd.choice([7,9,12,15])
        c = rnd.choice(pal); c = tuple(int(VOID[i]+(c[i]-VOID[i])*rnd.uniform(.4,.75)) for i in range(3)); w = 3
        def f(X, Y, k=k, s=s, c=c):
            s2 = s*S/2
            if k == "+": d.line([X-s2, Y, X+s2, Y], fill=c, width=w); d.line([X, Y-s2, X, Y+s2], fill=c, width=w)
            elif k == "x": q = s2*.7; d.line([X-q, Y-q, X+q, Y+q], fill=c, width=w); d.line([X-q, Y+q, X+q, Y-q], fill=c, width=w)
            else: d.ellipse([X-s2*.6, Y-s2*.6, X+s2*.6, Y+s2*.6], outline=c, width=w)
        at(x, y, f)
    for _ in range(9):  # tiny outlined galaxies
        x, y = rnd.uniform(60, W-60), rnd.uniform(0, H); a = rnd.uniform(-.5, .5); L = rnd.uniform(14, 26)
        warm = rnd.random() < .5; core = (255,225,190) if warm else (200,225,240); ring = (217,140,100) if warm else (95,143,143)
        def f(X, Y):
            g = Image.new("RGBA", (int(L*S*2.2), int(L*S*2.2)), (0,0,0,0)); gd = ImageDraw.Draw(g); cx = cy = g.width/2
            gd.ellipse([cx-L*S, cy-L*S*.38, cx+L*S, cy+L*S*.38], fill=(20,26,36,255), outline=ring, width=3)
            gd.ellipse([cx-L*S*.55, cy-L*S*.17, cx+L*S*.55, cy+L*S*.17], outline=(226,223,210), width=2)
            gd.ellipse([cx-L*S*.2, cy-L*S*.1, cx+L*S*.2, cy+L*S*.1], fill=core)
            g = g.rotate(math.degrees(a), resample=Image.BICUBIC)
            img.paste(g, (int(X-g.width/2), int(Y-g.height/2)), g)
        at(x, y, f)
    return img.resize((W, H), Image.LANCZOS)

# ---------- helpers for cutting cloud art out of the black concept background ----------
def cutout(rgb, thr=26, grow=2):
    a = np.asarray(rgb).astype(float); lum = a.max(2)
    fg = lum > thr
    fg = ndi.binary_closing(fg, iterations=3)           # bridge inner black keylines
    fg = ndi.binary_opening(fg, iterations=1)
    holes, nh = ndi.label(~fg)   # fill small enclosed gaps (inner keylines), leave real sky
    hs = ndi.sum(~fg, holes, range(1, nh+1))
    fg |= np.isin(holes, [i+1 for i, z in enumerate(hs) if z < 600])
    lab, n = ndi.label(fg); sizes = ndi.sum(fg, lab, range(1, n+1))
    keep = np.isin(lab, [i+1 for i, s in enumerate(sizes) if s > 400]); keep = ndi.binary_dilation(keep, iterations=grow)
    al = ndi.gaussian_filter(keep.astype(float), 1.0)
    return a, al

def fade_edges(al, px, top=False, bottom=False, left=False, right=False):
    h, w = al.shape; ramp = lambda n: np.linspace(0, 1, n)**1.5
    if top: al[:px] *= ramp(px)[:, None]
    if bottom: al[-px:] *= ramp(px)[::-1][:, None]
    if left: al[:, :px] *= ramp(px)[None, :]
    if right: al[:, -px:] *= ramp(px)[::-1][None, :]
    return al

def desat_violet(a):
    """Turn the burst's violet into dusty blue-grey (the glow layer carries the violet)."""
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    vio = (b > g + 25) & (r > g + 10)
    lum = (r+g+b)/3
    t = np.stack([lum*.75, lum*.95, lum*1.1], -1)
    a = a.copy(); a[vio] = t[vio]; return a

def paste_sprite(canvas, rgb, al, scale, x, y, dim):
    h, w = al.shape
    im = Image.fromarray(np.clip(rgb, 0, 255).astype(np.uint8)).convert("RGBA")
    A = Image.fromarray((np.clip(al*dim, 0, 1)*255).astype(np.uint8))
    nw, nh = int(w*scale), int(h*scale)
    im = im.resize((nw, nh), Image.LANCZOS); A = A.resize((nw, nh), Image.LANCZOS); im.putalpha(A)
    layer = Image.new("RGBA", canvas.size, (0,0,0,0)); layer.paste(im, (x, y), im)
    return Image.alpha_composite(canvas, layer)

TOP_Y = 60   # where the burst/top band sits in the 2160 canvas (the glow mask is aligned to it)

def build_l1():
    v9 = load("round-2/v9"); a, al = cutout(v9)
    yy, xx = np.mgrid[:1024, :1024]
    planet = ((xx-960)**2 + (yy-960)**2) < 296**2           # drop the planet; its own sprite ships separately
    al = al.copy(); al[planet] = 0
    al = ndi.gaussian_filter(al, 2.0)
    a = desat_violet(a)
    canvas = Image.new("RGBA", (W, H), (0,0,0,0)); s = 1440/1024
    # top band (burst + upper clouds): rows 0..450, only cut through at the very top edge
    canvas = paste_sprite(canvas, a[:450], fade_edges(al[:450].copy(), 50, top=True, bottom=True), s, 0, TOP_Y, .62)
    # lower mass (back clouds + big cloud): rows 470..1024, cut only at the bottom edge
    canvas = paste_sprite(canvas, a[470:], fade_edges(al[470:].copy(), 50, top=True, bottom=True), s, 0, 780, .6)
    # lower-half echo: the upper-right cloud, mirrored left-right and dimmer
    ra, ral = a[:260, 480:], fade_edges(al[:260, 480:].copy(), 50, top=True, bottom=True, left=True)
    canvas = paste_sprite(canvas, ra[:, ::-1], ral[:, ::-1], s, 0, 1640, .5)
    return canvas

def build_glow():
    """Glow mask: white, alpha = intensity. Forks (burst bolts) + soft halo, aligned with L1 'top' sprite at y=130."""
    r2 = np.asarray(load("round-2/v9")).astype(float)
    # bolts + burst: bright, violet/white pixels in the top-left area
    reg = r2[:430]; lum = reg.max(2); vio = (reg[...,2] > reg[...,1] + 25)
    bolt = ((lum > 215) & (reg.min(2) > 170)) | vio
    yy, xx = np.mgrid[:430, :1024]; near = ((xx-290)**2 + (yy-130)**2) < 330**2
    m = (bolt & near).astype(float)
    big = Image.fromarray((m*255).astype(np.uint8)).resize((1440, int(430*1440/1024)), Image.LANCZOS)
    core = np.asarray(big).astype(float)/255
    halo = ndi.gaussian_filter(core, 22)*1.4 + ndi.gaussian_filter(core, 6)*.6
    al = np.clip(core*.9 + halo*.55, 0, 1)
    full = np.zeros((H, W)); full[TOP_Y:TOP_Y+al.shape[0]] = al*.8   # forks sit in the upper part; kept moderate (code caps GLOW_PEAK)
    out = np.zeros((H, W, 4), np.uint8); out[..., :3] = 255; out[..., 3] = (full*255).astype(np.uint8)
    return Image.fromarray(out, "RGBA")

def build_l2():
    v6 = load("round-1/v6"); a, al = cutout(v6, thr=20, grow=1)
    canvas = Image.new("RGBA", (W, H), (0,0,0,0)); s = 1440/1024
    # wisps cover the whole concept; place the full tile at two heights (second mirrored) for 2160, all inside the canvas
    sal = fade_edges(al.copy(), 30, top=True, bottom=True)
    canvas = paste_sprite(canvas, a, sal, s, 0, 40, .55)
    canvas = paste_sprite(canvas, a[::-1, ::-1], sal[::-1, ::-1], s, 0, 1110, .4)
    return canvas

def build_planet():
    p = np.asarray(load("round-1/v9")).astype(float)
    yy, xx = np.mgrid[:1024, :1024]
    # fit: circle by thresholding vs background navy
    bg = np.array([22, 25, 32.]); diff = np.abs(p-bg).max(2) > 30
    diff = ndi.binary_opening(ndi.binary_fill_holes(diff), iterations=2)
    lab, n = ndi.label(diff); big = lab == (np.argmax(ndi.sum(diff, lab, range(1, n+1)))+1)
    cx, cy, r = 514, 520, 287   # eyeballed: the shadow side blends into the backdrop, so thresholding finds only the lit part
    print("planet circle", cx, cy, r)
    ss = 4; disc = Image.new("L", (1024*ss, 1024*ss), 0); ImageDraw.Draw(disc).ellipse([(cx-r+3)*ss, (cy-r+3)*ss, (cx+r-3)*ss, (cy+r-3)*ss], fill=255)
    A = disc.resize((1024, 1024), Image.LANCZOS)
    im = Image.fromarray(p.astype(np.uint8)).convert("RGBA"); im.putalpha(A)
    D = 520; box = (int(cx-r), int(cy-r), int(cx+r), int(cy+r)); im = im.crop(box).resize((D, D), Image.LANCZOS)
    # dim a little so it sits far away
    arr = np.asarray(im).astype(float); arr[..., :3] *= .85; im = Image.fromarray(arr.astype(np.uint8), "RGBA")
    out = Image.new("RGBA", (640, 640), (0,0,0,0)); out.paste(im, (60, 60), im); return out

if __name__ == "__main__":
    build_l0().save(f"{OUT}/space_l0.png", optimize=True)
    quant(build_l1()).save(f"{OUT}/space_l1.png", optimize=True)
    quant(build_l2()).save(f"{OUT}/space_l2.png", optimize=True)
    build_glow().save(f"{OUT}/space_l1_glow.png", optimize=True)
    build_planet().save(f"{OUT}/planet.png", optimize=True)
