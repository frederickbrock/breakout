//! Resolving a [`LevelDef`] into the bricks a run spawns: [`build_board`].
//!
//! `?` cells get their classes from [`crate::bricks::random_classes`] (the
//! weighted fill plus the patch pass), which also turns up to `powerups: N`
//! of them into reactor bricks. If N is larger than the number of `?` cells,
//! the rest go to random other bricks (not reactors, not already flagged) as
//! a `powerup` flag, keeping their class; N is capped at the brick count.
//! The "one of every class" guarantee applies among the `?` cells, as far as
//! their number allows.

use rand::seq::SliceRandom;
use rand::Rng;

use super::{ClassSpec, LevelDef};
use crate::bricks::{self, BrickCell, BrickClass, PlacedBrick};

/// Every brick of `def`, row-major, with `?` and `powerups: N` resolved. An
/// all-`?` level uses the rng exactly as the old random board did, so the
/// fallback level reproduces it seed for seed.
pub(crate) fn build_board<R: Rng + ?Sized>(def: &LevelDef, rng: &mut R) -> Vec<PlacedBrick> {
    let cells: Vec<_> = def
        .grid
        .iter()
        .enumerate()
        .flat_map(|(row, cells)| {
            cells
                .iter()
                .enumerate()
                .filter_map(move |(col, cell)| cell.map(|cell| (BrickCell { row, col }, cell)))
        })
        .collect();
    let random_count = cells
        .iter()
        .filter(|(_, cell)| cell.class == ClassSpec::Random)
        .count();
    let mut random = bricks::random_classes(random_count, def.extra_powerups, rng).into_iter();

    let mut board: Vec<PlacedBrick> = cells
        .into_iter()
        .map(|(cell, def)| {
            let class = match def.class {
                ClassSpec::Fixed(class) => class,
                // `random` has exactly one class per `?` cell.
                ClassSpec::Random => random.next().unwrap_or(BrickClass::Ceramic),
            };
            PlacedBrick {
                cell,
                class,
                hits: def.hits.unwrap_or(class.max_hits()),
                powerup: def.powerup,
            }
        })
        .collect();

    let overflow = def.extra_powerups.saturating_sub(random_count);
    if overflow > 0 {
        let mut candidates: Vec<usize> = board
            .iter()
            .enumerate()
            .filter(|(_, brick)| brick.class != BrickClass::Reactor && !brick.powerup)
            .map(|(i, _)| i)
            .collect();
        candidates.shuffle(rng);
        for &i in candidates.iter().take(overflow) {
            board[i].powerup = true;
        }
    }
    board
}

#[cfg(test)]
mod tests;
