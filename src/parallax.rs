//! Deep-space parallax behind the playfield well: three layers drifting
//! downward at different speeds, plus a distant planet.
//!
//! [`LAYERS`] is the one table (file, z, speed, tiling, planet start). Its
//! images load with the other sprites (`GameSprites::parallax`, so the web
//! splash waits for them too). [`ParallaxPlugin`] spawns the layer sprites
//! once at `Startup` and moves them every frame in [`drift`]:
//! - a **tiling** layer (a vertically seamless texture as wide as the well)
//!   is two sprites, each showing a crop of the texture ([`tile_slices`]), so
//!   the wrap point never shows a seam, gap or jump;
//! - the **planet** is one sprite at [`planet_y`], which re-enters from above
//!   the top once it has fully left the bottom, cropped to the well
//!   ([`crop_to_well`]).
//!
//! The **glow** layer (`space_l1_glow.png`, white with alpha = intensity)
//! drifts with L1, in register with its cloud forks, and is tinted
//! [`theme::NEBULA_GLOW`] at [`glow_intensity`]: a slow breathing between
//! [`GLOW_FLOOR`] and [`GLOW_BREATH_TOP`], plus lightning strikes every
//! [`STRIKE_GAP_SECS`] (2–4 quick flashes up to the capped [`GLOW_PEAK`],
//! then a fade) from a [`StrikeSchedule`] refilled as strikes pass. It's
//! ambient: real time, no gameplay reaction.
//!
//! Every pixel stays inside the [`PLAYFIELD_WIDTH`]×[`PLAYFIELD_HEIGHT`]
//! well: nothing leaks into the side panels or the letterbox bars.
//!
//! The layers are global (they survive restarts) and ambient: [`drift`] runs
//! on `Time<Real>`, ignoring `PlayState`, the physics clock and the virtual
//! clock (the particles pause that), so they keep moving on every screen.
//! A layer whose image is missing or still loading isn't drawn; with none,
//! the plain `Background` (`sprites`, z −10) shows as before.

use bevy::prelude::*;

use std::collections::VecDeque;

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::sprites::GameSprites;
use crate::theme;
use crate::world::{PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH};

/// One parallax layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LayerSpec {
    /// Image path, relative to `assets/`.
    pub(crate) path: &'static str,
    pub(crate) z: f32,
    /// Downward drift, world px/s.
    pub(crate) speed: f32,
    /// A vertically seamless texture as wide as the well, wrapping forever;
    /// otherwise a single sprite (the planet) that re-enters from the top.
    pub(crate) tiled: bool,
    /// A glow mask (white, alpha = intensity), tinted [`theme::NEBULA_GLOW`]
    /// and pulsed by [`glow_intensity`]; it shares its cloud layer's speed
    /// and size, so it stays in register with the clouds' forks.
    pub(crate) glow: bool,
    /// A non-tiling layer's centre at start, in world space.
    pub(crate) start: Vec2,
}

pub(crate) const LAYER_COUNT: usize = 5;

/// The layers, back to front. All sit between the `Background` (z −10) and
/// the frame (z −5). To put the planet behind the clouds, give it a z below
/// theirs.
pub(crate) const LAYERS: [LayerSpec; LAYER_COUNT] = [
    LayerSpec {
        path: "parallax/space_l0.png",
        z: -9.0,
        speed: 8.0,
        tiled: true,
        glow: false,
        start: Vec2::ZERO,
    },
    LayerSpec {
        path: "parallax/space_l1.png",
        z: -8.0,
        speed: 16.0,
        tiled: true,
        glow: false,
        start: Vec2::ZERO,
    },
    LayerSpec {
        // L1's lightning glow: L1's speed, between L1 and L2.
        path: "parallax/space_l1_glow.png",
        z: -7.5,
        speed: 16.0,
        tiled: true,
        glow: true,
        start: Vec2::ZERO,
    },
    LayerSpec {
        path: "parallax/space_l2.png",
        z: -7.0,
        speed: 28.0,
        tiled: true,
        glow: false,
        start: Vec2::ZERO,
    },
    LayerSpec {
        path: "parallax/planet.png",
        z: -6.0,
        speed: 2.0,
        tiled: false,
        glow: false,
        // Lower right, its centre 100 px inside the right edge.
        start: Vec2::new(
            PLAYFIELD_WIDTH / 2.0 - 100.0,
            -PLAYFIELD_HEIGHT / 2.0 + 200.0,
        ),
    },
];

/// The well's rectangle in world space.
pub(crate) fn well() -> Rect {
    Rect::from_center_size(Vec2::ZERO, Vec2::new(PLAYFIELD_WIDTH, PLAYFIELD_HEIGHT))
}

/// How far (`[0, period)`) a layer drifting at `speed` has moved after
/// `elapsed` seconds, wrapping every `period`.
pub(crate) fn tile_offset(elapsed: f32, speed: f32, period: f32) -> f32 {
    if period <= 0.0 {
        return 0.0;
    }
    (elapsed * speed).rem_euclid(period)
}

/// One visible piece of a tiling layer: rows `[src_top, src_top + height)`
/// of the texture, drawn `screen_top` below the top of the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Slice {
    pub(crate) src_top: f32,
    pub(crate) height: f32,
    pub(crate) screen_top: f32,
}

/// The two pieces of a `period`-tall seamless texture filling a `view`-tall
/// window after the content has drifted down by `offset` (`[0, period)`):
/// the top of the view shows texture row `period - offset`, and the rest
/// wraps to row 0. A piece with no height is not drawn.
pub(crate) fn tile_slices(offset: f32, period: f32, view: f32) -> [Slice; 2] {
    let top = (period - offset).rem_euclid(period);
    let first = (period - top).min(view);
    [
        Slice {
            src_top: top,
            height: first,
            screen_top: 0.0,
        },
        Slice {
            src_top: 0.0,
            height: view - first,
            screen_top: first,
        },
    ]
}

/// The planet's centre height after `elapsed` seconds, starting at `start_y`
/// and drifting down at `speed`. Once it's wholly below the well (its top
/// under the bottom edge) it re-enters from just above the top: one cycle
/// is `PLAYFIELD_HEIGHT + size` of travel.
pub(crate) fn planet_y(elapsed: f32, speed: f32, start_y: f32, size: f32) -> f32 {
    let top = PLAYFIELD_HEIGHT / 2.0 + size / 2.0;
    let cycle = PLAYFIELD_HEIGHT + size;
    top - (top - start_y + elapsed * speed).rem_euclid(cycle)
}

/// The part of a `size` sprite centred at `centre` (drawn at 1 texel per
/// world px) that lies inside the well: its world rect and the matching
/// texture rect (y down). `None` when it's wholly outside.
pub(crate) fn crop_to_well(centre: Vec2, size: Vec2) -> Option<(Rect, Rect)> {
    let full = Rect::from_center_size(centre, size);
    let visible = full.intersect(well());
    if visible.is_empty() {
        return None;
    }
    let texture = Rect::new(
        visible.min.x - full.min.x,
        full.max.y - visible.max.y,
        visible.max.x - full.min.x,
        full.max.y - visible.min.y,
    );
    Some((visible, texture))
}

/// The glow's breathing range (fraction of full brightness) and period.
pub(crate) const GLOW_FLOOR: f32 = 0.15;
pub(crate) const GLOW_BREATH_TOP: f32 = 0.35;
pub(crate) const GLOW_BREATH_SECS: f32 = 5.0;
/// The brightest a strike gets: capped below full so the brick area stays
/// calm (the art also dims the forks behind the brick rows).
///
/// Why 0.85: style.md keeps the background under ~12% contrast behind the
/// brick area. The glow is the mask's alpha times this tint alpha, and the
/// placeholder `space_l1_glow.png` peaks at alpha 0.47. So a strike tops out
/// at about 0.85 × 0.47 ≈ 0.40 composited, on the brightest forks only, and
/// most of the mask is far dimmer. Re-check this value against the cap when
/// the sim-rdl.10 mask replaces the placeholder: a brighter mask needs a lower peak.
pub(crate) const GLOW_PEAK: f32 = 0.85;
/// Seconds between strikes (random in this range), the flicker's length
/// and flash count ranges, and the fade back to breathing.
pub(crate) const STRIKE_GAP_SECS: (f32, f32) = (6.0, 15.0);
pub(crate) const STRIKE_FLICKER_SECS: (f32, f32) = (0.3, 0.6);
pub(crate) const STRIKE_FLASHES: (u8, u8) = (2, 4);
pub(crate) const STRIKE_FADE_SECS: f32 = 1.0;
/// Within each flash, the share of its slot spent lit, and the dip between.
const FLASH_ON: f32 = 0.6;
const FLASH_DIP: f32 = 0.35;

/// One lightning strike: when it starts, how long it flickers, how often.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Strike {
    pub(crate) start: f32,
    pub(crate) flicker: f32,
    pub(crate) flashes: u8,
}

impl Strike {
    /// When it has fully faded back to breathing.
    pub(crate) fn end(&self) -> f32 {
        self.start + self.flicker + STRIKE_FADE_SECS
    }

    /// How far toward [`GLOW_PEAK`] (`0..=1`) the strike pushes the glow at
    /// `t`: flashes of full brightness with dips between, then a smooth
    /// fade to 0.
    pub(crate) fn envelope(&self, t: f32) -> f32 {
        let dt = t - self.start;
        if dt < 0.0 || t >= self.end() {
            return 0.0;
        }
        if dt < self.flicker {
            let slot = dt / self.flicker * self.flashes as f32;
            // The last flash stays lit into the fade, so there's no jump.
            let last = slot >= self.flashes.saturating_sub(1) as f32;
            return if last || slot.fract() < FLASH_ON {
                1.0
            } else {
                FLASH_DIP
            };
        }
        let fade = (dt - self.flicker) / STRIKE_FADE_SECS;
        1.0 - fade * fade * (3.0 - 2.0 * fade)
    }
}

/// The slow breathing between strikes, in `GLOW_FLOOR..=GLOW_BREATH_TOP`.
pub(crate) fn breathing(t: f32) -> f32 {
    let phase = (t / GLOW_BREATH_SECS * std::f32::consts::TAU).cos();
    GLOW_FLOOR + (GLOW_BREATH_TOP - GLOW_FLOOR) * (0.5 - 0.5 * phase)
}

/// The glow's brightness at `t`: breathing, pushed toward [`GLOW_PEAK`] by
/// any strike under way. Always in `GLOW_FLOOR..=GLOW_PEAK`.
pub(crate) fn glow_intensity<'a>(t: f32, strikes: impl IntoIterator<Item = &'a Strike>) -> f32 {
    let base = breathing(t);
    let push = strikes
        .into_iter()
        .map(|s| s.envelope(t))
        .fold(0.0, f32::max);
    base + (GLOW_PEAK - base) * push
}

/// The upcoming strikes, drawn from a seeded rng and refilled as they pass.
#[derive(Resource)]
pub(crate) struct StrikeSchedule {
    rng: StdRng,
    pub(crate) strikes: VecDeque<Strike>,
    /// When the last scheduled strike starts.
    last_start: f32,
}

impl StrikeSchedule {
    pub(crate) fn seeded(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
            strikes: VecDeque::new(),
            last_start: 0.0,
        }
    }

    /// Drops strikes that have ended by `t` and schedules ahead so at least
    /// the next strike after `t` is known.
    pub(crate) fn advance(&mut self, t: f32) {
        while self.strikes.front().is_some_and(|s| s.end() <= t) {
            self.strikes.pop_front();
        }
        while self.last_start <= t || self.strikes.is_empty() {
            let gap = self.rng.random_range(STRIKE_GAP_SECS.0..=STRIKE_GAP_SECS.1);
            let start = self.last_start + gap;
            self.strikes.push_back(Strike {
                start,
                flicker: self
                    .rng
                    .random_range(STRIKE_FLICKER_SECS.0..=STRIKE_FLICKER_SECS.1),
                flashes: self.rng.random_range(STRIKE_FLASHES.0..=STRIKE_FLASHES.1),
            });
            self.last_start = start;
        }
    }
}

impl Default for StrikeSchedule {
    fn default() -> Self {
        Self::seeded(rand::rng().random())
    }
}

/// One sprite of a parallax layer: `part` 0/1 of a tiling layer, or the
/// planet's only sprite.
#[derive(Component, Debug)]
pub(crate) struct ParallaxSprite {
    pub(crate) layer: usize,
    pub(crate) part: usize,
}

pub(crate) struct ParallaxPlugin;

impl Plugin for ParallaxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StrikeSchedule>()
            .add_systems(Startup, spawn_layers)
            .add_systems(
                Update,
                (drift.run_if(resource_exists::<GameSprites>), pulse_glow),
            );
    }
}

fn spawn_layers(mut commands: Commands) {
    for (layer, spec) in LAYERS.iter().enumerate() {
        let parts = if spec.tiled { 2 } else { 1 };
        for part in 0..parts {
            commands.spawn((
                ParallaxSprite { layer, part },
                Sprite::default(),
                Transform::from_xyz(0.0, 0.0, spec.z),
                Visibility::Hidden,
            ));
        }
    }
}

/// Moves every layer to where `Time<Real>` puts it, cropping each sprite to
/// the well. A layer whose image isn't loaded stays hidden.
fn drift(
    time: Res<Time<Real>>,
    sprites: Res<GameSprites>,
    images: Res<Assets<Image>>,
    mut layers: Query<(
        &ParallaxSprite,
        &mut Sprite,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    let elapsed = time.elapsed_secs();
    let well = well();
    for (piece, mut sprite, mut transform, mut visibility) in &mut layers {
        let spec = LAYERS[piece.layer];
        let handle = &sprites.parallax[piece.layer];
        let placed = images.get(handle).and_then(|image| {
            let size = image.size_f32();
            if size.min_element() <= 0.0 {
                return None;
            }
            if spec.tiled {
                // Drawn as wide as the well; the same scale applies down.
                let scale = PLAYFIELD_WIDTH / size.x;
                let period = size.y * scale;
                let offset = tile_offset(elapsed, spec.speed, period);
                let slice = tile_slices(offset, period, PLAYFIELD_HEIGHT)[piece.part];
                (slice.height > 0.0).then(|| {
                    let world = Rect::new(
                        well.min.x,
                        well.max.y - slice.screen_top - slice.height,
                        well.max.x,
                        well.max.y - slice.screen_top,
                    );
                    let texture = Rect::new(
                        0.0,
                        slice.src_top / scale,
                        size.x,
                        (slice.src_top + slice.height) / scale,
                    );
                    (world, texture)
                })
            } else {
                let y = planet_y(elapsed, spec.speed, spec.start.y, size.y);
                crop_to_well(Vec2::new(spec.start.x, y), size)
            }
        });
        let Some((world, texture)) = placed else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        if sprite.image != *handle {
            sprite.image = handle.clone();
        }
        sprite.rect = Some(texture);
        sprite.custom_size = Some(world.size());
        transform.translation = world.center().extend(spec.z);
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
    }
}

/// Sets each glow layer's tint alpha to [`glow_intensity`] at `Time<Real>`.
fn pulse_glow(
    time: Res<Time<Real>>,
    mut schedule: ResMut<StrikeSchedule>,
    mut layers: Query<(&ParallaxSprite, &mut Sprite)>,
) {
    let t = time.elapsed_secs();
    schedule.advance(t);
    let color = theme::NEBULA_GLOW.with_alpha(glow_intensity(t, &schedule.strikes));
    for (piece, mut sprite) in &mut layers {
        if LAYERS[piece.layer].glow && sprite.color != color {
            sprite.color = color;
        }
    }
}

#[cfg(test)]
mod tests;
