//! Brick entities on the board: the [`Brick`] marker, [`BrickHealth`], the
//! grid layout and [`spawn_bricks`].
//!
//! [`BRICK_WIDTH`] is derived, not set: a full row fills the playfield well
//! less a [`SIDE_CHANNEL`] each side, and [`SIDE_CHANNEL_BALLS`] (ball widths)
//! is the one knob. The pure [`brick_x`] and [`brick_y`] centre the grid, so
//! fewer columns just widen the equal channels. Each brick spawns at its own
//! [`BrickMaxHits`] (its class's `max_hits()`) with its
//! [`crate::bricks::BrickCell`], scoped to the run.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::ball::BALL_SIZE;
use crate::bricks::{self, BrickCell};
use crate::game_state::AppState;
use crate::theme;
use crate::world::{GAME_SCALE, PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH};

/// Minimum clear gap between the outermost brick and each wall, in ball
/// widths. Raising it narrows the (derived) bricks; nothing else changes.
pub(crate) const SIDE_CHANNEL_BALLS: f32 = 3.0;
pub(crate) const SIDE_CHANNEL: f32 = SIDE_CHANNEL_BALLS * BALL_SIZE;
/// Gap between neighbouring bricks, both ways.
pub(crate) const BRICK_GAP: f32 = 5.0 * GAME_SCALE;
/// Distance from the top wall to the top of the first brick row.
pub(crate) const BRICK_TOP_MARGIN: f32 = 50.0 * GAME_SCALE;
pub(crate) const BRICK_HEIGHT: f32 = 30.0 * GAME_SCALE;
/// Derived so a full row fills the well less a [`SIDE_CHANNEL`] each side.
pub(crate) const BRICK_WIDTH: f32 =
    (PLAYFIELD_WIDTH - 2.0 * SIDE_CHANNEL - (bricks::BOARD_COLS as f32 - 1.0) * BRICK_GAP)
        / bricks::BOARD_COLS as f32;
// Board size, for tests across modules (the game itself uses `bricks::BOARD_*`).
#[cfg(test)]
pub(crate) const BRICK_ROWS: usize = bricks::BOARD_ROWS;
#[cfg(test)]
pub(crate) const BRICK_COLS: usize = bricks::BOARD_COLS;

#[derive(Component)]
pub(crate) struct Brick;

/// Hits a brick still takes before it breaks; it spawns at its
/// [`BrickMaxHits`].
#[derive(Component)]
pub(crate) struct BrickHealth(pub(crate) u8);

/// The hits a full-health brick of this kind takes: its class's
/// `max_hits()`, or a level's `hits=` override. Healing (regen) and damage
/// visuals compare against this, never the class default.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BrickMaxHits(pub(crate) u8);

/// A fresh random board of brick classes (see [`bricks::generate_board`]),
/// each brick at its class's colour and hit count.
pub(crate) fn spawn_bricks(commands: &mut Commands) {
    let board = bricks::generate_board(&mut rand::rng());
    for (row, classes) in board.iter().enumerate() {
        for (col, &class) in classes.iter().enumerate() {
            let cell = BrickCell { row, col };
            commands.spawn((
                Sprite::from_color(
                    theme::brick_color(class),
                    Vec2::new(BRICK_WIDTH, BRICK_HEIGHT),
                ),
                Transform::from_translation(brick_translation(cell)),
                RigidBody::Static,
                Collider::rectangle(BRICK_WIDTH, BRICK_HEIGHT),
                Brick,
                class,
                cell,
                BrickHealth(class.max_hits()),
                BrickMaxHits(class.max_hits()),
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

/// Where the brick in `cell` sits (see [`brick_x`] and [`brick_y`]).
pub(crate) fn brick_translation(cell: BrickCell) -> Vec3 {
    Vec3::new(
        brick_x(cell.col, bricks::BOARD_COLS),
        brick_y(cell.row),
        0.0,
    )
}

/// Centre x of column `col` in a row of `cols` bricks, centred in the well so
/// the left and right channels are equal (fewer columns, wider channels).
pub(crate) fn brick_x(col: usize, cols: usize) -> f32 {
    let grid_width = cols as f32 * BRICK_WIDTH + (cols as f32 - 1.0) * BRICK_GAP;
    -grid_width / 2.0 + BRICK_WIDTH / 2.0 + col as f32 * (BRICK_WIDTH + BRICK_GAP)
}

/// Centre y of row `row` (0 at the top), [`BRICK_TOP_MARGIN`] below the top
/// wall.
pub(crate) fn brick_y(row: usize) -> f32 {
    PLAYFIELD_HEIGHT / 2.0
        - BRICK_TOP_MARGIN
        - BRICK_HEIGHT / 2.0
        - row as f32 * (BRICK_HEIGHT + BRICK_GAP)
}

#[cfg(test)]
mod tests;
