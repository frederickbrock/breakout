import math
import random

R = random.Random(4711)

W, H = 1600, 900
HX, HY, HW, HH = 32, 44, 1024, 576
CX, CW = 1080, 488
SY, SH = 644, 224

MONO = "'JetBrains Mono','IBM Plex Mono',Consolas,'Courier New',monospace"

SHEET = "#080b0f"
PANEL = "#0c1118"
LINE = "#1c242f"
LINE2 = "#2b3644"
INK = "#dde6f1"
INK2 = "#8a9aab"
INK3 = "#5c6876"
CYAN = "#4fd8ff"
ICE = "#a9dcff"
AMBER = "#ffab4d"
RED = "#ff6a4d"
VIOLET = "#b58cff"

BW, BH = 62, 26
GX, GY = 70, 34
X0 = 61

DEFS = []
ART = []
FX = []
FRAME = []
OVER = []
SIDE = []
STRIP = []


def n(v):
    return round(v, 2)


def grad_v(gid, stops):
    s = "".join(
        f'<stop offset="{o}" stop-color="{c}" stop-opacity="{a}"/>' for o, c, a in stops
    )
    DEFS.append(
        f'<linearGradient id="{gid}" x1="0" y1="0" x2="0" y2="1">{s}</linearGradient>'
    )


def grad_h(gid, stops, x1=0, y1=0, x2=1, y2=0):
    s = "".join(
        f'<stop offset="{o}" stop-color="{c}" stop-opacity="{a}"/>' for o, c, a in stops
    )
    DEFS.append(
        f'<linearGradient id="{gid}" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" '
        f'gradientUnits="userSpaceOnUse">{s}</linearGradient>'
    )


def grad_r(gid, stops, cx=0.5, cy=0.5, r=0.5, fx=0.5, fy=0.5):
    s = "".join(
        f'<stop offset="{o}" stop-color="{c}" stop-opacity="{a}"/>' for o, c, a in stops
    )
    DEFS.append(
        f'<radialGradient id="{gid}" cx="{cx}" cy="{cy}" r="{r}" fx="{fx}" fy="{fy}">'
        f"{s}</radialGradient>"
    )


def filt(fid, body):
    DEFS.append(f'<filter id="{fid}" x="-60%" y="-60%" width="220%" height="220%">{body}</filter>')


def esc(t):
    return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def T(x, y, s, size=10, fill=INK2, weight=400, anchor="start", ls=0.4, op=1, fam=MONO):
    return (
        f'<text x="{n(x)}" y="{n(y)}" font-family="{fam}" font-size="{size}" '
        f'fill="{fill}" font-weight="{weight}" text-anchor="{anchor}" '
        f'letter-spacing="{ls}" opacity="{op}">{esc(s)}</text>'
    )


def wrap(s, width):
    words, lines, cur = s.split(), [], ""
    for w in words:
        if len(cur) + len(w) + 1 > width:
            lines.append(cur)
            cur = w
        else:
            cur = f"{cur} {w}".strip()
    if cur:
        lines.append(cur)
    return lines


def heading(x, y, s, w=CW):
    SIDE.append(T(x, y, s.upper(), 10.5, INK, 700, ls=1.8))
    SIDE.append(f'<line x1="{x}" y1="{y + 7}" x2="{x + w}" y2="{y + 7}" stroke="{LINE2}" stroke-width="1"/>')
    SIDE.append(f'<line x1="{x}" y1="{y + 7}" x2="{x + 34}" y2="{y + 7}" stroke="{CYAN}" stroke-width="1.6"/>')


def shead(x, y, s, w):
    STRIP.append(T(x, y, s.upper(), 10, INK, 700, ls=1.6))
    STRIP.append(f'<line x1="{x}" y1="{y + 7}" x2="{x + w}" y2="{y + 7}" stroke="{LINE2}"/>')
    STRIP.append(f'<line x1="{x}" y1="{y + 7}" x2="{x + 26}" y2="{y + 7}" stroke="{CYAN}" stroke-width="1.6"/>')


CLASSES = {
    "ceramic": ("#ffdcc8", "#ff7d58", "#79261a", "#ffc0a6", RED, "FIRED CERAMIC", 1),
    "titanium": ("#eef6ff", "#9fb5ca", "#2f3c4a", "#d3e9ff", "#7fc4ff", "TITANIUM ALLOY", 2),
    "tungsten": ("#ffe6bd", "#d69a4e", "#4f3212", "#ffd08a", AMBER, "TUNGSTEN PLATE", 3),
    "reactor": ("#f6ecff", "#a98cf0", "#331f5c", "#dcc9ff", VIOLET, "REACTOR CORE", 4),
}

for key, (top, mid, bot, seam, glow, name, hits) in CLASSES.items():
    grad_v(f"g_{key}", [(0, top, 1), (0.34, mid, 1), (0.72, mid, 1), (1, bot, 1)])
grad_v("g_gloss", [(0, "#ffffff", 0.34), (0.5, "#ffffff", 0.05), (1, "#000000", 0.22)])
grad_v("g_crack", [(0, "#000000", 0.55), (1, "#000000", 0.2)])

DEFS.append(
    '<pattern id="p_hazard" width="9" height="9" patternUnits="userSpaceOnUse" '
    'patternTransform="rotate(35)"><rect width="9" height="9" fill="#00000000"/>'
    f'<rect width="4.2" height="9" fill="{AMBER}" opacity="0.5"/></pattern>'
)
DEFS.append(
    '<pattern id="p_scan" width="1" height="3" patternUnits="userSpaceOnUse">'
    '<rect width="1" height="1" fill="#000000" opacity="0.34"/></pattern>'
)
DEFS.append(
    '<pattern id="p_grid" width="16" height="16" patternUnits="userSpaceOnUse">'
    f'<path d="M16 0H0V16" fill="none" stroke="{LINE}" stroke-width="0.5" opacity="0.5"/></pattern>'
)

filt("f2", '<feGaussianBlur stdDeviation="2"/>')
filt("f5", '<feGaussianBlur stdDeviation="5"/>')
filt("f10", '<feGaussianBlur stdDeviation="10"/>')
filt("f22", '<feGaussianBlur stdDeviation="22"/>')
filt("f40", '<feGaussianBlur stdDeviation="40"/>')
filt(
    "f_grain",
    '<feTurbulence type="fractalNoise" baseFrequency="0.85" numOctaves="3" stitchTiles="stitch"/>'
    '<feColorMatrix type="saturate" values="0"/>',
)
filt(
    "f_neb",
    '<feTurbulence type="fractalNoise" baseFrequency="0.0045" numOctaves="4" seed="9"/>'
    '<feColorMatrix type="matrix" values="0 0 0 0 0.16  0 0 0 0 0.34  0 0 0 0 0.52  0 0 0 -0.9 0.5"/>'
    '<feGaussianBlur stdDeviation="6"/>',
)
filt(
    "f_dust",
    '<feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="1" seed="3"/>'
    '<feColorMatrix type="matrix" values="0 0 0 0 0.75  0 0 0 0 0.86  0 0 0 0 1  0 0 0 -1.4 0.62"/>',
)

grad_v("g_sky", [(0, "#070a11", 1), (0.42, "#0b1119", 1), (0.78, "#101a24", 1), (1, "#050709", 1)])
grad_r("g_core", [(0, "#1d3346", 0.95), (1, "#0a1017", 0)], 0.5, 0.42, 0.72)
grad_h("g_shaft_c", [(0, ICE, 0.5), (1, ICE, 0)], 120, 0, 470, 520)
grad_h("g_shaft_a", [(0, AMBER, 0.42), (1, AMBER, 0)], 940, 576, 520, 180)
grad_v("g_deck", [(0, "#141c26", 1), (0.35, "#0b1017", 1), (1, "#04060a", 1)])
grad_r("g_ball", [(0, "#ffffff", 1), (0.1, "#e7eef6", 1), (0.34, "#9dabbb", 1), (0.62, "#5a6675", 1), (0.88, "#242b34", 1), (1, "#151a21", 1)], 0.42, 0.34, 0.62, 0.34, 0.26)
grad_r("g_ball_rim", [(0, AMBER, 0), (0.62, AMBER, 0), (0.88, "#ffc389", 0.85), (1, "#ffc389", 0)], 0.5, 0.5, 0.5)
grad_r("g_ball_cool", [(0, CYAN, 0), (0.7, CYAN, 0), (0.92, "#8fd8ff", 0.55), (1, "#8fd8ff", 0)], 0.5, 0.5, 0.5)
grad_r("g_hot", [(0, "#ffffff", 0.95), (0.4, "#dff0ff", 0.4), (1, "#dff0ff", 0)], 0.5, 0.5, 0.5)
grad_v("g_field", [(0, "#eaffff", 0.95), (0.4, CYAN, 0.85), (1, "#1f7ea8", 0.5)])
grad_v("g_prong", [(0, "#c9d6e4", 1), (0.3, "#6d7c8d", 1), (1, "#1b222b", 1)])
grad_v("g_beam", [(0, "#ffffff", 0.0), (0.15, "#9fd4ff", 0.3), (1, "#9fd4ff", 0)])
grad_v("g_truss", [(0, "#182130", 1), (1, "#0a0f16", 1)])
grad_r("g_vig", [(0, "#000000", 0), (0.55, "#000000", 0.06), (1, "#000000", 0.88)], 0.5, 0.48, 0.78)
grad_h("g_streak", [(0, "#ffffff", 0), (0.25, "#bfe6ff", 0.35), (0.72, "#eaf7ff", 0.75), (1, "#ffffff", 0.95)], 792, 176, 336, 462)

DEFS.insert(0, f'<clipPath id="c_hero"><rect x="0" y="0" width="{HW}" height="{HH}"/></clipPath>')


def brick(x, y, key, dmg=0, out=None, scale=1):
    top, mid, bot, seam, glow, name, hits = CLASSES[key]
    w, h = BW * scale, BH * scale
    g = [f'<g transform="translate({n(x)},{n(y)})">']
    g.append(f'<rect width="{n(w)}" height="{n(h)}" rx="{3 * scale}" fill="url(#g_{key})"/>')
    g.append(f'<rect width="{n(w)}" height="{n(h)}" rx="{3 * scale}" fill="url(#g_gloss)"/>')
    if key == "tungsten":
        g.append(
            f'<rect x="3" y="3" width="{n(w - 6)}" height="{n(h - 6)}" rx="2" '
            f'fill="url(#p_hazard)" opacity="0.3"/>'
        )
    g.append(f'<rect x="0.6" y="0.6" width="{n(w - 1.2)}" height="{n(h - 1.2)}" rx="2.6" fill="none" stroke="#000000" stroke-opacity="0.5" stroke-width="1.1"/>')
    g.append(f'<path d="M4 {1.4 * scale} H{n(w - 4)}" stroke="#ffffff" stroke-opacity="0.5" stroke-width="{1.2 * scale}"/>')
    g.append(f'<path d="M4 {n(h - 1.4 * scale)} H{n(w - 4)}" stroke="#000000" stroke-opacity="0.45" stroke-width="{1.4 * scale}"/>')
    sy = h * 0.5 - 1.1 * scale
    g.append(f'<rect x="{5 * scale}" y="{n(sy)}" width="{n(w - 10 * scale)}" height="{2.2 * scale}" fill="{seam}" opacity="0.92"/>')
    g.append(
        f'<g style="mix-blend-mode:screen" filter="url(#f5)">'
        f'<rect x="{5 * scale}" y="{n(sy)}" width="{n(w - 10 * scale)}" height="{2.6 * scale}" fill="{glow}" opacity="0.85"/></g>'
    )
    for bx in (7.5 * scale, w - 7.5 * scale):
        g.append(f'<circle cx="{n(bx)}" cy="{n(h * 0.26)}" r="{1.5 * scale}" fill="#0b0f14" fill-opacity="0.7"/>')
        g.append(f'<circle cx="{n(bx)}" cy="{n(h * 0.26 - 0.5)}" r="{1.1 * scale}" fill="#ffffff" fill-opacity="0.32"/>')
    for i in range(hits):
        g.append(
            f'<rect x="{n(w * 0.5 - (hits * 3.4) / 2 + i * 3.4)}" y="{n(h - 5.4 * scale)}" '
            f'width="2.2" height="2.2" fill="{seam}" opacity="0.7"/>'
        )
    if dmg:
        g.append(
            f'<path d="M{n(w * 0.18)} {n(h * 0.12)} L{n(w * 0.34)} {n(h * 0.5)} '
            f'L{n(w * 0.22)} {n(h * 0.86)} M{n(w * 0.62)} {n(h * 0.1)} L{n(w * 0.52)} '
            f'{n(h * 0.44)} L{n(w * 0.72)} {n(h * 0.78)}" fill="none" stroke="#04070a" '
            f'stroke-opacity="0.72" stroke-width="{1.5 * scale}" stroke-linecap="round"/>'
        )
        g.append(
            f'<path d="M{n(w * 0.18 + 1)} {n(h * 0.12)} L{n(w * 0.34 + 1)} {n(h * 0.5)} '
            f'L{n(w * 0.22 + 1)} {n(h * 0.86)}" fill="none" stroke="{glow}" stroke-opacity="0.5" '
            f'stroke-width="{1 * scale}"/>'
        )
    g.append("</g>")
    (out if out is not None else ART).extend(g)


def ball(cx, cy, r, state="steel", out=None):
    o = out if out is not None else ART
    glow = {"steel": ICE, "heated": AMBER, "charged": VIOLET}[state]
    if state != "steel":
        o.append(
            f'<g style="mix-blend-mode:screen" filter="url(#f10)">'
            f'<circle cx="{n(cx)}" cy="{n(cy)}" r="{n(r * 1.5)}" fill="{glow}" opacity="0.4"/></g>'
        )
    o.append(f'<circle cx="{n(cx)}" cy="{n(cy)}" r="{n(r + 0.9)}" fill="#05080c"/>')
    o.append(f'<circle cx="{n(cx)}" cy="{n(cy)}" r="{n(r)}" fill="url(#g_ball)"/>')
    if state == "steel":
        o.append(f'<circle cx="{n(cx)}" cy="{n(cy)}" r="{n(r)}" fill="url(#g_ball_rim)"/>')
        o.append(f'<circle cx="{n(cx)}" cy="{n(cy)}" r="{n(r)}" fill="url(#g_ball_cool)" opacity="0.9"/>')
    o.append(
        f'<ellipse cx="{n(cx - r * 0.1)}" cy="{n(cy + r * 0.06)}" rx="{n(r * 0.94)}" '
        f'ry="{n(r * 0.13)}" fill="#ffffff" opacity="0.1"/>'
    )
    o.append(
        f'<ellipse cx="{n(cx - r * 0.34)}" cy="{n(cy - r * 0.38)}" rx="{n(r * 0.2)}" '
        f'ry="{n(r * 0.14)}" fill="#ffffff" opacity="0.95" filter="url(#f2)"/>'
    )
    o.append(f'<circle cx="{n(cx - r * 0.36)}" cy="{n(cy - r * 0.4)}" r="{n(r * 0.075)}" fill="#ffffff"/>')
    o.append(
        f'<circle cx="{n(cx + r * 0.42)}" cy="{n(cy + r * 0.3)}" r="{n(r * 0.1)}" '
        f'fill="{glow}" opacity="0.5" filter="url(#f2)"/>'
    )
    o.append(
        f'<path d="M{cx - r} {cy} A{r} {r * 0.34} 0 0 0 {cx + r} {cy}" fill="none" '
        f'stroke="#ffffff" stroke-opacity="0.14" stroke-width="{n(r * 0.07)}"/>'
    )
    if state == "heated":
        o.append(
            f'<g style="mix-blend-mode:screen" filter="url(#f2)">'
            f'<path d="M{cx - r * 0.5} {cy - r * 0.2} L{cx - r * 0.1} {cy + r * 0.1} L{cx + r * 0.3} {cy - r * 0.35}" '
            f'stroke="{AMBER}" stroke-width="{n(r * 0.12)}" fill="none" opacity="0.9"/></g>'
        )
    if state == "charged":
        o.append(
            f'<g style="mix-blend-mode:screen"><circle cx="{n(cx)}" cy="{n(cy)}" r="{n(r * 1.16)}" '
            f'fill="none" stroke="{VIOLET}" stroke-width="1" opacity="0.7" filter="url(#f2)"/></g>'
        )


def paddle(cx, cy, pw, out=None, scale=1):
    o = out if out is not None else ART
    w, h = pw * scale, 13 * scale
    x = cx - w / 2
    o.append(
        f'<g style="mix-blend-mode:screen" filter="url(#f22)">'
        f'<rect x="{n(x)}" y="{n(cy - 9)}" width="{n(w)}" height="{n(h + 22)}" fill="{CYAN}" opacity="0.5"/></g>'
    )
    o.append(
        f'<rect x="{n(x + 15 * scale)}" y="{n(cy - 2)}" width="{n(w - 30 * scale)}" height="{n(h * 0.5)}" '
        f'rx="{n(h * 0.25)}" fill="url(#g_field)"/>'
    )
    o.append(
        f'<rect x="{n(x + 15 * scale)}" y="{n(cy - 0.6)}" width="{n(w - 30 * scale)}" height="{n(h * 0.16)}" '
        f'fill="#ffffff" opacity="0.9"/>'
    )
    for i in range(1, 7):
        lx = x + 15 * scale + (w - 30 * scale) * i / 7
        o.append(
            f'<line x1="{n(lx)}" y1="{n(cy - 2)}" x2="{n(lx)}" y2="{n(cy + h * 0.5)}" '
            f'stroke="#ffffff" stroke-opacity="0.22" stroke-width="0.8"/>'
        )
    for sx in (x, x + w - 15 * scale):
        o.append(f'<rect x="{n(sx)}" y="{n(cy - 5 * scale)}" width="{n(15 * scale)}" height="{n(h + 9 * scale)}" rx="{3 * scale}" fill="url(#g_prong)"/>')
        o.append(f'<rect x="{n(sx + 1.2)}" y="{n(cy - 3.8 * scale)}" width="{n(12.6 * scale)}" height="{n(h + 6.6 * scale)}" rx="2" fill="none" stroke="#000000" stroke-opacity="0.55"/>')
        o.append(f'<rect x="{n(sx + 4 * scale)}" y="{n(cy - 1.5 * scale)}" width="{n(7 * scale)}" height="{n(h + 2 * scale)}" rx="1.6" fill="{CYAN}" opacity="0.9"/>')
        o.append(
            f'<g style="mix-blend-mode:screen" filter="url(#f5)"><rect x="{n(sx + 4 * scale)}" '
            f'y="{n(cy - 1.5 * scale)}" width="{n(7 * scale)}" height="{n(h + 2 * scale)}" rx="1.6" fill="{CYAN}"/></g>'
        )
    o.append(f'<rect x="{n(x + 4)}" y="{n(cy + h + 3 * scale)}" width="{n(w - 8)}" height="{n(3.4 * scale)}" rx="1.6" fill="#141b24"/>')
    o.append(f'<path d="M{n(x + 8)} {n(cy + h + 3 * scale)} h{n(w - 16)}" stroke="#8fa3b8" stroke-opacity="0.4" stroke-width="0.9"/>')
    for i in range(5):
        vx = x + w * 0.28 + i * w * 0.11
        o.append(f'<rect x="{n(vx)}" y="{n(cy + h + 3.6 * scale)}" width="{n(w * 0.05)}" height="{n(2.4 * scale)}" fill="#04070a" opacity="0.8"/>')


def spark_burst(cx, cy, rad, count, color, out=None, seed=0):
    o = out if out is not None else FX
    rr = random.Random(seed)
    g = ['<g style="mix-blend-mode:screen">']
    for _ in range(count):
        a = rr.uniform(0, math.tau)
        l = rr.uniform(rad * 0.35, rad)
        w = rr.uniform(0.7, 2.1)
        x2 = cx + math.cos(a) * l
        y2 = cy + math.sin(a) * l
        g.append(
            f'<line x1="{n(cx)}" y1="{n(cy)}" x2="{n(x2)}" y2="{n(y2)}" stroke="{color}" '
            f'stroke-width="{n(w)}" stroke-linecap="round" opacity="{rr.uniform(0.35, 0.95):.2f}"/>'
        )
    for _ in range(int(count * 0.55)):
        a = rr.uniform(0, math.tau)
        d = rr.uniform(rad * 0.5, rad * 1.5)
        g.append(
            f'<circle cx="{n(cx + math.cos(a) * d)}" cy="{n(cy + math.sin(a) * d)}" '
            f'r="{rr.uniform(0.8, 2.4):.2f}" fill="#fff3e0" opacity="{rr.uniform(0.4, 1):.2f}"/>'
        )
    g.append(
        f'<circle cx="{n(cx)}" cy="{n(cy)}" r="{n(rad * 0.34)}" fill="#ffffff" opacity="0.55" filter="url(#f10)"/>'
    )
    g.append("</g>")
    o.extend(g)


def ring(cx, cy, r, color, wd=1.6, op=0.55, f="f2", out=None):
    (out if out is not None else FX).append(
        f'<g style="mix-blend-mode:screen" filter="url(#{f})"><ellipse cx="{n(cx)}" cy="{n(cy)}" '
        f'rx="{n(r)}" ry="{n(r * 0.62)}" fill="none" stroke="{color}" stroke-width="{wd}" opacity="{op}"/></g>'
    )


def badge(x, y, num, tx, ty, out=None):
    o = out if out is not None else FRAME
    o.append(
        f'<line x1="{n(x)}" y1="{n(y)}" x2="{n(tx)}" y2="{n(ty)}" stroke="{ICE}" '
        f'stroke-opacity="0.5" stroke-width="0.9" stroke-dasharray="3 3"/>'
    )
    o.append(f'<circle cx="{n(tx)}" cy="{n(ty)}" r="1.7" fill="{ICE}" opacity="0.9"/>')
    o.append(f'<circle cx="{n(x)}" cy="{n(y)}" r="10.5" fill="#05080c" fill-opacity="0.86" stroke="{ICE}" stroke-opacity="0.7"/>')
    o.append(T(x, y + 3.6, str(num), 10.5, ICE, 700, "middle", 0))


ART.append(f'<rect x="0" y="0" width="{HW}" height="{HH}" fill="{PANEL}"/>')
ART.append(f'<rect width="{HW}" height="{HH}" fill="url(#g_sky)"/>')
ART.append(f'<rect width="{HW}" height="{HH}" fill="url(#g_core)"/>')
ART.append('<g style="mix-blend-mode:screen" opacity="0.5"><rect width="1024" height="576" filter="url(#f_neb)"/></g>')
ART.append(
    f'<rect width="{HW}" height="{HH}" fill="{ICE}" opacity="0.05" filter="url(#f_dust)"/>'
)
ART.append('<g style="mix-blend-mode:screen" opacity="0.5"><rect width="1024" height="576" filter="url(#f_dust)"/></g>')

for i, (sx, sy, sr) in enumerate([(512, 250, 210), (512, 250, 150), (795, 150, 96)]):
    ART.append(
        f'<circle cx="{sx}" cy="{sy}" r="{sr}" fill="none" stroke="#2a3d52" stroke-opacity="{0.5 - i * 0.1}" stroke-width="{2.4 - i * 0.5}"/>'
    )
ART.append(
    '<circle cx="512" cy="250" r="118" fill="none" stroke="#33506b" stroke-opacity="0.35" stroke-width="1" stroke-dasharray="2 8"/>'
)

for tx, flip in ((58, 1), (908, -1)):
    ART.append(f'<g transform="translate({tx},0)">')
    ART.append(f'<rect x="0" y="-10" width="58" height="600" fill="url(#g_truss)"/>')
    ART.append(f'<rect x="0" y="-10" width="58" height="600" fill="url(#p_grid)" opacity="0.5"/>')
    for i in range(13):
        yy = -10 + i * 46
        d = f"M0 {yy} L58 {yy + 46} M58 {yy} L0 {yy + 46}"
        ART.append(f'<path d="{d}" stroke="#22303f" stroke-width="2.4" fill="none" opacity="0.85"/>')
    ART.append(f'<rect x="0" y="-10" width="3" height="600" fill="{"#7fa6c9" if flip > 0 else "#5c748c"}" opacity="0.5"/>')
    ART.append(f'<rect x="55" y="-10" width="3" height="600" fill="#0a0e14"/>')
    ART.append("</g>")

ART.append('<rect x="-20" y="18" width="1064" height="26" fill="url(#g_truss)"/>')
ART.append('<path d="M-20 44 H1044" stroke="#7fa6c9" stroke-opacity="0.35" stroke-width="1.2"/>')
for i in range(9):
    lx = 40 + i * 118
    ART.append(f'<line x1="{lx}" y1="44" x2="{lx + (i % 3) * 6 - 8}" y2="{96 + (i % 4) * 22}" stroke="#0d141d" stroke-width="1.4"/>')
for i in range(7):
    ART.append(
        f'<circle cx="{96 + i * 140}" cy="31" r="2.6" fill="{AMBER}" opacity="0.9"/>'
        f'<circle cx="{96 + i * 140}" cy="31" r="6" fill="{AMBER}" opacity="0.28" filter="url(#f5)"/>'
    )

ART.append(
    '<g style="mix-blend-mode:screen"><path d="M-40 -10 L250 -10 L640 576 L120 576 Z" fill="url(#g_shaft_c)" opacity="0.5" filter="url(#f40)"/>'
    '<path d="M1064 586 L820 586 L470 120 L760 60 Z" fill="url(#g_shaft_a)" opacity="0.42" filter="url(#f40)"/></g>'
)

ART.append(f'<rect x="0" y="528" width="{HW}" height="{HH - 528}" fill="url(#g_deck)"/>')
ART.append(f'<path d="M0 528 H{HW}" stroke="#8fb4d6" stroke-opacity="0.3" stroke-width="1"/>')
for i in range(9):
    yy = 528 + (i * i * 0.72) + i * 1.4
    if yy > HH:
        break
    ART.append(f'<line x1="0" y1="{n(yy)}" x2="{HW}" y2="{n(yy)}" stroke="#22303f" stroke-opacity="{0.75 - i * 0.07:.2f}" stroke-width="0.9"/>')
for i in range(-6, 25):
    xt = 512 + i * 46
    xb = 512 + i * 92
    ART.append(f'<line x1="{xt}" y1="528" x2="{xb}" y2="{HH}" stroke="#1d2937" stroke-opacity="0.6" stroke-width="0.9"/>')

CLUSTER = {
    "L": (0, 5, 132, 3),
    "C": (5, 9, 106, 4),
    "R": (9, 13, 136, 3),
}
CLASSMAP = {
    "L": ["ceramic", "titanium", "tungsten"],
    "C": ["titanium", "tungsten", "titanium", "ceramic"],
    "R": ["ceramic", "titanium", "tungsten"],
}
GONE = {("L", 4, 2), ("L", 3, 2), ("C", 5, 3), ("R", 9, 2), ("R", 10, 2), ("R", 12, 0)}
CRACKED = {("L", 2, 2), ("C", 6, 3), ("R", 11, 2)}

for name, (c0, c1, y0, rows) in CLUSTER.items():
    bx0 = X0 + c0 * GX
    bx1 = X0 + (c1 - 1) * GX + BW
    by1 = y0 + (rows - 1) * GY + BH
    ART.append(
        f'<g style="mix-blend-mode:screen" opacity="0.6"><path d="M{bx0 + 6} {by1} L{bx1 - 6} {by1} '
        f'L{bx1 - 40} {min(HH, by1 + 150)} L{bx0 + 40} {min(HH, by1 + 150)} Z" fill="url(#g_beam)" filter="url(#f22)"/></g>'
    )
    for col in range(c0, c1):
        for row in range(rows):
            if (name, col, row) in GONE:
                continue
            key = CLASSMAP[name][row]
            if name == "C" and row == 2 and col in (6, 7):
                key = "reactor"
            dmg = 1 if (name, col, row) in CRACKED else 0
            brick(X0 + col * GX, y0 + row * GY, key, dmg)
    ART.append(
        f'<rect x="{bx0 - 7}" y="{y0 - 8}" width="{bx1 - bx0 + 14}" height="{by1 - y0 + 16}" rx="7" '
        f'fill="none" stroke="#7fa6c9" stroke-opacity="0.16" stroke-width="1"/>'
    )

for gx, gy, tag in [(X0 + 3 * GX, 132 + 2 * GY, "L"), (X0 + 10 * GX, 136 + 2 * GY, "R"), (X0 + 5 * GX, 106 + 3 * GY, "C")]:
    for i in range(7):
        a = R.uniform(0, math.tau)
        d = R.uniform(4, 34)
        s = R.uniform(1.4, 4.2)
        ART.append(
            f'<g transform="translate({n(gx + BW / 2 + math.cos(a) * d)},{n(gy + BH / 2 + math.sin(a) * d)}) '
            f'rotate({R.uniform(0, 360):.1f})"><rect x="{-s / 2}" y="{-s / 2}" width="{s}" height="{s * 0.6}" '
            f'rx="0.8" fill="#2b3a4a" stroke="#8fb4d6" stroke-opacity="0.35" stroke-width="0.5"/></g>'
        )

IMPX, IMPY = X0 + 10 * GX + BW / 2, 136 + 2 * GY + BH / 2
spark_burst(IMPX, IMPY, 62, 26, "#ffd9a8", seed=11)
ring(IMPX, IMPY, 40, AMBER, 1.8, 0.5)
ring(IMPX, IMPY, 66, "#ffe0b0", 1, 0.28, "f5")

BCX, BCY, BR = 336, 462, 27
pts = [(792, 176), (560, 232), (400, 350), (BCX, BCY)]
ART.append('<g style="mix-blend-mode:screen">')
ART.append(
    f'<path d="M{pts[0][0]} {pts[0][1] - 5} C{pts[1][0]} {pts[1][1] - 9} {pts[2][0]} {pts[2][1] - 15} {pts[3][0]} {pts[3][1] - BR * 0.5} '
    f'L{pts[3][0]} {pts[3][1] + BR * 0.5} C{pts[2][0]} {pts[2][1] + 15} {pts[1][0]} {pts[1][1] + 9} {pts[0][0]} {pts[0][1] + 5} Z" '
    'fill="url(#g_streak)" opacity="0.4" filter="url(#f10)"/>'
)
ART.append(
    f'<path d="M{pts[0][0]} {pts[0][1] - 2} C{pts[1][0]} {pts[1][1] - 4} {pts[2][0]} {pts[2][1] - 7} {pts[3][0]} {pts[3][1] - BR * 0.3} '
    f'L{pts[3][0]} {pts[3][1] + BR * 0.3} C{pts[2][0]} {pts[2][1] + 7} {pts[1][0]} {pts[1][1] + 4} {pts[0][0]} {pts[0][1] + 2} Z" '
    'fill="url(#g_streak)" opacity="0.62"/>'
)
ART.append("</g>")
for i in range(16):
    t = i / 16
    px = pts[0][0] + (pts[3][0] - pts[0][0]) * t * t * 0.9 + t * 20
    py = pts[0][1] + (pts[3][1] - pts[0][1]) * (t ** 1.7)
    ART.append(
        f'<circle cx="{n(px + R.uniform(-9, 9))}" cy="{n(py + R.uniform(-9, 9))}" r="{R.uniform(0.7, 2):.2f}" '
        f'fill="#dff0ff" opacity="{0.15 + t * 0.5:.2f}"/>'
    )
ball(BCX, BCY, BR)
ART.append(
    f'<g style="mix-blend-mode:screen" filter="url(#f22)"><ellipse cx="{BCX}" cy="{BCY + 4}" rx="{BR * 2.1}" '
    f'ry="{BR * 1.5}" fill="{ICE}" opacity="0.22"/></g>'
)

PCX, PCY, PW = 300, 508, 152
paddle(PCX, PCY, PW)
ART.append(
    f'<g style="mix-blend-mode:screen" opacity="0.5" filter="url(#f10)">'
    f'<ellipse cx="{PCX}" cy="556" rx="{PW * 0.8}" ry="14" fill="{CYAN}"/></g>'
)
ART.append(
    f'<g style="mix-blend-mode:screen" opacity="0.28" filter="url(#f5)">'
    f'<ellipse cx="{BCX + 30}" cy="552" rx="34" ry="9" fill="{ICE}"/></g>'
)

for _ in range(90):
    x, y = R.uniform(0, HW), R.uniform(0, HH)
    r = R.uniform(0.4, 1.7)
    FX.append(
        f'<circle cx="{n(x)}" cy="{n(y)}" r="{r:.2f}" fill="{"#ffffff" if R.random() > 0.4 else AMBER}" '
        f'opacity="{R.uniform(0.06, 0.4):.2f}"/>'
    )
for _ in range(7):
    x, y, s = R.uniform(60, 960), R.uniform(60, 480), R.uniform(9, 26)
    p = " ".join(
        f"{n(x + math.cos(math.pi / 3 * i + 0.5) * s)},{n(y + math.sin(math.pi / 3 * i + 0.5) * s)}"
        for i in range(6)
    )
    FX.append(
        f'<polygon points="{p}" fill="none" stroke="{ICE}" stroke-width="1.1" '
        f'opacity="{R.uniform(0.06, 0.2):.2f}" filter="url(#f5)"/>'
    )
for _ in range(6):
    x, y = R.uniform(120, 900), R.uniform(180, 500)
    FX.append(
        f'<ellipse cx="{n(x)}" cy="{n(y)}" rx="{R.uniform(20, 46):.1f}" '
        f'ry="{R.uniform(9, 20):.1f}" fill="#7e93a8" opacity="{R.uniform(0.03, 0.08):.2f}" filter="url(#f22)"/>'
    )

OVER.append(f'<rect width="{HW}" height="{HH}" fill="url(#g_vig)"/>')
OVER.append(f'<rect width="{HW}" height="{HH}" fill="url(#p_scan)" opacity="0.34"/>')
OVER.append(
    f'<rect width="{HW}" height="{HH}" fill="#8090a0" opacity="0.05" filter="url(#f_grain)" '
    'style="mix-blend-mode:overlay"/>'
)
FRAME.append(
    f'<rect x="24" y="30" width="{HW - 48}" height="{HH - 74}" rx="4" fill="none" stroke="{CYAN}" '
    'stroke-opacity="0.22" stroke-width="1" stroke-dasharray="10 6 2 6"/>'
)
for cx_, cy_, dx, dy in [(24, 30, 1, 1), (HW - 24, 30, -1, 1), (24, HH - 44, 1, -1), (HW - 24, HH - 44, -1, -1)]:
    FRAME.append(
        f'<path d="M{cx_ + dx * 26} {cy_} H{cx_} V{cy_ + dy * 26}" fill="none" stroke="{ICE}" '
        'stroke-opacity="0.65" stroke-width="1.8"/>'
    )

OVER.append(T(44, 62, "SCORE", 9, INK3, 500, ls=2.2))
OVER.append(T(44, 88, "018 400", 22, INK, 700, ls=1.4, op=0.95))
OVER.append(T(44, 104, "x3 COMBO", 9, AMBER, 500, ls=1.4, op=0.85))
OVER.append(T(HW - 44, 62, "SECTOR 03 // DECK 7", 9.5, INK2, 500, "end", 2))
OVER.append(T(HW - 44, 80, "BREACH 41%", 9.5, CYAN, 700, "end", 1.6, 0.9))
OVER.append(f'<rect x="{HW - 184}" y="88" width="140" height="4" rx="2" fill="#16202b"/>')
OVER.append(f'<rect x="{HW - 184}" y="88" width="57" height="4" rx="2" fill="{CYAN}" opacity="0.9"/>')
for i in range(3):
    x = 44 + i * 17
    OVER.append(
        f'<path d="M{x} 546 l7 -11 l7 11 z" fill="{ICE}" opacity="{0.9 - i * 0.22}"/>'
    )
OVER.append(T(44, 566, "LIVES", 8, INK3, 500, ls=2))
OVER.append(T(HW - 44, 566, "BALL VEL 0.86c", 8.5, INK3, 400, "end", 1.4))

badge(404, 396, 1, BCX + BR + 4, BCY - 10)
badge(604, 268, 2, X0 + 6 * GX + BW / 2, 106 + 2 * GY + BH / 2)
badge(846, 274, 3, IMPX + 8, IMPY - 4)
badge(146, 560, 4, PCX - PW / 2 - 4, PCY + 6)
badge(392, 330, 5, X0 + 5 * GX + BW / 2, 106 + 3 * GY + BH + 26)
badge(872, 122, 6, 936, 40)

yc = HY + 6
heading(CX, yc + 10, "Palette / material codes")
sw = [
    ("#0a0e15", "HANGAR VOID", "backdrop / negative space"),
    ("#16202c", "STRUCTURAL STEEL", "trusses, girders, deck plates"),
    ("#a9bacd", "POLISHED BALL", "specular white + cool rim"),
    (CYAN, "EMITTER FIELD", "paddle, arena bound, HUD"),
    (ICE, "COLD KEY LIGHT", "upper-left 6500K shaft"),
    (AMBER, "WARM BOUNCE", "lower-right 2700K fill, hazards"),
    (RED, "CERAMIC BAND", "1-hit brick"),
    (VIOLET, "REACTOR CORE", "objective brick / powerup"),
]
yy = yc + 30
for hexv, nm, role in sw:
    SIDE.append(f'<rect x="{CX}" y="{yy - 9}" width="30" height="13" rx="2" fill="{hexv}"/>')
    SIDE.append(f'<rect x="{CX}" y="{yy - 9}" width="30" height="13" rx="2" fill="none" stroke="#000" stroke-opacity="0.5"/>')
    SIDE.append(T(CX + 40, yy - 0.5, nm, 9.5, INK, 600, ls=1.1))
    SIDE.append(T(CX + 40, yy + 10, role, 8.5, INK3, 400, ls=0.3, fam=MONO))
    SIDE.append(T(CX + CW, yy - 0.5, hexv.upper(), 8.5, INK2, 400, "end", 0.6))
    yy += 25

yy += 14
heading(CX, yy, "Brick classes")
yy += 20
for key in ("ceramic", "titanium", "tungsten", "reactor"):
    top, mid, bot, seam, glow, name, hits = CLASSES[key]
    SIDE.append(f'<g transform="translate({CX},{yy}) scale(1)"></g>')
    brick(CX, yy, key, 1 if key == "titanium" else 0, out=SIDE)
    behaviour = {
        "ceramic": "kinetic shatter · drops nothing",
        "titanium": "2 hits · spawns debris shard",
        "tungsten": "3 hits · needs heated ball",
        "reactor": "4 hits · breaches the sector",
    }[key]
    SIDE.append(T(CX + BW + 14, yy + 11, name, 9.5, INK, 600, ls=1.1))
    SIDE.append(T(CX + BW + 14, yy + 22, behaviour, 8.5, INK3, 400, ls=0.2))
    SIDE.append(T(CX + CW, yy + 11, f"{hits} HIT" if hits == 1 else f"{hits} HITS", 9, glow, 600, "end", 0.8))
    yy += 42

yy += 12
heading(CX, yy, "Callouts")
yy += 18
notes = [
    (1, "Steel ball reads as a sphere, not a disc: hard key specular, cool rim from the field, warm rim from the hangar lamps."),
    (2, "Reactor core brick is the level objective — its seam pulses violet and lights nearby alloy faces."),
    (3, "Cleared slot keeps drifting shards for 0.6s so destruction has weight."),
    (4, "Paddle is a magnetic emitter: two prongs, no solid surface. Field brightens on impact."),
    (5, "Rafts float on containment beams; beams double as the read for which rows are still active."),
    (6, "Far truss + exhaust rings are parallax layers at 0.3x scroll, kept under 12% contrast."),
]
for num, text in notes:
    SIDE.append(f'<circle cx="{CX + 8}" cy="{yy - 3.5}" r="8" fill="none" stroke="{ICE}" stroke-opacity="0.6"/>')
    SIDE.append(T(CX + 8, yy - 0.4, str(num), 9, ICE, 700, "middle", 0))
    lines = wrap(text, 62)
    for i, ln in enumerate(lines):
        SIDE.append(T(CX + 24, yy, ln, 9, INK2 if i else INK, 400 if i else 500, ls=0.2))
        yy += 12
    yy += 8

yy += 4
heading(CX, yy, "Lighting & fx budget")
yy += 18
for ln in [
    "key  cold 6500K shaft, upper-left, 45deg",
    "fill warm 2700K bounce, lower-right, 0.4x key",
    "glow  3 additive passes: r2 / r5 / r22",
    "fx   sparks, embers, dust, hex bokeh, shards",
    "post  scanline 0.34 + grain 0.05 + vignette",
]:
    SIDE.append(T(CX, yy, ln.split(" ")[0].upper(), 8.5, CYAN, 600, ls=1))
    SIDE.append(T(CX + 40, yy, " ".join(ln.split(" ")[1:]), 9, INK2, 400, ls=0.2))
    yy += 14

shead(32, SY + 10, "Ball material & states", 232)
for i, (st, lab) in enumerate([("steel", "POLISHED"), ("heated", "HEATED"), ("charged", "CHARGED")]):
    bx = 62 + i * 78
    ball(bx, SY + 74, 21, st, out=STRIP)
    STRIP.append(T(bx, SY + 112, lab, 8, INK2, 600, "middle", 1))
STRIP.append(T(32, SY + 140, "single albedo map + 2 rim", 8.5, INK3, ls=0.2))
STRIP.append(T(32, SY + 152, "gradients · no animation frames", 8.5, INK3, ls=0.2))
STRIP.append(T(32, SY + 172, "STATE DRIVES TRAIL: none / ember", 8.5, INK2, ls=0.2))
STRIP.append(T(32, SY + 184, "chain / violet arc lattice", 8.5, INK2, ls=0.2))

shead(296, SY + 10, "Paddle // magnetic emitter", 232)
paddle(296 + 116, SY + 76, 168, out=STRIP, scale=1.15)
for ly, lab in [(SY + 44, "EMITTER PRONG · COLD CATHODE TIP"), (SY + 118, "FIELD CORE · STRETCHES ON IMPACT"), (SY + 132, "CHASSIS · HYDRAULIC VENT ROW")]:
    STRIP.append(T(296, ly, lab, 8.5, INK2, ls=0.4))
STRIP.append(T(296, SY + 160, "hit feedback: field widens 1.2x,", 8.5, INK3, ls=0.2))
STRIP.append(T(296, SY + 172, "prong glow spikes 90ms, deck", 8.5, INK3, ls=0.2))
STRIP.append(T(296, SY + 184, "pool brightens with ball speed", 8.5, INK3, ls=0.2))

shead(560, SY + 10, "Brick damage model", 232)
stages = [("intact", 0), ("cracked", 1), ("shatter", 2)]
for i, (lab, dmg) in enumerate(stages):
    bx = 560 + i * 78
    if dmg == 2:
        for j in range(6):
            s = 5 + (j % 3) * 3
            ang = 0.9 + j * 1.05
            STRIP.append(
                f'<g transform="translate({560 + i * 78 + 26 + math.cos(ang) * 13},{SY + 66 + math.sin(ang) * 11}) '
                f'rotate({j * 47})"><path d="M{-s} 0 L0 {-s} L{s} 0 L0 {s * 0.7} Z" fill="#9fb5ca" '
                'stroke="#e6f2ff" stroke-opacity="0.4" stroke-width="0.6"/></g>'
            )
        STRIP.append(
            f'<g style="mix-blend-mode:screen" filter="url(#f5)"><circle cx="{bx + 26}" cy="{SY + 66}" r="16" fill="{ICE}" opacity="0.5"/></g>'
        )
    else:
        brick(bx, SY + 54, "titanium", dmg, out=STRIP, scale=0.86)
    STRIP.append(T(bx + 26, SY + 100, lab.upper(), 8, INK2, 600, "middle", 1))
    if i < 2:
        STRIP.append(f'<path d="M{bx + 62} {SY + 66} h10 m-4 -4 l4 4 l-4 4" stroke="{INK3}" fill="none" stroke-width="1"/>')
STRIP.append(T(560, SY + 128, "seam emissivity rises per hit; the", 8.5, INK3, ls=0.2))
STRIP.append(T(560, SY + 140, "crack pass is a 2px dark line with", 8.5, INK3, ls=0.2))
STRIP.append(T(560, SY + 152, "a lit offset edge, never a redraw", 8.5, INK3, ls=0.2))
STRIP.append(T(560, SY + 176, "SHARDS: 4-7 quads, 0.6s, gravity", 8.5, INK2, ls=0.3))

shead(816, SY + 10, "FX + HUD atoms", 240)
spark_burst(872, SY + 56, 30, 14, "#ffd9a8", out=STRIP, seed=5)
ring(872, SY + 56, 22, AMBER, 1.2, 0.5, out=STRIP)
STRIP.append(T(816, SY + 104, "IMPACT BURST", 8, INK2, 600, ls=1))
for i in range(5):
    x = 986 + (i % 3) * 26
    y = SY + 44 + (i // 3) * 26
    STRIP.append(f'<circle cx="{x}" cy="{y}" r="{2 + i * 0.5}" fill="#fff3e0" opacity="{0.9 - i * 0.12:.2f}"/>')
    STRIP.append(f'<circle cx="{x}" cy="{y}" r="{5 + i}" fill="{AMBER}" opacity="0.2" filter="url(#f5)"/>')
STRIP.append(T(986, SY + 104, "EMBERS", 8, INK2, 600, ls=1))
STRIP.append(T(816, SY + 132, "HUD ATOMS", 8, CYAN, 600, ls=1.4))
STRIP.append(T(816, SY + 152, "018 400", 17, INK, 700, ls=1.2))
STRIP.append(T(816, SY + 166, "SCORE · TABULAR 700", 7.5, INK3, ls=1))
for i in range(2):
    STRIP.append(f'<path d="M{944 + i * 15} {SY + 152} l6 -9 l6 9 z" fill="{ICE}" opacity="{0.9 - i * 0.3}"/>')
STRIP.append(T(944, SY + 166, "LIFE PIP", 7.5, INK3, ls=1))
STRIP.append(f'<rect x="{1010}" y="{SY + 143}" width="46" height="4" rx="2" fill="#16202b"/>')
STRIP.append(f'<rect x="{1010}" y="{SY + 143}" width="19" height="4" rx="2" fill="{CYAN}"/>')
STRIP.append(T(1010, SY + 166, "BREACH", 7.5, INK3, ls=1))
STRIP.append(T(816, SY + 190, "type: mono, uppercase, 2px tracking", 8.5, INK3, ls=0.2))
STRIP.append(T(816, SY + 202, "opacity ceiling 0.95 · never over art", 8.5, INK3, ls=0.2))

svg = [
    f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">',
    f"<defs>{''.join(DEFS)}</defs>",
    f'<rect width="{W}" height="{H}" fill="{SHEET}"/>',
    f'<rect x="{HX - 8}" y="{HY - 8}" width="{HW + 16}" height="{HH + 16}" rx="3" fill="{PANEL}" stroke="{LINE}"/>',
    f'<rect x="{CX - 16}" y="{HY - 8}" width="{CW + 24}" height="{H - HY - 24}" rx="3" fill="none" stroke="{LINE}"/>',
    f'<rect x="{HX - 8}" y="{SY - 16}" width="{HW + 16}" height="{SH + 24}" rx="3" fill="none" stroke="{LINE}"/>',
    T(HX, 28, "STEELBREAK", 15, INK, 700, ls=3.4),
    T(HX + 138, 28, "// 2D BREAKOUT · CONCEPT KEY ART", 10.5, INK3, 500, ls=1.6),
    T(HX + HW, 28, "REV 0.1 · SCI-FI INDUSTRIAL · 16:9 PLAYFIELD", 9.5, INK3, 400, "end", 1.4),
    f'<line x1="{HX}" y1="34" x2="{HX + HW}" y2="34" stroke="{LINE2}"/>',
    f'<line x1="{HX + HW - 120}" y1="34" x2="{HX + HW}" y2="34" stroke="{CYAN}" stroke-width="1.6"/>',
    f'<g transform="translate({HX},{HY})" clip-path="url(#c_hero)" style="isolation:isolate">',
    "".join(ART),
    "".join(FX),
    "".join(OVER),
    "".join(FRAME),
    "</g>",
    "".join(STRIP),
    "".join(SIDE),
    T(HX, H - 12, "OPENCODE CONCEPT · SINGLE-SCREEN SHEET", 8, INK3, ls=1.2),
    T(HX + HW, H - 12, "01", 9, INK2, 600, "end", 1.4),
    "</svg>"
]

for mx, my in [(12, 12), (W - 12, 12), (12, H - 12), (W - 12, H - 12)]:
    dx = 1 if mx < W / 2 else -1
    dy = 1 if my < H / 2 else -1
    svg.insert(
        -3,
        f'<path d="M{mx + dx * 14} {my} H{mx} V{my + dy * 14}" fill="none" stroke="{LINE2}" stroke-width="1.2"/>',
    )

open("art/steelbreak_concept.svg", "w").write("\n".join(svg))

hero = [
    f'<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="0 0 {HW} {HH}">',
    f"<defs>{''.join(DEFS)}</defs>",
    f'<g style="isolation:isolate">{"".join(ART)}{"".join(FX)}{"".join(OVER)}</g>',
    "</svg>",
]
open("art/steelbreak_keyart.svg", "w").write("\n".join(hero))
print("wrote sheet", sum(len(x) for x in svg), "bytes; keyart", sum(len(x) for x in hero), "bytes")
