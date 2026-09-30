use super::*;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn board(seed: u64) -> Vec<BrickClass> {
    random_classes(
        BOARD_ROWS * BOARD_COLS,
        REACTOR_BRICKS,
        &mut StdRng::seed_from_u64(seed),
    )
}

#[test]
fn max_hits_match_the_concept_sheet() {
    use BrickClass::*;
    assert_eq!(Ceramic.max_hits(), 1);
    assert_eq!(Titanium.max_hits(), 2);
    assert_eq!(Tungsten.max_hits(), 3);
    assert_eq!(Reactor.max_hits(), 2);
    for kind in [
        ExplosiveKind::Charge,
        ExplosiveKind::Breach,
        ExplosiveKind::Demolition,
    ] {
        assert_eq!(Explosive(kind).max_hits(), 1);
    }
    assert_eq!(Regen.max_hits(), 2);
    assert_eq!(Shield.max_hits(), 1);
}

#[test]
fn every_board_has_each_class_and_exactly_six_reactors() {
    for seed in 0..1000 {
        let board = board(seed);
        assert_eq!(
            count(&board, BrickClass::Reactor),
            REACTOR_BRICKS,
            "seed {seed}"
        );
        for (class, _) in FILL_WEIGHTS {
            assert!(count(&board, class) >= 1, "seed {seed}: no {class:?}");
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_board() {
    assert_eq!(board(7), board(7));
}

#[test]
fn boards_differ_between_seeds() {
    let boards: Vec<Vec<BrickClass>> = (0..20).map(board).collect();
    for (i, a) in boards.iter().enumerate() {
        for b in &boards[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn patching_an_all_ceramic_board_adds_each_missing_class_once() {
    for seed in 0..100 {
        let mut board = vec![BrickClass::Ceramic; BOARD_ROWS * BOARD_COLS];
        patch_classes(&mut board, REACTOR_BRICKS, &mut StdRng::seed_from_u64(seed));
        assert_eq!(count(&board, BrickClass::Reactor), REACTOR_BRICKS);
        for (class, _) in &FILL_WEIGHTS[1..] {
            assert_eq!(count(&board, *class), 1, "seed {seed}: {class:?}");
        }
        assert_eq!(
            count(&board, BrickClass::Ceramic),
            BOARD_ROWS * BOARD_COLS - REACTOR_BRICKS - 7
        );
    }
}

#[test]
fn patching_a_complete_board_only_places_reactors() {
    let original: Vec<BrickClass> = (0..BOARD_ROWS * BOARD_COLS)
        .map(|i| FILL_WEIGHTS[i % FILL_WEIGHTS.len()].0)
        .collect();
    let mut board = original.clone();
    patch_classes(&mut board, REACTOR_BRICKS, &mut StdRng::seed_from_u64(3));
    let changed: Vec<BrickClass> = board
        .iter()
        .zip(original.iter())
        .filter(|(a, b)| a != b)
        .map(|(a, _)| *a)
        .collect();
    assert_eq!(changed, [BrickClass::Reactor; REACTOR_BRICKS]);
}

#[test]
fn the_fill_roughly_follows_the_weights() {
    let (mut ceramic, mut explosive, mut shield, mut total) = (0, 0, 0, 0);
    for seed in 0..1000 {
        for class in board(seed) {
            match class {
                BrickClass::Reactor => continue,
                BrickClass::Ceramic => ceramic += 1,
                BrickClass::Explosive(_) => explosive += 1,
                BrickClass::Shield => shield += 1,
                _ => {}
            }
            total += 1;
        }
    }
    let share = |n: i32| n as f64 / total as f64;
    assert!(
        (0.28..0.32).contains(&share(ceramic)),
        "ceramic {}",
        share(ceramic)
    );
    assert!(
        (0.14..0.18).contains(&share(explosive)),
        "explosive {}",
        share(explosive)
    );
    assert!(
        (0.08..0.12).contains(&share(shield)),
        "shield {}",
        share(shield)
    );
}

#[test]
fn patching_few_random_cells_places_what_fits() {
    for n in 0..=16 {
        for reactors in [0, 3, 6, 20] {
            for seed in 0..50 {
                let mut classes = vec![BrickClass::Ceramic; n];
                patch_classes(&mut classes, reactors, &mut StdRng::seed_from_u64(seed));
                assert_eq!(
                    count(&classes, BrickClass::Reactor),
                    reactors.min(n),
                    "{n} cells, {reactors} reactors, seed {seed}"
                );
            }
        }
    }
}

#[test]
fn enough_random_cells_always_get_every_class() {
    for reactors in [0, 6] {
        let n = reactors + 8;
        for seed in 0..200 {
            let classes = random_classes(n, reactors, &mut StdRng::seed_from_u64(seed));
            assert_eq!(classes.len(), n);
            assert_eq!(count(&classes, BrickClass::Reactor), reactors);
            for (class, _) in FILL_WEIGHTS {
                assert!(
                    count(&classes, class) >= 1,
                    "{n} cells, seed {seed}: no {class:?}"
                );
            }
        }
    }
}
