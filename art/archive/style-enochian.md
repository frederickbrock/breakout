# Art style guide — Enochian painterly (ARCHIVED 2026-09-27, superseded by Steelbreak in art/style.md)

Canonical look for all `art` work in this repo. Owned by the art-director; changed
only with the user. First established for `enoch-keyart` (realm I, the Watchers'
camp). Supersedes nothing: the earlier `steelbreak` sci-fi-industrial direction is
kept in `art/` as a separate, abandoned lane.

## Look

Late-Romantic oil painting in the Dürer-to-Doré register: colossal winged Watchers
and horned Nephilim titans ranged against tiny mortal silhouettes, seen across a
valley or a sea of cloud. Scale is the subject — nothing reads as "epic" until
something reads as small. Heavy atmospheric perspective collapses the distance into
blue-grey haze, and a single celestial source cuts divine gold through an otherwise
muted, earthen frame. Matte, dry-brushed, cracked-varnish surface. No neon, no
chromatic bloom, no glossy game-art specular.

The mechanic this serves: bricks are the titan ranks, the paddle is an ancient warship,
clearing a level translocates the ship to the next realm where the battle still rages.
So every realm image is a *battle in progress*, never a ruin after one.

## Palette

Hexes sampled from the reference set by median-cut quantisation, not eyeballed.

| role | hex | usage |
|---|---|---|
| void | `#0d2334` `#191919` `#2a323c` | deep sky, canyon shadow, negative space for HUD |
| cold stone / teal | `#315560` `#516b67` `#788677` | mid-distance rock, water realm, aerial haze |
| warm umber / earth | `#3f3133` `#654842` `#7b634c` | foreground ground, timber, hide, leather |
| divine gold | `#f0d291` `#cd945b` `#ecc39d` | halo, cloud-break, the ball, fire realm |
| bone / pale cloud | `#d8e4be` `#ccd3c9` `#dbcbaf` | high sky, mist, wind realm, rim light |
| fire accent | `#a46f51` `#865443` `#5a342c` | ember, forge-glow, blood, the burning host |

### Element → brick class

Four elements, four distinct silhouettes (shape carries the read, not hue alone):

| realm | element | brick read | dominant | accent |
|---|---|---|---|---|
| I — Watchers' Camp | **earth** | cracked basalt slab, ochre dust | `#3f3133` `#654842` | `#cd945b` |
| II — Drowned Valley | **water** | wet teal slate, flowing caustic seams | `#315560` `#516b67` | `#d8e4be` |
| III — High Sky | **wind** | bone-white cloud, translucent, torn edges | `#d8e4be` `#ccd3c9` | `#f0d291` |
| IV — Burning Host | **fire** | ember gold, licking flame, dark core | `#f0d291` `#a46f51` | `#5a342c` |
| V — The Grigori *(optional finale)* | abyss | corrupted violet, the only break from the rule | `#3e304c` `#52476a` | `#9b839e` |

Realm V is deliberately the one saturated departure in the whole game. Earn it by
keeping I–IV strictly muted.

## Resolution & scale

Smooth painterly, not pixel art. Base playfield 16:9. Brick tiles author at 96×48 and
export at 2× (192×96) so the brush texture survives scaling; the ship exports at 256×96.
Concept key art renders 1920×1080; annotated sheets 1600×900.

Text never enters the artwork. Score, lives and realm name are Bevy `Text2d` overlays
in a mono face at 2px tracking — painted lettering cannot be re-skinned or localised.

## Prompt preamble

Verbatim block prepended to **every** prompt in this project. `gen` records the full
composed prompt in each round's `meta.json`; the canonical text lives here.

```
Late-Romantic oil painting, Dürer-to-Doré register: colossal winged Watchers and horned
Nephilim titans ranged against tiny mortal silhouettes. Atmospheric perspective with
heavy aerial haze; deep slate-blue void #0d2334 receding to umber foreground #3f3133 and
#7b634c; divine gold light #f0d291 breaking from a single celestial source, cold
teal-sage midtones #315560 and #516b67, bone-white cloud #d8e4be. Visible dry-brush
texture, cracked varnish matte, muted saturation, no neon, no chromatic bloom. Epic
vertical scale, low horizon, figures read as silhouettes first. 16:9 widescreen
cinematic frame. Absolutely no text, lettering, logos, watermarks, borders, signatures
or UI overlays.
```

For **sprite** prompts (`stage:produce`), append the isolation rule instead of scenery:

```
Single isolated object, centred, whole object inside frame, orthographic front view,
plain flat solid #FF00FF magenta background, no shadow on the background, no scenery,
no other objects, no text.
```

Never mix the two regimes: key art wants full scenery, sprites want a flat key colour
so `artgen process --bg key` can cut them.

## Do / Don't

**Do**
- Put something small in frame for scale, always, and put it low.
- Keep one light source per realm and let it be visible in the composition.
- Read as silhouette first: a value-thumbnail of the piece should still parse.
- Leave clean dark space in the top corners for HUD.
- Make bricks rectangular blocks even when the fiction is a titan — the model must be
  free to paint a rank of them, not organic blobs that cannot tile.

**Don't**
- No neon, no bloom, no lens flare, no glossy specular, no subsurface-scatter skin.
- No violet or magenta outside realm V.
- No legible faces at gameplay scale — they read as noise and invite zooming.
- No text, sigils, or watermark-like marks anywhere.
- No modern materials (glass, plastic, chrome, concrete).

## Reference licensing

References live in `art/enoch_concept/reference/`, kept separate from the generated
art in `art/enoch_concept/`. The stock previews and one purchased still there are
private mood references only: **never** shipped in-game, published, or redistributed.
Generated work derives from them; it does not contain them. Crops made to strip agency
footers/badges (`reference/ref_crop_*.png`) exist so the model is not asked to reproduce
a watermark — they remain derivative references under the same restriction.
