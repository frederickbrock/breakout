# stlbrk_graffiti_v1: Steelbreak graffiti line-art LoRA (SDXL)

The user trained this LoRA locally on 2026-10-05 from the project's own
graffiti-era concept picks. sim-7bu.4 records it here and A/Bs it.
This folder holds the recipe only: no images and no weights.

| File | What |
|---|---|
| `dataset.toml` | the 38 training images: dataset file name, the concept pick it came from (`art/concepts/…`, git-ignored, main checkout), caption |
| `build_dataset.py` | builds the dataset from those picks: keys chroma/white backgrounds onto the void colour, crops tight, centres, writes the captions |
| `train.sh` | the exact kohya training command |
| `sample_prompts.txt` | the prompts kohya sampled every 500 steps |

## Training

- **Trainer:** kohya [sd-scripts](https://github.com/kohya-ss/sd-scripts) at commit
  `690ea7f96c23182352ec63def76d431c6120bd2f`, in `~/Projects/lora/` (`.venv-kohya`).
- **Base:** SDXL base 1.0 (`sd_xl_base_1.0.safetensors`) with the `sdxl_vae_fp16_fix` VAE.
- **Data:** 38 images × 10 repeats (`dataset/10_stlbrk/`) = 380 steps per epoch, buckets
  512–1536 in steps of 64, latents cached.
- **Captions:** trigger token `stlbrk_graffiti` first, then a plain content description.
  `--shuffle_caption --keep_tokens=1` keeps the trigger first.
- **Network:** LoRA rank 16, alpha 16.
- **Optimiser:** AdamW8bit; unet lr 1e-4, text-encoder lr 3e-5, cosine with restarts (1 cycle),
  100 warm-up steps.
- **Steps:** 2000 (about 5.3 epochs), batch 1, bf16 training, fp16 save.
- **Other settings:** gradient checkpointing, SDPA, noise offset 0.03, min-SNR γ 5, seed 42.
- **Saves:** every 250 steps; the A/B uses 1000, 1500 and 2000 (final = 2000).
- **Run time:** 24 min 07 s for 2000 steps (1.38 it/s), about 25 min wall-clock from start to
  last save (20:15–20:40), on an RTX 4070 Ti (12 GB).

Output files went to `~/Projects/lora/output/` and were copied into
`~/Projects/ComfyUI/models/loras/` as `stlbrk_graffiti_v1-step0000{1000,1500,2000}` and
`-final`. The kept file, with its sha256, base, licence and trigger, is in
`~/Projects/ComfyUI/models/MODELS.md`. The licence follows SDXL base (CreativeML Open RAIL++-M),
because the training images are the project's own art.

## Retraining

1. Add or remove picks in `build_dataset.py` (`add(issue, ['rN/vK'], mode, caption)`),
   with captions starting with `stlbrk_graffiti`. Then run it from `~/Projects/lora`
   (`.venv-kohya/bin/python build_dataset.py`). It rebuilds `dataset/10_stlbrk/`.
2. Stop ComfyUI first (`systemctl --user stop comfyui`). Training needs the whole 12 GB GPU and
   about 20 GB of RAM, so don't run cargo/trunk builds alongside it.
3. Run `./train.sh`. For a new version, change `--output_name` (e.g. `stlbrk_graffiti_v2`).
   Checkpoints land in `~/Projects/lora/output/`; samples are under `output/sample/`.
4. Copy the kept step file into `models/loras/`, record it in MODELS.md, A/B it as sim-7bu.4
   did, and update this folder (a new `graffiti-v2/` for a new version).

## Using it

Put `stlbrk_graffiti` first in the prompt (`art/style.md`). The A/B results and the chosen
step/strength are recorded in sim-7bu.4 and `art/comfy/README.md`.
