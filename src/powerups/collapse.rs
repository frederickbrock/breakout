//! Collapse: catching it makes every column of bricks fall down to fill the
//! gaps below, a rare (weight 0.5) chaos power-up, at most once per level.
//!
//! **Rule** ([`collapse_moves`]). In every column of the level grid
//! ([`BoardSize`], empty rows included), the bricks stack down onto the
//! grid's bottom row in the same top-to-bottom order. Packed columns don't
//! move. A brick keeps everything it carries (class, health, max hits, regen
//! timer, outline, power-up) because it's the same entity, just moved.
//!
//! **Cells change at once, the picture follows.** On pickup each moved
//! brick's [`BrickCell`] is set to its new cell, so the explosive blast map
//! and anything else keyed on cells see the new board right away, and no two
//! bricks ever share a cell. The brick then gets [`Falling`] and Avian's
//! `ColliderDisabled`, and [`animate_collapse`] moves its `Transform`:
//! - a stagger: the lowest moving bricks start first, each row higher
//!   [`STAGGER_SECS`] later
//! - an ease-in fall ([`fall_secs`], longer for a longer drop)
//! - a small bounce of [`BOUNCE_SECS`]; at touchdown [`BrickLanded`] fires,
//!   which the particles module turns into a dust puff
//!
//! The worst case (a brick dropping from the top of a 10-row grid) is under
//! [`MAX_COLLAPSE_SECS`].
//!
//! **Solid again on settling.** After the bounce the brick drops
//! `ColliderDisabled`, unless the ball's collider AABB overlaps its cell. Then
//! it keeps it, with [`AwaitingClearance`], until the ball has left, so a
//! brick never lands solid on top of the ball. The ball flies through falling
//! bricks. Like other gameplay systems, the animation runs only while
//! playing, so pausing freezes it.

use super::{PowerUpCollected, PowerUpKind, PowerUpSpawner};
use crate::ball::Ball;
use crate::bricks::grid::{brick_translation, BoardSize, Brick, BRICK_HEIGHT, BRICK_WIDTH};
use crate::bricks::BrickCell;
use crate::game_state::PlayState;
use crate::world::GAME_SCALE;
use avian2d::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::f32::consts::PI;

const WEIGHT: f32 = 0.5;
/// Each row higher starts falling this much later.
const STAGGER_SECS: f32 = 0.04;
/// Fall time grows with the square root of the drop (rows), within bounds.
const FALL_PER_SQRT_ROW: f32 = 0.16;
const FALL_MIN_SECS: f32 = 0.35;
const FALL_MAX_SECS: f32 = 0.45;
/// The landing bounce: up and back down.
const BOUNCE_SECS: f32 = 0.12;
const BOUNCE_HEIGHT: f32 = 4.0 * GAME_SCALE;
/// The whole collapse never takes longer than this.
const MAX_COLLAPSE_SECS: f32 = 0.9;
// The worst case: the lowest moving brick starts at row MAX_ROWS - 2, and
// a brick at row 0 starts MAX_ROWS - 2 staggers later, then falls and bounces.
const _: () = assert!(
    STAGGER_SECS * (crate::levels::MAX_ROWS - 2) as f32 + FALL_MAX_SECS + BOUNCE_SECS
        <= MAX_COLLAPSE_SECS
);

pub struct CollapsePlugin;

impl Plugin for CollapsePlugin {
    fn build(&self, app: &mut App) {
        app.world_mut()
            .resource_mut::<PowerUpSpawner>()
            .register(
                PowerUpKind::Collapse,
                WEIGHT,
                crate::theme::POWER_UP_COLLAPSE,
            )
            .tinted(crate::theme::POWER_UP_COLLAPSE)
            .once_per_cycle();

        app.add_observer(collapse).add_systems(
            Update,
            animate_collapse.run_if(in_state(PlayState::Playing)),
        );
    }
}

/// A brick on its way to its new cell.
#[derive(Component, Debug)]
pub(crate) struct Falling {
    from: Vec3,
    to: Vec3,
    /// Seconds before it starts to fall (the stagger).
    delay: f32,
    /// Seconds the fall itself takes.
    fall: f32,
    elapsed: f32,
    landed: bool,
}

/// A brick that settled while the ball overlapped its cell: still not solid
/// until the ball has left.
#[derive(Component, Debug)]
pub(crate) struct AwaitingClearance;

/// A collapsing brick touched down in its new cell (the dust puff).
#[derive(Event, Debug, Clone, Copy)]
pub(crate) struct BrickLanded {
    /// The new cell's centre.
    pub(crate) position: Vec2,
}

/// (from, to) for every brick that moves when the columns of a grid of
/// `rows` rows collapse. Bricks in packed columns don't appear.
pub(crate) fn collapse_moves(cells: &[BrickCell], rows: usize) -> Vec<(BrickCell, BrickCell)> {
    let mut columns: HashMap<usize, Vec<usize>> = HashMap::default();
    for cell in cells {
        columns.entry(cell.col).or_default().push(cell.row);
    }
    let mut moves = Vec::new();
    for (col, mut col_rows) in columns {
        col_rows.sort_unstable_by(|a, b| b.cmp(a));
        // Defensive: never below a brick that already sits under the grid.
        let bottom = col_rows[0].max(rows.saturating_sub(1));
        for (i, row) in col_rows.into_iter().enumerate() {
            let target = bottom - i;
            if target != row {
                moves.push((BrickCell { row, col }, BrickCell { row: target, col }));
            }
        }
    }
    moves.sort_by_key(|(from, _)| (from.col, from.row));
    moves
}

/// Seconds a brick takes to fall `rows_dropped` rows.
pub(crate) fn fall_secs(rows_dropped: usize) -> f32 {
    (FALL_PER_SQRT_ROW * (rows_dropped as f32).sqrt()).clamp(FALL_MIN_SECS, FALL_MAX_SECS)
}

/// The stagger: a brick starting at `from_row` waits this long after the
/// lowest moving brick (at `lowest_row`) starts.
pub(crate) fn fall_delay(from_row: usize, lowest_row: usize) -> f32 {
    STAGGER_SECS * lowest_row.saturating_sub(from_row) as f32
}

/// On catching Collapse: every brick that moves gets its new cell now and
/// starts its staggered fall.
fn collapse(
    on: On<PowerUpCollected>,
    mut commands: Commands,
    board: Res<BoardSize>,
    mut bricks: Query<(Entity, &mut BrickCell, &Transform), With<Brick>>,
) {
    if on.kind != PowerUpKind::Collapse {
        return;
    }
    let by_cell: HashMap<BrickCell, Entity> = bricks
        .iter()
        .map(|(entity, cell, _)| (*cell, entity))
        .collect();
    let cells: Vec<BrickCell> = by_cell.keys().copied().collect();
    let moves = collapse_moves(&cells, board.rows);
    let Some(lowest_row) = moves.iter().map(|(from, _)| from.row).max() else {
        return;
    };
    for (from, to) in moves {
        let Ok((entity, mut cell, transform)) = bricks.get_mut(by_cell[&from]) else {
            continue;
        };
        *cell = to;
        commands
            .entity(entity)
            .insert((
                Falling {
                    from: transform.translation,
                    to: brick_translation(to, board.cols).with_z(transform.translation.z),
                    delay: fall_delay(from.row, lowest_row),
                    fall: fall_secs(to.row - from.row),
                    elapsed: 0.0,
                    landed: false,
                },
                ColliderDisabled,
            ))
            .remove::<AwaitingClearance>();
    }
}

/// The ball, for the landing check (an anchored ball isn't solid: skipped).
type SolidBall = (With<Ball>, Without<Brick>, Without<ColliderDisabled>);
type BallShape<'a> = (&'a Transform, &'a Collider);
/// A settled brick still waiting for the ball to leave its cell.
type Waiting = (With<AwaitingClearance>, Without<Falling>, With<Brick>);

/// Moves falling bricks (stagger, ease-in fall, bounce) and makes settled
/// ones solid once the ball is clear of their cell.
fn animate_collapse(
    mut commands: Commands,
    time: Res<Time>,
    balls: Query<BallShape, SolidBall>,
    mut falling: Query<(Entity, &mut Transform, &mut Falling), With<Brick>>,
    waiting: Query<(Entity, &Transform), Waiting>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut fall) in &mut falling {
        fall.elapsed += dt;
        let t = fall.elapsed - fall.delay;
        let mut y = fall.from.y;
        if t >= fall.fall + BOUNCE_SECS {
            transform.translation = fall.to;
            let mut brick = commands.entity(entity);
            brick.remove::<Falling>();
            if ball_overlaps(&balls, fall.to) {
                brick.insert(AwaitingClearance);
            } else {
                brick.remove::<ColliderDisabled>();
            }
            if !fall.landed {
                commands.trigger(BrickLanded {
                    position: fall.to.truncate(),
                });
            }
            continue;
        }
        if t >= fall.fall {
            if !fall.landed {
                fall.landed = true;
                commands.trigger(BrickLanded {
                    position: fall.to.truncate(),
                });
            }
            let v = (t - fall.fall) / BOUNCE_SECS;
            y = fall.to.y + BOUNCE_HEIGHT * (PI * v).sin();
        } else if t > 0.0 {
            let u = t / fall.fall;
            y = fall.from.y + (fall.to.y - fall.from.y) * u * u;
        }
        transform.translation.x = fall.to.x;
        transform.translation.y = y;
    }
    for (entity, transform) in &waiting {
        if !ball_overlaps(&balls, transform.translation) {
            commands
                .entity(entity)
                .remove::<(ColliderDisabled, AwaitingClearance)>();
        }
    }
}

/// Whether a solid ball's collider AABB overlaps the brick cell centred at
/// `cell_centre`. Computed from the collider, so it doesn't depend on the
/// physics step having updated `ColliderAabb`.
fn ball_overlaps(balls: &Query<BallShape, SolidBall>, cell_centre: Vec3) -> bool {
    let cell = ColliderAabb::new(
        cell_centre.truncate(),
        Vec2::new(BRICK_WIDTH, BRICK_HEIGHT) / 2.0,
    );
    balls.iter().any(|(transform, collider)| {
        collider
            .aabb(transform.translation.truncate(), Rotation::IDENTITY)
            .intersects(&cell)
    })
}

#[cfg(test)]
mod tests;
