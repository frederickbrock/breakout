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
//! Every pixel stays inside the [`PLAYFIELD_WIDTH`]×[`PLAYFIELD_HEIGHT`]
//! well: nothing leaks into the side panels or the letterbox bars.
//!
//! The layers are global (they survive restarts) and ambient: [`drift`] runs
//! on `Time<Real>`, ignoring `PlayState`, the physics clock and the virtual
//! clock (the particles pause that), so they keep moving on every screen.
//! A layer whose image is missing or still loading isn't drawn; with none,
//! the plain `Background` (`sprites`, z −10) shows as before.

use bevy::prelude::*;

use crate::sprites::GameSprites;
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
    /// A non-tiling layer's centre at start, in world space.
    pub(crate) start: Vec2,
}

/// The layers, back to front. All sit between the `Background` (z −10) and
/// the frame (z −5). To put the planet behind the clouds, give it a z below
/// theirs.
pub(crate) const LAYERS: [LayerSpec; 4] = [
    LayerSpec {
        path: "parallax/space_l0.png",
        z: -9.0,
        speed: 8.0,
        tiled: true,
        start: Vec2::ZERO,
    },
    LayerSpec {
        path: "parallax/space_l1.png",
        z: -8.0,
        speed: 16.0,
        tiled: true,
        start: Vec2::ZERO,
    },
    LayerSpec {
        path: "parallax/space_l2.png",
        z: -7.0,
        speed: 28.0,
        tiled: true,
        start: Vec2::ZERO,
    },
    LayerSpec {
        path: "parallax/planet.png",
        z: -6.0,
        speed: 2.0,
        tiled: false,
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
        app.add_systems(Startup, spawn_layers)
            .add_systems(Update, drift.run_if(resource_exists::<GameSprites>));
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

#[cfg(test)]
mod tests;
