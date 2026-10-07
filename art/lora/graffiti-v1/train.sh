#!/usr/bin/env bash
# Steelbreak graffiti-line-art SDXL LoRA (kohya sd-scripts)
set -euo pipefail
cd ~/Projects/lora/sd-scripts
M=~/Projects/ComfyUI/models
exec ../.venv-kohya/bin/accelerate launch --num_cpu_threads_per_process 4 --mixed_precision bf16 sdxl_train_network.py \
  --pretrained_model_name_or_path=$M/checkpoints/sd_xl_base_1.0.safetensors \
  --vae=$M/vae/sdxl_vae_fp16_fix.safetensors \
  --train_data_dir=$HOME/Projects/lora/dataset \
  --output_dir=$HOME/Projects/lora/output --output_name=stlbrk_graffiti_v1 \
  --logging_dir=$HOME/Projects/lora/output/logs \
  --caption_extension=.txt --shuffle_caption --keep_tokens=1 \
  --resolution=1024,1024 --enable_bucket --min_bucket_reso=512 --max_bucket_reso=1536 --bucket_reso_steps=64 \
  --network_module=networks.lora --network_dim=16 --network_alpha=16 \
  --optimizer_type=AdamW8bit --learning_rate=1e-4 --unet_lr=1e-4 --text_encoder_lr=3e-5 \
  --lr_scheduler=cosine_with_restarts --lr_scheduler_num_cycles=1 --lr_warmup_steps=100 \
  --max_train_steps=2000 --train_batch_size=1 \
  --mixed_precision=bf16 --save_precision=fp16 \
  --gradient_checkpointing --sdpa --cache_latents --cache_latents_to_disk \
  --noise_offset=0.03 --min_snr_gamma=5 \
  --save_every_n_steps=250 --save_model_as=safetensors \
  --sample_prompts=$HOME/Projects/lora/sample_prompts.txt --sample_every_n_steps=500 --sample_sampler=euler_a \
  --seed=42
