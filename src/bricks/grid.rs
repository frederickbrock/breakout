//! Brick entities on the board: the [`Brick`] marker, [`BrickHealth`],
//! [`BrickMaxHits`], [`CarriesPowerUp`], the grid layout and
//! [`spawn_bricks`], which spawns a resolved board (from
//! `crate::levels::build_board`) centred for its column count.
//!
//! [`BRICK_WIDTH`] is derived, not set: a full row fills the playfield well
//! less a [`SIDE_CHANNEL`] each side, and [`SIDE_CHANNEL_BALLS`] (ball widths)
//! is the one knob. The pure [`brick_x`] and [`brick_y`] centre the grid, so
//! fewer columns just widen the equal channels. Row 0's top sits
//! [`BRICK_TOP_OFFSET`] (120) below the top wall: the original
//! [`BRICK_TOP_MARGIN`] plus one brick height of [`BRICK_HEADROOM`]. Each brick
//! spawns at its own [`BrickMaxHits`] (its class's `max_hits()` or a level's
//! `hits=`) with its
//! [`crate::bricks::BrickCell`], scoped to the run.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::ball::BALL_SIZE;
use crate::bricks::{self, BrickCell, PlacedBrick};
use crate::game_state::AppState;
use crate::theme;
use crate::world::{GAME_SCALE, PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH};

/// Minimum clear gap between the outermost brick and each wall, in ball
/// widths. Raising it narrows the (derived) bricks; nothing else changes.
pub(crate) const SIDE_CHANNEL_BALLS: f32 = 3.0;
pub(crate) const SIDE_CHANNEL: f32 = SIDE_CHANNEL_BALLS * BALL_SIZE;
/// Gap between neighbouring bricks, both ways.
pub(crate) const BRICK_GAP: f32 = 5.0 * GAME_SCALE;
/// The original gap from the top wall to the first brick row (design value).
pub(crate) const BRICK_TOP_MARGIN: f32 = 50.0 * GAME_SCALE;
pub(crate) const BRICK_HEIGHT: f32 = 30.0 * GAME_SCALE;
/// Extra room above the grid, on top of [`BRICK_TOP_MARGIN`], so a ball that
/// gets up a side channel can travel across the top of the bricks. The knob
/// for how far down the whole grid sits.
pub(crate) const BRICK_HEADROOM: f32 = BRICK_HEIGHT;
/// Distance from the top wall to the top of the first brick row (120).
pub(crate) const BRICK_TOP_OFFSET: f32 = BRICK_TOP_MARGIN + BRICK_HEADROOM;
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

/// A non-reactor brick a level flagged `powerup`: power-ups equip it with a
/// `PowerUpBrick` at run start, like a reactor.
#[derive(Component)]
pub(crate) struct CarriesPowerUp;

/// Spawns a resolved board (see `crate::levels::build_board`) of `cols`
/// columns, each brick at its class's colour and its own hit count.
pub(crate) fn spawn_bricks(commands: &mut Commands, board: &[PlacedBrick], cols: usize) {
    for brick in board {
        let mut entity = commands.spawn((
            Sprite::from_color(
                theme::brick_color(brick.class),
                Vec2::new(BRICK_WIDTH, BRICK_HEIGHT),
            ),
            Transform::from_translation(brick_translation(brick.cell, cols)),
            RigidBody::Static,
            Collider::rectangle(BRICK_WIDTH, BRICK_HEIGHT),
            Brick,
            brick.class,
            brick.cell,
            BrickHealth(brick.hits),
            BrickMaxHits(brick.hits),
            DespawnOnExit(AppState::InGame),
        ));
        if brick.powerup {
            entity.insert(CarriesPowerUp);
        }
    }
}

/// Where the brick in `cell` of a `cols`-wide grid sits (see [`brick_x`] and
/// [`brick_y`]).
pub(crate) fn brick_translation(cell: BrickCell, cols: usize) -> Vec3 {
    Vec3::new(brick_x(cell.col, cols), brick_y(cell.row), 0.0)
}

/// Centre x of column `col` in a row of `cols` bricks, centred in the well so
/// the left and right channels are equal (fewer columns, wider channels).
pub(crate) fn brick_x(col: usize, cols: usize) -> f32 {
    let grid_width = cols as f32 * BRICK_WIDTH + (cols as f32 - 1.0) * BRICK_GAP;
    -grid_width / 2.0 + BRICK_WIDTH / 2.0 + col as f32 * (BRICK_WIDTH + BRICK_GAP)
}

/// Centre y of row `row` (0 at the top); row 0's top is [`BRICK_TOP_OFFSET`]
/// below the top wall.
pub(crate) fn brick_y(row: usize) -> f32 {
    PLAYFIELD_HEIGHT / 2.0
        - BRICK_TOP_OFFSET
        - BRICK_HEIGHT / 2.0
        - row as f32 * (BRICK_HEIGHT + BRICK_GAP)
}

#[cfg(test)]
mod tests;
