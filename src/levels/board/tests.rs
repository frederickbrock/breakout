use super::*;
use crate::bricks::{BOARD_COLS, BOARD_ROWS, REACTOR_BRICKS};
use crate::levels::parse_level;
use crate::tuning::BrickTuning;
use rand::rngs::StdRng;
use rand::SeedableRng;

/// Every class the random fill must produce at least once on a full board.
const EVERY_CLASS: [BrickClass; 8] = [
    BrickClass::Ceramic,
    BrickClass::Titanium,
    BrickClass::Tungsten,
    BrickClass::Explosive(crate::bricks::ExplosiveKind::Charge),
    BrickClass::Explosive(crate::bricks::ExplosiveKind::Breach),
    BrickClass::Explosive(crate::bricks::ExplosiveKind::Demolition),
    BrickClass::Regen,
    BrickClass::Shield,
];

fn level(text: &str) -> LevelDef {
    match parse_level(text) {
        Ok(def) => def,
        Err(e) => panic!("{e}"),
    }
}

fn build(def: &LevelDef, seed: u64) -> Vec<PlacedBrick> {
    build_board(
        def,
        &mut StdRng::seed_from_u64(seed),
        &BrickTuning::default(),
    )
}

fn cell(row: usize, col: usize) -> BrickCell {
    BrickCell { row, col }
}

fn at(board: &[PlacedBrick], row: usize, col: usize) -> Option<PlacedBrick> {
    board.iter().copied().find(|b| b.cell == cell(row, col))
}

fn count(board: &[PlacedBrick], class: BrickClass) -> usize {
    board.iter().filter(|b| b.class == class).count()
}

#[test]
fn fixed_cells_keep_their_class_position_hits_and_powerup() {
    let def = level("legend:\nk = titanium hits=4\nv = ceramic powerup\ngrid:\nC.T\nkPv");
    let board = build(&def, 1);
    assert_eq!(board.len(), 5);
    assert_eq!(at(&board, 0, 1), None);
    let brick = |class, hits, powerup| (class, hits, powerup);
    let got = |r, c| at(&board, r, c).map(|b| brick(b.class, b.hits, b.powerup));
    assert_eq!(got(0, 0), Some(brick(BrickClass::Ceramic, 1, false)));
    assert_eq!(got(0, 2), Some(brick(BrickClass::Titanium, 2, false)));
    assert_eq!(got(1, 0), Some(brick(BrickClass::Titanium, 4, false)));
    assert_eq!(got(1, 1), Some(brick(BrickClass::Reactor, 2, false)));
    assert_eq!(got(1, 2), Some(brick(BrickClass::Ceramic, 1, true)));
}

#[test]
fn the_fallback_board_is_the_random_board() {
    let def = LevelDef::fallback();
    for seed in 0..500 {
        let board = build(&def, seed);
        assert_eq!(board.len(), BOARD_ROWS * BOARD_COLS, "seed {seed}");
        for row in 0..BOARD_ROWS {
            for col in 0..BOARD_COLS {
                assert!(at(&board, row, col).is_some(), "seed {seed}: ({row},{col})");
            }
        }
        assert_eq!(count(&board, BrickClass::Reactor), REACTOR_BRICKS);
        for class in EVERY_CLASS {
            assert!(count(&board, class) >= 1, "seed {seed}: no {class:?}");
        }
        for brick in &board {
            assert!(!brick.powerup);
            assert_eq!(brick.hits, brick.class.max_hits());
        }
    }
}

#[test]
fn the_fallback_board_matches_random_classes_seed_for_seed() {
    let def = LevelDef::fallback();
    for seed in 0..50 {
        let classes: Vec<BrickClass> = build(&def, seed).iter().map(|b| b.class).collect();
        let expected = crate::bricks::random_classes(
            BOARD_ROWS * BOARD_COLS,
            REACTOR_BRICKS,
            &crate::bricks::FILL_WEIGHTS,
            &mut StdRng::seed_from_u64(seed),
        );
        assert_eq!(classes, expected, "seed {seed}");
    }
}

#[test]
fn the_same_seed_builds_the_same_board() {
    let def = LevelDef::fallback();
    assert_eq!(build(&def, 9), build(&def, 9));
    assert_ne!(build(&def, 9), build(&def, 10));
}

#[test]
fn powerups_turn_random_cells_into_reactors() {
    let def = level("powerups: 3\ngrid:\nCC???\n?????");
    for seed in 0..100 {
        let board = build(&def, seed);
        assert_eq!(board.len(), 10);
        assert_eq!(count(&board, BrickClass::Reactor), 3, "seed {seed}");
        for brick in &board {
            if brick.class == BrickClass::Reactor {
                assert!(
                    !(brick.cell.row == 0 && brick.cell.col < 2),
                    "a reactor on a fixed cell"
                );
            }
            assert!(!brick.powerup);
        }
        assert_eq!(at(&board, 0, 0).map(|b| b.class), Some(BrickClass::Ceramic));
        assert_eq!(at(&board, 0, 1).map(|b| b.class), Some(BrickClass::Ceramic));
    }
}

#[test]
fn extra_powerups_beyond_the_random_cells_flag_other_bricks() {
    let def = level("powerups: 3\ngrid:\nCCCC?");
    for seed in 0..100 {
        let board = build(&def, seed);
        assert_eq!(at(&board, 0, 4).map(|b| b.class), Some(BrickClass::Reactor));
        assert_eq!(count(&board, BrickClass::Ceramic), 4);
        let flagged: Vec<_> = board.iter().filter(|b| b.powerup).collect();
        assert_eq!(flagged.len(), 2, "seed {seed}");
        assert!(flagged.iter().all(|b| b.class == BrickClass::Ceramic));
    }
}

#[test]
fn extra_powerups_are_capped_at_the_brick_count() {
    let board = build(&level("powerups: 9\ngrid:\nCC"), 4);
    assert_eq!(board.len(), 2);
    assert!(board
        .iter()
        .all(|b| b.powerup && b.class == BrickClass::Ceramic));
}

#[test]
fn random_cells_honour_hits_and_powerup() {
    let def = level("legend:\nr = random hits=5 powerup\ngrid:\nrr");
    let mut classes = std::collections::HashSet::new();
    for seed in 0..100 {
        for brick in build(&def, seed) {
            assert_eq!(brick.hits, 5);
            assert!(brick.powerup);
            classes.insert(brick.class);
        }
    }
    assert!(classes.len() > 2, "the class is random: {classes:?}");
}

#[test]
fn a_level_without_random_cells_or_extra_powerups_ignores_the_seed() {
    let def = level("grid:\nXBD\nRSP\nCTG");
    let first = build(&def, 0);
    for seed in 1..20 {
        assert_eq!(build(&def, seed), first);
    }
}
