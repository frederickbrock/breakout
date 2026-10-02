//! Brick classes, the random board and brick behaviours. Regen (sim-rdl.7.2)
//! and explosive (sim-rdl.7.3) rules build on [`BrickClass`] and
//! [`BrickCell`].
//!
//! Every run resolves its random (`?`) cells with [`random_classes`], a pure
//! function (rng in, classes out), so its guarantees are unit-testable with a
//! seeded rng.
//!
//! [`BrickClass`] is `Ceramic`, `Titanium`, `Tungsten`, `Reactor` (the
//! power-up brick), `Explosive(ExplosiveKind::{Charge, Breach, Demolition})`,
//! `Regen` or `Shield`, with `max_hits()` 1/2/3/2/1/2/1. Every brick carries
//! a [`BrickCell`] (row 0 at the top). [`random_classes`] does a weighted
//! fill (ceramic 30, titanium 20, tungsten 12, explosive 16 split over the
//! three variants, regen 12, shield 10), then one bounded patch pass that
//! places the requested number of reactors and, given enough cells, at least
//! one of every other class and variant. For the default 7x10 board with
//! [`REACTOR_BRICKS`] reactors that guarantee always holds.
//!
//! The brick entities themselves and the grid layout are in `grid`.
//! Per-class behaviours are submodules composed into [`BricksPlugin`]
//! (`regen`, `explosive`, and `outline` for the behaviour borders), which
//! also runs the shield-glass flash timer ([`ShieldFlash`], frozen while
//! paused). Colours come from [`crate::theme::brick_color`]. [`damage_look`]
//! is the damage-sprite ladder (`sprites` draws it).

mod explosive;
pub(crate) mod grid;
mod outline;
mod regen;

use crate::game_state::PlayState;
use crate::theme;
use bevy::prelude::*;
use rand::seq::SliceRandom;
use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};

pub const BOARD_ROWS: usize = 7;
pub const BOARD_COLS: usize = 10;
/// Power-up (reactor) bricks on every board.
pub const REACTOR_BRICKS: usize = 6;

/// The three explosive variants, told apart later by their outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExplosiveKind {
    Charge,
    Breach,
    Demolition,
}

/// A brick's Steelbreak class: its hit count, colour and (later) behaviour.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// One resolved brick of a board: where it goes, its class, its full health
/// and whether it drops a power-up besides being a reactor. Produced by
/// `crate::levels::build_board`, spawned by [`grid::spawn_bricks`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedBrick {
    pub cell: BrickCell,
    pub class: BrickClass,
    pub hits: u8,
    pub powerup: bool,
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

/// Classes for `slots` random (`?`) cells, in order: a weighted fill, then
/// [`patch_classes`] places `reactors` reactors (capped at `slots`) and fills
/// in missing classes. For 70 slots and [`REACTOR_BRICKS`] reactors this is
/// exactly the old 7x10 random board, consuming the rng the same way.
pub fn random_classes<R: Rng + ?Sized>(
    slots: usize,
    reactors: usize,
    rng: &mut R,
) -> Vec<BrickClass> {
    let mut classes: Vec<BrickClass> = (0..slots).map(|_| pick_weighted(rng)).collect();
    patch_classes(&mut classes, reactors, rng);
    classes
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
/// 1. Shuffle every cell; the first `reactors` (capped at the cell count)
///    become reactor. Nothing below touches those cells, so there are
///    exactly that many.
/// 2. For each class missing from the rest, overwrite one random cell whose
///    class has at least 2 cells. A present class never drops to 0, and a
///    patched-in class (count 1) is never a donor. A donor always exists
///    when the rest has at least 8 cells (pigeonhole: while a class is
///    missing, those cells hold at most 7 classes); with fewer it stops
///    quietly (best effort).
fn patch_classes<R: Rng + ?Sized>(classes: &mut [BrickClass], reactors: usize, rng: &mut R) {
    let mut order: Vec<usize> = (0..classes.len()).collect();
    order.shuffle(rng);
    let (reactor_slots, rest) = order.split_at(reactors.min(classes.len()));
    for &i in reactor_slots {
        classes[i] = BrickClass::Reactor;
    }
    for (needed, _) in FILL_WEIGHTS {
        if count(classes, needed) > 0 {
            continue;
        }
        // Too few random cells for every class: best effort.
        let Some(&i) = rest.iter().find(|&&i| count(classes, classes[i]) >= 2) else {
            break;
        };
        classes[i] = needed;
    }
}

fn count(classes: &[BrickClass], class: BrickClass) -> usize {
    classes.iter().filter(|&&c| c == class).count()
}

/// Which plate a damaged brick draws (sim-rdl.7.9). `Broken` always means
/// the next hit destroys it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageLook {
    Intact,
    Cracked,
    Broken,
}

/// The damage ladder for a brick of `max_hits` (its own `BrickMaxHits`: the
/// class's `max_hits()` or a level's `hits=`): at full health it's intact,
/// with a single hit left it's broken, and anything in between is cracked
/// (by default only tungsten's 2 of 3). A 1-hit brick never shows damage.
pub fn damage_look(max_hits: u8, hits_left: u8) -> DamageLook {
    let max = max_hits;
    if max <= 1 || hits_left >= max {
        DamageLook::Intact
    } else if hits_left <= 1 {
        DamageLook::Broken
    } else {
        DamageLook::Cracked
    }
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
