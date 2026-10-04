//! Levels as data: the run's board comes from a level file instead of being
//! hard-coded.
//!
//! [`LevelDef`] is format-independent: the `.level` text grid parsed by
//! [`parse_level`] is the first format, and a later JSON or HTTP loader will
//! produce the same model. A `.level` file has optional `name:`,
//! `speed_factor:` and `powerups:` keys, an optional `legend:` that adds or
//! overrides symbols, and a `grid:` of 1-10 equal rows of 1-10 symbols
//! (`C` ceramic, `T` titanium, `G` tungsten, `X`/`B`/`D` explosive
//! charge/breach/demolition, `R` regen, `S` shield, `P` reactor, `?` random,
//! `.` empty). The full reference is `docs/levels.md`.
//!
//! The campaign manifest `assets/levels/campaign.txt` lists the level files
//! in order (the web build can't list directories); a run plays them in that
//! order and clearing the last one wins (see `crate::campaign`).
//! [`LevelsPlugin`] loads both as assets and keeps [`CampaignLevels`] (every
//! playable level, in order) current. It is registered from `main()`, not
//! `add_game`, so the headless tests never read files: they insert
//! [`CampaignLevels`] directly.
//!
//! `start_run` reads level 0 through [`campaign_level`] fresh every run and
//! resolves it with [`build_board`]; each later level is read when the run
//! reaches it. With no campaign (not loaded yet, every level invalid, or in
//! tests) the run plays [`LevelDef::fallback`], the built-in random 7x10
//! board with 6 reactors, as its only level. Hot reload (native) therefore
//! applies at the next run or when the run reaches the edited level, never
//! mid-board. An invalid file is logged by Bevy with the file, line and
//! column, and that level is skipped.

mod board;
mod loader;
mod parse;

pub(crate) use board::build_board;
pub(crate) use loader::LevelsPlugin;
pub(crate) use parse::{parse_campaign, parse_level};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::bricks::{BrickClass, BOARD_COLS, BOARD_ROWS, REACTOR_BRICKS};

/// Most rows a level's grid may have.
pub(crate) const MAX_ROWS: usize = 10;
/// Most columns a level's grid may have: a full row is the default board's
/// width, which sizes the bricks.
pub(crate) const MAX_COLS: usize = BOARD_COLS;

const _: () = assert!(MAX_COLS == 10);

/// What a grid cell holds: a fixed class, or a weighted-random one (`?`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassSpec {
    Fixed(BrickClass),
    Random,
}

/// One brick of a level, before `?` and `powerups:` are resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellDef {
    pub class: ClassSpec,
    /// Overrides the class's `max_hits()`.
    pub hits: Option<u8>,
    /// This brick drops a power-up when broken (reactors always do).
    pub powerup: bool,
}

/// A level, independent of the file format it came from.
#[derive(Asset, TypePath, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelDef {
    pub name: String,
    /// `speed_factor: F`: the ball's speed is 300 x F world units/s. `None`
    /// uses the per-round ramp, `ball::speed_factor(round)` (1.8 = 540 in
    /// round 1, +0.1 a round, capped at 2.5).
    pub speed_factor: Option<f32>,
    /// `powerups: N`: this many random bricks become power-up bricks.
    pub extra_powerups: usize,
    /// Rows top to bottom, all the same width; `None` is an empty cell.
    pub grid: Vec<Vec<Option<CellDef>>>,
}

impl LevelDef {
    /// Columns in every row.
    pub fn cols(&self) -> usize {
        self.grid.first().map_or(0, Vec::len)
    }

    /// The built-in random board: 7x10 of `?`, `powerups: 6`, the default
    /// speed. Identical to `assets/levels/01-random.level`.
    pub fn fallback() -> Self {
        let random = CellDef {
            class: ClassSpec::Random,
            hits: None,
            powerup: false,
        };
        Self {
            name: "Random".into(),
            speed_factor: None,
            extra_powerups: REACTOR_BRICKS,
            grid: vec![vec![Some(random); BOARD_COLS]; BOARD_ROWS],
        }
    }
}

/// The campaign manifest (`campaign.txt`): level file names, in play order.
#[derive(Asset, TypePath, Clone, Debug, PartialEq, Default)]
pub struct Campaign {
    pub levels: Vec<String>,
}

/// Every playable campaign level, in play order. Kept current by
/// [`LevelsPlugin`]; tests insert it directly. Absent: the built-in
/// [`LevelDef::fallback`] is the only level.
#[derive(Resource, Clone, Debug, PartialEq)]
pub(crate) struct CampaignLevels(pub(crate) Vec<LevelDef>);

/// The campaign's level at 0-based `index`; with no (or an empty) campaign,
/// [`LevelDef::fallback`] is the only level. `None` past the last.
pub(crate) fn campaign_level(campaign: Option<&CampaignLevels>, index: usize) -> Option<LevelDef> {
    match campaign {
        Some(c) if !c.0.is_empty() => c.0.get(index).cloned(),
        _ => (index == 0).then(LevelDef::fallback),
    }
}

#[cfg(test)]
mod tests;
