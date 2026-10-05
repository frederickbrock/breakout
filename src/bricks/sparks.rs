//! Outline sparks: a few small sparks travelling along each special brick's
//! painted frame, so the board feels alive without competing with play.
//!
//! When a [`BrickOutline`] is added, [`add_sparks`] spawns its [`Spark`]s as
//! children of the outline (so they ride with the brick, Collapse included,
//! and go when it goes). Each spark is a small sprite just above the frame.
//! `crate::sprites` skins it with its class's 4-frame sheet
//! (`particles/outline_<class>.png`, 32×32 cells) once that image is loaded,
//! tinted in the class hue. Until then, or if the file is missing, it's a
//! small square in that hue.
//!
//! [`move_sparks`] moves each spark along the frame's rounded-rect edge
//! ([`perimeter_point`]) and flips through the sheet's frames. Each class
//! has its own [`SparkMotion`]:
//!
//! | Class | Sparks | Motion |
//! |---|---|---|
//! | explosive (all kinds) | 4 | fast and jittery (speed and offset wobble) |
//! | regen | 2 | slow and calm; faster as a hit brick nears healing |
//! | shield | 3 | a smooth, steady glide |
//! | reactor | 2 | pulsing speed and brightness; twice as fast once hit |
//!
//! It runs only while playing, so the sparks freeze while paused. A board
//! full of specials is a few hundred sprites at most.

use super::grid::{BrickHealth, BrickMaxHits};
use super::outline::{BrickOutline, OutlineStyle, FRAME_SIZE};
use super::regen::RegenTimer;
use crate::game_state::PlayState;
use crate::theme;
use crate::world::GAME_SCALE;
use bevy::prelude::*;
use std::f32::consts::TAU;

/// A spark's drawn size.
pub const SPARK_SIZE: f32 = 10.0 * GAME_SCALE;
/// Above the frame (local to the outline, which is above the plate).
const SPARK_Z: f32 = 0.05;
/// The path runs along the middle of the frame's ring: inset from its edge.
const PATH_INSET: f32 = 3.0 * GAME_SCALE;
/// The path's corner radius.
const PATH_RADIUS: f32 = 8.0 * GAME_SCALE;
/// Sheet frames per second while a spark flickers through them.
const FRAME_FPS: f32 = 8.0;

/// Which sheet and motion a spark uses (the explosive kinds share one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SparkClass {
    Explosive,
    Regen,
    Shield,
    Reactor,
}

impl SparkClass {
    /// Every class, in the order `GameSprites::spark_sheets` holds them.
    pub const ALL: [Self; 4] = [Self::Explosive, Self::Regen, Self::Shield, Self::Reactor];

    pub fn of(style: OutlineStyle) -> Self {
        match style {
            OutlineStyle::Charge | OutlineStyle::Breach | OutlineStyle::Demolition => {
                Self::Explosive
            }
            OutlineStyle::Regen => Self::Regen,
            OutlineStyle::Shield => Self::Shield,
            OutlineStyle::Reactor => Self::Reactor,
        }
    }

    /// The 4-frame sheet, relative to `assets/`.
    pub fn sheet_path(self) -> &'static str {
        match self {
            Self::Explosive => "particles/outline_explosive.png",
            Self::Regen => "particles/outline_regen.png",
            Self::Shield => "particles/outline_shield.png",
            Self::Reactor => "particles/outline_reactor.png",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Explosive => theme::OUTLINE_EXPLOSIVE,
            Self::Regen => theme::OUTLINE_REGEN,
            Self::Shield => theme::OUTLINE_SHIELD,
            Self::Reactor => theme::OUTLINE_REACTOR,
        }
    }

    pub fn motion(self) -> SparkMotion {
        match self {
            Self::Explosive => SparkMotion {
                count: 4,
                laps_per_sec: 0.55,
                wobble: 0.6,
                wobble_hz: 7.0,
                jitter: 2.0 * GAME_SCALE,
                pulse_hz: 0.0,
            },
            Self::Regen => SparkMotion {
                count: 2,
                laps_per_sec: 0.12,
                wobble: 0.0,
                wobble_hz: 0.0,
                jitter: 0.0,
                pulse_hz: 0.0,
            },
            Self::Shield => SparkMotion {
                count: 3,
                laps_per_sec: 0.25,
                wobble: 0.0,
                wobble_hz: 0.0,
                jitter: 0.0,
                pulse_hz: 0.0,
            },
            Self::Reactor => SparkMotion {
                count: 2,
                laps_per_sec: 0.2,
                wobble: 0.7,
                wobble_hz: 0.6,
                jitter: 0.0,
                pulse_hz: 0.6,
            },
        }
    }
}

/// How a class's sparks move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SparkMotion {
    /// Sparks per brick (2–4), spread evenly round the frame.
    pub count: usize,
    /// Base speed, in laps of the frame per second.
    pub laps_per_sec: f32,
    /// Speed swing (0..1) around the base, at `wobble_hz`.
    pub wobble: f32,
    pub wobble_hz: f32,
    /// Sideways jitter off the path (world units); 0 = smooth.
    pub jitter: f32,
    /// Brightness swell rate (Hz); 0 = steady.
    pub pulse_hz: f32,
}

/// One spark on an outline (a child of the [`BrickOutline`]).
#[derive(Component, Debug)]
pub struct Spark {
    pub class: SparkClass,
    /// Where it is along the frame, in laps (0..1).
    pub lap: f32,
    /// Seconds it has run (drives wobble, pulse and the sheet frame).
    pub age: f32,
    /// Its place among its outline's sparks (offsets wobble and frames).
    index: usize,
}

pub struct SparksPlugin;

impl Plugin for SparksPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(add_sparks)
            .add_systems(Update, move_sparks.run_if(in_state(PlayState::Playing)));
    }
}

/// The point `lap` (0..1, by arc length, clockwise from the top-left end of
/// the top edge) round a rounded rectangle of `size` centred on the origin.
pub fn perimeter_point(size: Vec2, radius: f32, lap: f32) -> Vec2 {
    let half = size / 2.0;
    let r = radius.min(half.x).min(half.y);
    let (w, h) = (size.x - 2.0 * r, size.y - 2.0 * r);
    let arc = TAU * r / 4.0;
    let total = 2.0 * (w + h) + 4.0 * arc;
    let mut d = lap.rem_euclid(1.0) * total;
    // Straight edges and corners, in order: top, top-right corner, right,
    // bottom-right, bottom, bottom-left, left, top-left.
    let corner = |centre: Vec2, start_angle: f32, d: f32| {
        let a = start_angle - d / r.max(f32::EPSILON);
        centre + Vec2::new(a.cos(), a.sin()) * r
    };
    let (ix, iy) = (half.x - r, half.y - r);
    if d < w {
        return Vec2::new(-ix + d, half.y);
    }
    d -= w;
    if d < arc {
        return corner(Vec2::new(ix, iy), TAU / 4.0, d);
    }
    d -= arc;
    if d < h {
        return Vec2::new(half.x, iy - d);
    }
    d -= h;
    if d < arc {
        return corner(Vec2::new(ix, -iy), 0.0, d);
    }
    d -= arc;
    if d < w {
        return Vec2::new(ix - d, -half.y);
    }
    d -= w;
    if d < arc {
        return corner(Vec2::new(-ix, -iy), -TAU / 4.0, d);
    }
    d -= arc;
    if d < h {
        return Vec2::new(-half.x, -iy + d);
    }
    d -= h;
    corner(Vec2::new(-ix, iy), TAU / 2.0, d.min(arc))
}

/// The rounded rect the sparks travel: the middle of the frame's ring.
pub fn spark_path() -> Vec2 {
    FRAME_SIZE - Vec2::splat(2.0 * PATH_INSET)
}

/// How much faster than its base a spark of `class` runs on a brick at
/// `health` of `max_hits` (with `heal`, a regen brick's heal progress 0..1).
/// Full-health bricks run at 1×.
pub fn damage_speedup(class: SparkClass, health: u8, max_hits: u8, heal: Option<f32>) -> f32 {
    if health >= max_hits {
        return 1.0;
    }
    match class {
        // Matches the frame's blink: quicker the closer it is to healing.
        SparkClass::Regen => 1.5 + 2.5 * heal.unwrap_or(0.0).clamp(0.0, 1.0),
        SparkClass::Reactor => 2.0,
        // Explosive and shield bricks die in one hit by default; a level's
        // `hits=` can make them tougher, so speed them up a little too.
        SparkClass::Explosive | SparkClass::Shield => 1.5,
    }
}

/// Gives every new outline its class's sparks, spread evenly round the frame.
fn add_sparks(on: On<Add, BrickOutline>, mut commands: Commands, outlines: Query<&BrickOutline>) {
    let Ok(outline) = outlines.get(on.entity) else {
        return;
    };
    let class = SparkClass::of(outline.style);
    let count = class.motion().count;
    for index in 0..count {
        let lap = index as f32 / count as f32;
        commands.spawn((
            Spark {
                class,
                lap,
                age: 0.0,
                index,
            },
            Sprite::from_color(class.color(), Vec2::splat(SPARK_SIZE)),
            Transform::from_translation(
                perimeter_point(spark_path(), PATH_RADIUS, lap).extend(SPARK_Z),
            ),
            ChildOf(on.entity),
        ));
    }
}

/// The parts of a spark this system moves.
type SparkParts<'a> = (
    &'a mut Spark,
    &'a ChildOf,
    &'a mut Transform,
    &'a mut Sprite,
);

/// Moves every spark along its frame and flickers it through the sheet.
fn move_sparks(
    time: Res<Time>,
    mut sparks: Query<SparkParts>,
    outlines: Query<&ChildOf, With<BrickOutline>>,
    bricks: Query<(&BrickHealth, &BrickMaxHits, Option<&RegenTimer>)>,
) {
    let dt = time.delta_secs();
    let path = spark_path();
    for (mut spark, parent, mut transform, mut sprite) in &mut sparks {
        let motion = spark.class.motion();
        let speedup = outlines
            .get(parent.parent())
            .ok()
            .and_then(|brick| bricks.get(brick.parent()).ok())
            .map_or(1.0, |(health, max, heal)| {
                damage_speedup(
                    spark.class,
                    health.0,
                    max.0,
                    heal.map(RegenTimer::fraction_elapsed),
                )
            });
        spark.age += dt;
        let phase = spark.index as f32 * 1.7;
        let wobble = 1.0 + motion.wobble * (TAU * motion.wobble_hz * spark.age + phase).sin();
        spark.lap =
            (spark.lap + motion.laps_per_sec * speedup * wobble.max(0.1) * dt).rem_euclid(1.0);

        let mut at = perimeter_point(path, PATH_RADIUS, spark.lap);
        if motion.jitter > 0.0 {
            let t = spark.age * 23.0 + phase;
            at += Vec2::new((t * 1.3).sin(), (t * 1.7).cos()) * motion.jitter;
        }
        transform.translation.x = at.x;
        transform.translation.y = at.y;

        let brightness = if motion.pulse_hz > 0.0 {
            0.65 + 0.35 * (0.5 + 0.5 * (TAU * motion.pulse_hz * spark.age).sin())
        } else {
            1.0
        };
        sprite.color = spark.class.color().with_alpha(brightness);
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.index = ((spark.age * FRAME_FPS) as usize + spark.index) % 4;
        }
    }
}

#[cfg(test)]
mod tests;
