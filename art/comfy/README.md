# ComfyUI workflows for artgen

The art pipeline can generate concepts on the local ComfyUI service (the
`comfyui` systemd user service, `http://127.0.0.1:8188`) instead of OpenRouter.
The **pipeline is the workflow file**: checkpoint, LoRA and strength, sampler,
steps, ControlNet type and strength all live in the JSON here. artgen only
fills in a few nodes, so you can change the look without touching code.

| File | Use |
|---|---|
| `sprite.json` | Sprites: SDXL base + fp16-fix VAE, LoRA `sdxl-boldline` 0.8, ControlNet-Union promax (canny/lineart type, strength 0.8, steps 0–80%) fed by the asset-shape silhouette, 30 steps, cfg 6.5, dpmpp_2m karras |

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
| `artgen:output` | SaveImage | the image artgen downloads | |

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
