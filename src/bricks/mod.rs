//! Brick classes, the random board and brick behaviours. Regen (sim-rdl.7.2)
//! and explosive (sim-rdl.7.3) rules build on [`BrickClass`] and
//! [`BrickCell`].
//!
//! Every run builds a fresh board with [`generate_board`], a pure function
//! (rng in, class grid out), so its guarantees are unit-testable with a
//! seeded rng.
//!
//! [`BrickClass`] is `Ceramic`, `Titanium`, `Tungsten`, `Reactor` (the
//! power-up brick), `Explosive(ExplosiveKind::{Charge, Breach, Demolition})`,
//! `Regen` or `Shield`, with `max_hits()` 1/2/3/2/1/2/1. Every brick carries
//! a [`BrickCell`] (row 0 at the top). [`generate_board`] does a weighted
//! fill (ceramic 30, titanium 20, tungsten 12, explosive 16 split over the
//! three variants, regen 12, shield 10), then one bounded patch pass that
//! places exactly [`REACTOR_BRICKS`] reactors and guarantees at least one of
//! every other class and variant.
//!
//! The brick entities themselves and the grid layout are in `grid`.
//! Per-class behaviours are submodules composed into [`BricksPlugin`]
//! (`regen`, `explosive`, and `outline` for the behaviour borders), which
//! also runs the shield-glass flash timer ([`ShieldFlash`], frozen while
//! paused). Colours come from [`crate::theme::brick_color`].

mod explosive;
pub(crate) mod grid;
mod outline;
mod regen;

use crate::game_state::PlayState;
use crate::theme;
use bevy::prelude::*;
use rand::seq::SliceRandom;
use rand::{Rng, RngExt};

pub const BOARD_ROWS: usize = 7;
pub const BOARD_COLS: usize = 10;
/// Power-up (reactor) bricks on every board.
pub const REACTOR_BRICKS: usize = 6;

pub type Board = [[BrickClass; BOARD_COLS]; BOARD_ROWS];

/// The three explosive variants, told apart later by their outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExplosiveKind {
    Charge,
    Breach,
    Demolition,
}

/// A brick's Steelbreak class: its hit count, colour and (later) behaviour.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BrickClass {
    Ceramic,
    Titanium,
    Tungsten,
    /// The power-up brick.
    Reactor,
    Explosive(ExplosiveKind),
    Regen,
    /// Shield glass: only breaks when hit from above.
    Shield,
}

impl BrickClass {
    /// Hits a full-health brick of this class takes to break.
    pub const fn max_hits(self) -> u8 {
        match self {
            BrickClass::Ceramic => 1,
            BrickClass::Titanium => 2,
            BrickClass::Tungsten => 3,
            BrickClass::Reactor => 2,
            BrickClass::Explosive(_) => 1,
            BrickClass::Regen => 2,
            BrickClass::Shield => 1,
        }
    }
}

/// A brick's slot on the board; row 0 is the top row.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BrickCell {
    pub row: usize,
    pub col: usize,
}

/// Weighted-fill table: the spec's weights ×3, so explosive's 16 splits
/// evenly over its three variants (ceramic 30, titanium 20, tungsten 12,
/// explosive 16, regen 12, shield 10). Also the list of classes every board
/// must contain at least once. Reactor bricks are placed separately.
const FILL_WEIGHTS: [(BrickClass, u32); 8] = [
    (BrickClass::Ceramic, 90),
    (BrickClass::Titanium, 60),
    (BrickClass::Tungsten, 36),
    (BrickClass::Explosive(ExplosiveKind::Charge), 16),
    (BrickClass::Explosive(ExplosiveKind::Breach), 16),
    (BrickClass::Explosive(ExplosiveKind::Demolition), 16),
    (BrickClass::Regen, 36),
    (BrickClass::Shield, 30),
];

/// A random board: every slot gets a weighted-random class, then the board
/// is patched so it has exactly [`REACTOR_BRICKS`] reactor bricks and at
/// least one brick of every other class and explosive variant.
pub fn generate_board<R: Rng + ?Sized>(rng: &mut R) -> Board {
    let mut board = [[BrickClass::Ceramic; BOARD_COLS]; BOARD_ROWS];
    for cell in board.iter_mut().flatten() {
        *cell = pick_weighted(rng);
    }
    patch_board(&mut board, rng);
    board
}

fn pick_weighted<R: Rng + ?Sized>(rng: &mut R) -> BrickClass {
    let total: u32 = FILL_WEIGHTS.iter().map(|(_, weight)| weight).sum();
    let mut roll = rng.random_range(0..total);
    for (class, weight) in FILL_WEIGHTS {
        if roll < weight {
            return class;
        }
        roll -= weight;
    }
    FILL_WEIGHTS[FILL_WEIGHTS.len() - 1].0
}

/// Places the reactor bricks and fills in any missing class, in one bounded
/// pass (no retry loop):
///
/// 1. Shuffle every cell; the first [`REACTOR_BRICKS`] become reactor.
///    Nothing below touches those cells, so there are exactly that many.
/// 2. For each class missing from the rest, overwrite one random cell whose
///    class has at least 2 cells. A present class never drops to 0, and a
///    patched-in class (count 1) is never a donor. A donor always exists:
///    while a class is missing, the 64 other cells hold at most 7 classes.
fn patch_board<R: Rng + ?Sized>(board: &mut Board, rng: &mut R) {
    let mut order: Vec<(usize, usize)> = (0..BOARD_ROWS * BOARD_COLS)
        .map(|i| (i / BOARD_COLS, i % BOARD_COLS))
        .collect();
    order.shuffle(rng);
    let (reactors, rest) = order.split_at(REACTOR_BRICKS);
    for &(row, col) in reactors {
        board[row][col] = BrickClass::Reactor;
    }
    for (needed, _) in FILL_WEIGHTS {
        if count(board, needed) > 0 {
            continue;
        }
        let &(row, col) = rest
            .iter()
            .find(|&&(row, col)| count(board, board[row][col]) >= 2)
            .expect("pigeonhole: 64 cells over at most 7 present classes");
        board[row][col] = needed;
    }
}

fn count(board: &Board, class: BrickClass) -> usize {
    board.iter().flatten().filter(|&&c| c == class).count()
}

/// How long shield glass flashes when hit from below or the side.
const SHIELD_FLASH_SECS: f32 = 0.15;

/// Shield glass is flashing after a hit that did no damage;
/// [`end_shield_flashes`] restores its face when the timer runs out.
/// Re-inserting restarts it.
#[derive(Component)]
pub struct ShieldFlash(Timer);

impl Default for ShieldFlash {
    fn default() -> Self {
        Self(Timer::from_seconds(SHIELD_FLASH_SECS, TimerMode::Once))
    }
}

pub struct BricksPlugin;

impl Plugin for BricksPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            regen::RegenPlugin,
            explosive::ExplosivePlugin,
            outline::OutlinePlugin,
        ))
        .add_systems(
            Update,
            end_shield_flashes.run_if(in_state(PlayState::Playing)),
        );
    }
}

fn end_shield_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut flashes: Query<(Entity, &mut ShieldFlash, &BrickClass, &mut Sprite)>,
) {
    for (entity, mut flash, &class, mut sprite) in &mut flashes {
        if flash.0.tick(time.delta()).is_finished() {
            sprite.color = theme::brick_color(class);
            commands.entity(entity).remove::<ShieldFlash>();
        }
    }
}

#[cfg(test)]
mod tests;
