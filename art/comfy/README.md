# ComfyUI workflows for artgen

The art pipeline can generate concepts on the local ComfyUI service (the
`comfyui` systemd user service, `http://127.0.0.1:8188`) instead of OpenRouter.
The **pipeline is the workflow file**: checkpoint, LoRA and strength, sampler,
steps, ControlNet type and strength all live in the JSON here. artgen only
fills in a few nodes, so you can change the look without touching code.

| File | Use |
|---|---|
| `sprite.json` | Sprites: SDXL base + fp16-fix VAE, LoRA `stlbrk_graffiti_v1-step00001500` 0.8 (the project's own graffiti LoRA, sim-7bu.4; recipe in `art/lora/graffiti-v1/`), ControlNet-Union promax (canny/lineart type, strength 0.8, steps 0–80%) fed by the asset-shape silhouette, 50 steps, cfg 10 (sim-7bu.1 sweep, user pick), dpmpp_2m karras |
| `transparent.json` | Transparent sprites: the `sprite.json` pipeline, then ComfyUI's **built-in** background removal (BiRefNet: `LoadBackgroundRemovalModel` → `RemoveBackground` → `InvertMask` → `JoinImageWithAlpha`) and an RGBA `SaveImage`. No keying, so `artgen process --bg none --trim --size WxH` |
| `tile_bg.json` | Seamless tiling backgrounds and parallax layers: a private SDXL instance (`unCLIPCheckpointLoader`) + the graffiti LoRA (step 1500 @ 0.8), with its model and VAE made circular (`SeamlessTile`, `MakeCircularVAE`; seamless-tiling pack). No ControlNet; `--size WxH` for wide layers |
| `ref_style.json` | Keep a picked look: `sprite.json` plus IP-Adapter plus SDXL (`style transfer`, weight 0.8) fed by `--ref <png>` through `artgen:ref`. The silhouette still sets the shape |

`.claude/workflow.yaml` → `art.comfy.workflow` says which file the
concept-artist uses.

## The slots

artgen finds nodes by their **title** and changes only these inputs. Every
graph must keep the titled nodes (right-click a node → *Title*):

| Title | Node | artgen sets | |
|---|---|---|---|
| `artgen:prompt` | CLIPTextEncode | `text` ← the prompt file | required |
| `artgen:seed` | KSampler | `seed` (one per variation) | required |
| `artgen:negative` | CLIPTextEncode | `text` ← `--negative-file` | |
| `artgen:size` | EmptyLatentImage | `width`/`height` ← `--size`, or the SDXL bucket for `--control-size` | |
| `artgen:control` | LoadImage | `image` ← the uploaded control PNG (`--control-size` silhouette or `--control`) | |
| `artgen:ref` | LoadImage | `image` ← the uploaded `--ref` image (one only) | |
| `artgen:output` | SaveImage | the image artgen downloads | |

Slots per graph: `sprite.json` and `transparent.json` have all of the
above except `artgen:ref`. `ref_style.json` adds `artgen:ref` (a LoadImage
for the `--ref` image, required for that graph). `tile_bg.json` has prompt,
negative, seed, size and output: no control, so don't pass
`--control-size`.

`artgen comfy-info --workflow art/comfy/sprite.json` lists the slots a file
has. A missing required slot, or a flag for a slot the graph lacks, fails
with an error that names the slot.

## Editing a workflow

1. In ComfyUI (`http://127.0.0.1:8188`): *Workflow → Open*, or drag
   `sprite.json` onto the canvas. The `artgen:control` LoadImage points at
   ComfyUI's bundled `example.png` so the graph also runs on its own there.
2. Change what you like: LoRA or strength, steps, cfg, ControlNet type or
   strength, extra nodes. Keep the `artgen:*` titles on the slot nodes.
3. *Workflow → Export (API)* and save over the file (or as a new file, then
   point `art.comfy.workflow` at it). The plain *Save* format (with `"nodes"`
   and `"links"`) is rejected: artgen needs the **API** export.
4. Check it with `artgen comfy-info --workflow <file>`, then run a round.

Each round's `meta.json` keeps the full filled-in graph, the seeds and the
workflow path, so any concept can be regenerated.

## Running a round by hand

```bash
ARTGEN="python3 ~/.claude/skills/art-pipeline/scripts/artgen.py"
$ARTGEN gen --backend comfy --workflow art/comfy/sprite.json --issue <id> --n 4 \
  --prompt-file prompt.txt --negative-file negative.txt --control-size 160x60
```

`--control-size` is the asset contract's size (e.g. a 160×60 brick or a
240×40 paddle). It draws a rounded-rect silhouette at that aspect ratio, which
keeps the object to that shape. Comfy rounds cost $0, count toward
`max_rounds` and not toward `budget_usd`.

## Which graph when

- **A sprite with a transparent background:** `transparent.json` with
  `--control-size` from the contract. BiRefNet is good at outlines, but it
  can punch holes where an inner panel matches the background colour. Use
  the style guide's dark-grey background for light objects, and check each
  pick on the sheet.
- **A tiling background or parallax layer:** `tile_bg.json` with `--size`
  (e.g. `1536x640` for a wide layer). Check it tiled 2×2. How it stays safe:
  the seamless-tiling pack's copy modes break on ComfyUI 0.38, so it patches
  conv padding in place. It does so on a **private SDXL instance**: the graph
  loads the checkpoint with `unCLIPCheckpointLoader`, a loader class no other
  graph uses, and decodes with that checkpoint's own VAE. ComfyUI's node cache
  keys on the loader class and its inputs, so the `CheckpointLoaderSimple` and
  `VAELoader` objects the other graphs share are never touched. Verified: a
  fixed-seed `sprite.json` round gives a bit-identical image before and after
  a `tile_bg --keep-loaded` round. Keep the separate loader (and no
  `VAELoader`) when editing this graph. With `--keep-loaded` both SDXL
  instances stay in RAM.
- **Variants that keep an existing look** (a picked concept or a shipped
  plate): `ref_style.json --ref <png>` plus `--control-size`. Peak VRAM with
  IP-Adapter and ControlNet is about 11.5 GB of 12. Keep other GPU work off
  while it runs.
  artgen prepares the reference before upload (`--ref-prep`, default from
  `art.comfy.ref_prep`, else `auto`). `auto` floods a `#00FF00`/`#FF00FF` key
  background to plain light grey (`#d0d0d0`), so the key doesn't bleed into
  every image (sim-2vw). Any other reference is sent unchanged. `grey` does
  the same and then makes the reference greyscale. `none` sends it as-is.
  `round-N/ref.png` is the prepared image.
- **Anything else:** `sprite.json`.

Installed models, node packs and their hashes and licences:
`~/Projects/ComfyUI/models/MODELS.md` (outside git).

The graphs load SDXL base. sim-7bu.1 A/B'd base against Animagine XL 4.0-Opt and
Proteus v0.4 on the same seeds (contact sheet `/visual/sim-7bu.1/`). Base with the
style guide's sprite block kept the graffiti keylines best. Animagine draws clean
illustrated game bricks but only with booru tags in its own order (no humans, safe,
…, quality tags last), Euler a, 28 steps and cfg 5, and without the graffiti look.
Proteus goes painterly or photoreal. Both stay installed for other uses.

All four graphs load the project's own LoRA `stlbrk_graffiti_v1-step00001500.safetensors`
at 0.8 (model and clip) in place of `sdxl-boldline`. The user picked it from the sim-7bu.4
A/B (`/visual/sim-7bu.4/`, stage 1 = steps 1000/1500/2000 × strength 0.6/0.8/1.0;
stage 2 = ± IP-Adapter ref, ± boldline/lineart-style-xl stacked). The winner was
**step 1500 @ 0.8 with the IP-Adapter reference** (r43–45). Prompts start with the
trigger `stlbrk_graffiti` (style.md). `sdxl-boldline` and `lineart-style-xl` stay
installed but aren't loaded by default. The recipe is in `art/lora/graffiti-v1/`.
