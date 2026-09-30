//! Explosive bricks: when one is destroyed by the ball, its blast hits its
//! neighbours by grid cell, and chains resolve within the same frame.
//!
//! - **Charge:** the 8 surrounding bricks each take 1 hit. Explosives it
//!   destroys explode too.
//! - **Breach:** the 4 orthogonal neighbours are destroyed outright. Explosives
//!   it destroys explode too.
//! - **Demolition:** the 8 surrounding bricks are destroyed outright;
//!   explosives caught in it do *not* explode.
//!
//! The chain is worked out by the pure [`resolve_blast`] on a snapshot of the
//! board, so every brick loses each hit point at most once and nothing is
//! scored twice. [`explode`] then applies the result through the same path as
//! a ball break: `BrickDestroyed` (power-up drops), score, `broke_brick` (the
//! win check), and `BrickDamaged` for survivors (regen's heal timer). Blasts
//! ignore shield glass's "from above" rule.
//!
//! After the chain is applied, [`BrickExploded`] is triggered once per
//! explosion (origin first, then each chained one) for visuals to observe;
//! the placeholder [`BlastFlash`] is one such observer.

use super::grid::{Brick, BrickHealth, BRICK_HEIGHT, BRICK_WIDTH};
use super::{BrickCell, BrickClass, ExplosiveKind};
use crate::collision::{BallCollisionSignals, BrickDamaged, BrickDestroyed};
use crate::game_state::{AppState, PlayState};
use crate::run::Score;
use crate::theme;
use bevy::prelude::*;
use std::collections::{BTreeMap, VecDeque};

/// What the board looks like to a blast: each live brick's class and hits
/// left, by cell.
pub type BlastGrid = BTreeMap<BrickCell, (BrickClass, u8)>;

/// What a blast chain does to one brick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlastHit {
    /// Hit points the chain removed (each scores 10).
    pub removed: u8,
    /// Hits left afterwards; 0 means destroyed.
    pub left: u8,
}

/// The outcome of one explosion and everything it chains into.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Blast {
    pub hits: BTreeMap<BrickCell, BlastHit>,
    /// Every cell that exploded, in order: the origin first.
    pub explosions: Vec<BrickCell>,
}

/// The cells a `kind` blast at `cell` reaches.
fn reach(cell: BrickCell, kind: ExplosiveKind) -> Vec<BrickCell> {
    let offsets: &[(i64, i64)] = match kind {
        ExplosiveKind::Breach => &[(-1, 0), (1, 0), (0, -1), (0, 1)],
        ExplosiveKind::Charge | ExplosiveKind::Demolition => &[
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
        ],
    };
    offsets
        .iter()
        .filter_map(|&(dr, dc)| {
            let row = usize::try_from(cell.row as i64 + dr).ok()?;
            let col = usize::try_from(cell.col as i64 + dc).ok()?;
            Some(BrickCell { row, col })
        })
        .collect()
}

/// Resolves the blast of a `kind` explosive destroyed at `origin`, including
/// every chained explosion, on `grid` (which need not contain the origin).
/// Bricks with 0 hits left count as already gone.
pub fn resolve_blast(grid: &BlastGrid, origin: BrickCell, kind: ExplosiveKind) -> Blast {
    let mut grid = grid.clone();
    grid.remove(&origin);
    let mut blast = Blast {
        explosions: vec![origin],
        ..default()
    };
    let mut pending = VecDeque::from([(origin, kind)]);
    while let Some((cell, kind)) = pending.pop_front() {
        for target in reach(cell, kind) {
            let Some((class, left)) = grid.get_mut(&target) else {
                continue;
            };
            if *left == 0 {
                continue;
            }
            let removed = match kind {
                ExplosiveKind::Charge => 1,
                ExplosiveKind::Breach | ExplosiveKind::Demolition => *left,
            };
            *left -= removed;
            let hit = blast.hits.entry(target).or_insert(BlastHit {
                removed: 0,
                left: 0,
            });
            hit.removed += removed;
            hit.left = *left;
            if *left == 0 {
                if let BrickClass::Explosive(next) = *class {
                    // Demolition destroys explosives without setting them off.
                    if kind != ExplosiveKind::Demolition {
                        blast.explosions.push(target);
                        pending.push_back((target, next));
                    }
                }
            }
        }
    }
    blast
}

/// Everything [`explode`] reads and changes on each brick.
type BlastTarget = (
    Entity,
    &'static BrickCell,
    &'static BrickClass,
    &'static mut BrickHealth,
    &'static Transform,
);

/// Fired once per explosion: the ball-destroyed origin, then every explosive
/// its chain sets off, in order. Visuals (the placeholder flash here, later
/// the particles epic) observe this; gameplay effects are already applied.
#[derive(Event, Debug, Clone, Copy, PartialEq)]
pub struct BrickExploded {
    pub cell: BrickCell,
    pub position: Vec2,
    pub kind: ExplosiveKind,
}

/// How long the placeholder burst lasts, and how far it grows.
const FLASH_SECS: f32 = 0.2;
const FLASH_GROWTH: f32 = 1.5;

/// A quick expanding, fading flash where an explosive went off.
#[derive(Component)]
pub struct BlastFlash(Timer);

pub struct ExplosivePlugin;

impl Plugin for ExplosivePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(explode)
            .add_observer(flash_on_explosion)
            .add_systems(
                Update,
                fade_blast_flashes.run_if(in_state(PlayState::Playing)),
            );
    }
}

/// A ball-destroyed explosive sets off its blast chain. Blast-destroyed
/// bricks (`by_blast`) are ignored here: [`resolve_blast`] already chained
/// them.
fn explode(
    on: On<BrickDestroyed>,
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut signals: ResMut<BallCollisionSignals>,
    classes: Query<(&BrickCell, &BrickClass)>,
    mut bricks: Query<BlastTarget, With<Brick>>,
) {
    if on.by_blast {
        return;
    }
    let Ok((&origin, &BrickClass::Explosive(kind))) = classes.get(on.brick) else {
        return;
    };
    let grid: BlastGrid = bricks
        .iter()
        .map(|(_, cell, class, health, _)| (*cell, (*class, health.0)))
        .collect();
    let blast = resolve_blast(&grid, origin, kind);

    let mut positions = BTreeMap::new();
    positions.insert(origin, on.position);
    for (entity, cell, class, mut health, transform) in &mut bricks {
        let position = transform.translation.truncate();
        positions.insert(*cell, position);
        let Some(hit) = blast.hits.get(cell) else {
            continue;
        };
        score.0 += 10 * i32::from(hit.removed);
        health.0 = hit.left;
        if hit.left == 0 {
            commands.trigger(BrickDestroyed {
                brick: entity,
                position,
                class: *class,
                by_blast: true,
            });
            commands.entity(entity).despawn();
            signals.broke_brick = true;
        } else {
            commands.trigger(BrickDamaged {
                brick: entity,
                position,
                class: *class,
            });
        }
    }
    // One event per explosion, the origin first, then each chained one.
    for cell in &blast.explosions {
        let kind = if *cell == origin {
            kind
        } else {
            match grid.get(cell) {
                Some((BrickClass::Explosive(kind), _)) => *kind,
                _ => continue,
            }
        };
        if let Some(&position) = positions.get(cell) {
            commands.trigger(BrickExploded {
                cell: *cell,
                position,
                kind,
            });
        }
    }
}

/// Placeholder burst for each explosion; the particles epic's explosions
/// task replaces it by observing [`BrickExploded`] too.
fn flash_on_explosion(on: On<BrickExploded>, mut commands: Commands) {
    commands.spawn((
        BlastFlash(Timer::from_seconds(FLASH_SECS, TimerMode::Once)),
        Sprite::from_color(theme::BLAST_FLASH, Vec2::new(BRICK_WIDTH, BRICK_HEIGHT)),
        Transform::from_translation(on.position.extend(0.8)),
        DespawnOnExit(AppState::InGame),
    ));
}

fn fade_blast_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut flashes: Query<(Entity, &mut BlastFlash, &mut Transform, &mut Sprite)>,
) {
    for (entity, mut flash, mut transform, mut sprite) in &mut flashes {
        if flash.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let t = flash.0.fraction();
        transform.scale = Vec3::splat(1.0 + t * FLASH_GROWTH);
        sprite.color = theme::BLAST_FLASH.with_alpha(1.0 - t);
    }
}

#[cfg(test)]
mod tests;
