use super::*;
use crate::tuning::BlastTuning;
use ExplosiveKind::*;

const C: BrickClass = BrickClass::Ceramic;
const T: BrickClass = BrickClass::Titanium;
const G: BrickClass = BrickClass::Tungsten;

fn cell(row: usize, col: usize) -> BrickCell {
    BrickCell { row, col }
}

/// A grid from rows of classes (`None` = empty), each at full health.
fn grid(rows: &[&[Option<BrickClass>]]) -> BlastGrid {
    let mut grid = BlastGrid::new();
    for (r, row) in rows.iter().enumerate() {
        for (c, class) in row.iter().enumerate() {
            if let Some(class) = class {
                grid.insert(cell(r, c), (*class, class.max_hits()));
            }
        }
    }
    grid
}

/// A 5×5 grid of `fill` with `center` at (2, 2).
fn around(fill: BrickClass, center: BrickClass) -> BlastGrid {
    let mut g = BlastGrid::new();
    for r in 0..5 {
        for c in 0..5 {
            let class = if (r, c) == (2, 2) { center } else { fill };
            g.insert(cell(r, c), (class, class.max_hits()));
        }
    }
    g
}

fn ring() -> Vec<BrickCell> {
    (1..=3)
        .flat_map(|r| (1..=3).map(move |c| cell(r, c)))
        .filter(|&c| c != cell(2, 2))
        .collect()
}

#[test]
fn charge_takes_one_hit_from_each_of_the_8_around() {
    let mut g = around(T, BrickClass::Explosive(Charge));
    g.insert(cell(1, 1), (C, 1));
    let blast = resolve_blast(&g, cell(2, 2), Charge, &BlastTuning::default());
    assert_eq!(blast.hits.len(), 8);
    assert_eq!(
        blast.hits[&cell(1, 1)],
        BlastHit {
            removed: 1,
            left: 0
        },
        "1-hit breaks"
    );
    for c in ring().into_iter().filter(|&c| c != cell(1, 1)) {
        assert_eq!(
            blast.hits[&c],
            BlastHit {
                removed: 1,
                left: 1
            },
            "{c:?} cracks"
        );
    }
    assert!(
        !blast.hits.contains_key(&cell(0, 0)),
        "two cells away: untouched"
    );
    assert_eq!(blast.explosions, [cell(2, 2)]);
}

#[test]
fn breach_destroys_the_4_orthogonal_outright_but_not_diagonals() {
    let blast = resolve_blast(
        &around(G, BrickClass::Explosive(Breach)),
        cell(2, 2),
        Breach,
        &BlastTuning::default(),
    );
    let expected: BTreeMap<_, _> = [cell(1, 2), cell(3, 2), cell(2, 1), cell(2, 3)]
        .into_iter()
        .map(|c| {
            (
                c,
                BlastHit {
                    removed: 3,
                    left: 0,
                },
            )
        })
        .collect();
    assert_eq!(
        blast.hits, expected,
        "3-hit tungsten destroyed outright, +30 each"
    );
}

#[test]
fn demolition_destroys_all_8_and_does_not_chain() {
    let mut g = around(G, BrickClass::Explosive(Demolition));
    g.insert(cell(1, 2), (BrickClass::Explosive(Charge), 1));
    let blast = resolve_blast(&g, cell(2, 2), Demolition, &BlastTuning::default());
    assert_eq!(blast.hits.len(), 8);
    assert!(blast.hits.values().all(|h| h.left == 0));
    assert_eq!(
        blast.explosions,
        [cell(2, 2)],
        "the charge caught in it doesn't go off"
    );
    assert!(!blast.hits.contains_key(&cell(0, 2)));
}

#[test]
fn charge_and_breach_chain_into_explosives_they_destroy() {
    let x = Some(BrickClass::Explosive(Charge));
    let b = Some(BrickClass::Explosive(Breach));
    let c = Some(C);
    let t = Some(T);
    // Row 0: charge(origin) · breach · ceramic · titanium
    let g = grid(&[&[x, b, c, t], &[c, c, c, c]]);
    let blast = resolve_blast(&g, cell(0, 0), Charge, &BlastTuning::default());
    // The charge destroys the breach at (0,1), which goes off in turn.
    assert_eq!(blast.explosions, [cell(0, 0), cell(0, 1)]);
    // (0,2) is out of the charge's reach but orthogonal to the breach.
    assert_eq!(
        blast.hits[&cell(0, 2)],
        BlastHit {
            removed: 1,
            left: 0
        }
    );
    // (1,1) is hit by the charge first; the breach finds it already gone.
    assert_eq!(
        blast.hits[&cell(1, 1)],
        BlastHit {
            removed: 1,
            left: 0
        }
    );
    // (0,3) is two cells from both: untouched.
    assert!(!blast.hits.contains_key(&cell(0, 3)));
}

#[test]
fn no_brick_is_scored_twice_in_a_chain() {
    // Two charges side by side, surrounded by titanium: the shared
    // neighbours take one hit from each blast, never more than they have.
    let x = Some(BrickClass::Explosive(Charge));
    let t = Some(T);
    let g = grid(&[&[t, t, t, t], &[t, x, x, t], &[t, t, t, t]]);
    let blast = resolve_blast(&g, cell(1, 1), Charge, &BlastTuning::default());
    assert_eq!(blast.explosions, [cell(1, 1), cell(1, 2)]);
    for (c, hit) in &blast.hits {
        let (_, full) = g[c];
        assert_eq!(hit.removed + hit.left, full, "{c:?}");
    }
    // Shared neighbours of both charges: two hits, titanium destroyed.
    assert_eq!(
        blast.hits[&cell(0, 1)],
        BlastHit {
            removed: 2,
            left: 0
        }
    );
    assert_eq!(
        blast.hits[&cell(0, 0)],
        BlastHit {
            removed: 1,
            left: 1
        }
    );
}

#[test]
fn blasts_ignore_the_board_edge_and_gone_bricks() {
    let mut g = grid(&[
        &[Some(BrickClass::Explosive(Charge)), Some(C)],
        &[Some(C), Some(C)],
    ]);
    g.insert(cell(1, 1), (C, 0)); // already broken this frame
    let blast = resolve_blast(&g, cell(0, 0), Charge, &BlastTuning::default());
    assert_eq!(blast.hits.len(), 2);
    assert!(!blast.hits.contains_key(&cell(1, 1)));
}

mod in_game {
    use super::super::*;
    use crate::ball::{Ball, BallApproach};
    use crate::bricks::grid::BrickMaxHits;
    use crate::bricks::regen::RegenTimer;
    use crate::bricks::ExplosiveKind::*;
    use crate::game_state::{AppState, GameOutcome};
    use crate::powerups::{PowerUp, PowerUpBrick};
    use crate::test_support::*;
    use crate::theme;

    const FROM_ABOVE: Vec2 = Vec2::new(60.0, -300.0);

    fn at(app: &mut App, row: usize, col: usize) -> Entity {
        app.world_mut()
            .query::<(Entity, &BrickCell)>()
            .iter(app.world())
            .find(|(_, c)| **c == BrickCell { row, col })
            .map(|(e, _)| e)
            .expect("a brick in that cell")
    }

    /// Makes the brick at (row, col) a full-health `class` brick.
    fn set(app: &mut App, row: usize, col: usize, class: BrickClass) -> Entity {
        let brick = at(app, row, col);
        app.world_mut().entity_mut(brick).insert((
            class,
            BrickHealth(class.max_hits()),
            BrickMaxHits(class.max_hits()),
            Sprite::from_color(theme::brick_color(class), Vec2::ONE),
        ));
        brick
    }

    /// Makes the 8 bricks around (row, col) plain ceramic, so a random
    /// explosive on the board can't join the chain under test.
    fn isolate(app: &mut App, row: usize, col: usize) {
        for r in row.saturating_sub(1)..=(row + 1).min(6) {
            for c in col.saturating_sub(1)..=(col + 1).min(9) {
                if (r, c) != (row, col) {
                    set(app, r, c, BrickClass::Ceramic);
                }
            }
        }
    }

    fn score(app: &App) -> i32 {
        app.world().resource::<Score>().0
    }

    fn gone(app: &App, brick: Entity) -> bool {
        app.world().get_entity(brick).is_err()
    }

    fn detonate(app: &mut App, brick: Entity) {
        hit_moving(app, brick, FROM_ABOVE);
    }

    #[test]
    fn a_ball_set_off_charge_hits_its_ring_through_the_normal_break_path() {
        let mut app = app();
        let charge = set(&mut app, 3, 4, BrickClass::Explosive(Charge));
        let ceramic = set(&mut app, 2, 3, BrickClass::Ceramic);
        let tungsten = set(&mut app, 4, 5, BrickClass::Tungsten);
        for (r, c) in [(2, 4), (2, 5), (3, 3), (3, 5), (4, 3), (4, 4)] {
            set(&mut app, r, c, BrickClass::Titanium);
        }
        let far = set(&mut app, 0, 0, BrickClass::Ceramic);
        detonate(&mut app, charge);

        assert!(gone(&app, charge) && gone(&app, ceramic));
        assert_eq!(app.world().get::<BrickHealth>(tungsten).unwrap().0, 2);
        assert_eq!(
            app.world().get::<Sprite>(tungsten).unwrap().color,
            theme::TUNGSTEN,
            "damage shows as particles, not a darker colour"
        );
        assert!(!gone(&app, far));
        // 10 for the charge + 10 per ring brick (8).
        assert_eq!(score(&app), 90);
        assert!(app.world().resource::<BallCollisionSignals>().broke_brick);
    }

    #[test]
    fn a_blast_breaks_shield_glass_whatever_the_ball_did() {
        let mut app = app();
        let breach = set(&mut app, 3, 4, BrickClass::Explosive(Breach));
        let shield = set(&mut app, 4, 4, BrickClass::Shield);
        // The ball was moving *up* when it hit the shield's neighbour —
        // irrelevant to the blast.
        let ball = app
            .world_mut()
            .query_filtered::<Entity, With<Ball>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .entity_mut(ball)
            .insert(BallApproach(Vec2::new(0.0, 300.0)));
        hit(&mut app, breach);
        assert!(gone(&app, shield));
    }

    #[test]
    fn a_power_up_brick_destroyed_by_a_blast_drops_its_power_up() {
        let mut app = app();
        let (reactor, cell) = app
            .world_mut()
            .query_filtered::<(Entity, &BrickCell), With<PowerUpBrick>>()
            .iter(app.world())
            .map(|(e, c)| (e, *c))
            .next()
            .unwrap();
        // A breach right next to it (left or right, whichever is on the board).
        let col = if cell.col > 0 {
            cell.col - 1
        } else {
            cell.col + 1
        };
        let breach = set(&mut app, cell.row, col, BrickClass::Explosive(Breach));
        detonate(&mut app, breach);
        assert!(gone(&app, reactor));
        assert!(count::<With<PowerUp>>(&mut app) >= 1);
    }

    #[test]
    fn charge_damage_restarts_a_regen_bricks_heal_timer() {
        let mut app = app();
        isolate(&mut app, 3, 4);
        let charge = set(&mut app, 3, 4, BrickClass::Explosive(Charge));
        let regen = set(&mut app, 3, 5, BrickClass::Regen);
        detonate(&mut app, charge);
        assert_eq!(app.world().get::<BrickHealth>(regen).unwrap().0, 1);
        assert!(app.world().entity(regen).contains::<RegenTimer>());
    }

    #[test]
    fn a_blast_that_clears_the_last_bricks_wins() {
        let mut app = app();
        tap(&mut app, KeyCode::Space);
        let demolition = set(&mut app, 3, 4, BrickClass::Explosive(Demolition));
        let neighbour = set(&mut app, 2, 4, BrickClass::Tungsten);
        for brick in bricks(&mut app) {
            if brick != demolition && brick != neighbour {
                app.world_mut().despawn(brick);
            }
        }
        detonate(&mut app, demolition);
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::GameOver);
        assert_eq!(
            app.world().get_resource::<GameOutcome>(),
            Some(&GameOutcome::Won)
        );
    }

    #[derive(Resource, Default)]
    struct Exploded(Vec<BrickExploded>);

    #[test]
    fn each_explosion_fires_brick_exploded_including_chains() {
        let mut app = app();
        app.init_resource::<Exploded>().add_observer(
            |on: On<BrickExploded>, mut seen: ResMut<Exploded>| seen.0.push(*on.event()),
        );
        isolate(&mut app, 3, 4);
        isolate(&mut app, 3, 5);
        let charge = set(&mut app, 3, 4, BrickClass::Explosive(Charge));
        set(&mut app, 3, 5, BrickClass::Explosive(Breach));
        let charge_at = app
            .world()
            .get::<Transform>(charge)
            .unwrap()
            .translation
            .truncate();
        let breach = at(&mut app, 3, 5);
        let breach_at = app
            .world()
            .get::<Transform>(breach)
            .unwrap()
            .translation
            .truncate();
        detonate(&mut app, charge);

        let seen = &app.world().resource::<Exploded>().0;
        assert_eq!(
            seen,
            &[
                BrickExploded {
                    cell: BrickCell { row: 3, col: 4 },
                    position: charge_at,
                    kind: Charge,
                    radius: 1,
                },
                BrickExploded {
                    cell: BrickCell { row: 3, col: 5 },
                    position: breach_at,
                    kind: Breach,
                    radius: 1,
                },
            ]
        );
    }

    #[test]
    fn a_demolition_fires_one_event_for_itself_only() {
        let mut app = app();
        app.init_resource::<Exploded>().add_observer(
            |on: On<BrickExploded>, mut seen: ResMut<Exploded>| seen.0.push(*on.event()),
        );
        isolate(&mut app, 3, 4);
        let demolition = set(&mut app, 3, 4, BrickClass::Explosive(Demolition));
        set(&mut app, 3, 5, BrickClass::Explosive(Charge));
        detonate(&mut app, demolition);
        let seen = &app.world().resource::<Exploded>().0;
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].kind, Demolition);
    }
}

// ---- tunable radius and chaining (sim-dj6.4) ----

/// A 7×7 grid of `fill` with `center` at (3, 3).
fn around7(fill: BrickClass, center: BrickClass) -> BlastGrid {
    let mut g = BlastGrid::new();
    for r in 0..7 {
        for c in 0..7 {
            let class = if (r, c) == (3, 3) { center } else { fill };
            g.insert(cell(r, c), (class, class.max_hits()));
        }
    }
    g
}

fn tuned(f: impl FnOnce(&mut BlastTuning)) -> BlastTuning {
    let mut t = BlastTuning::default();
    f(&mut t);
    t
}

#[test]
fn radius_one_is_exactly_the_original_offset_tables() {
    assert_eq!(blast_offsets(Breach, 1), [(-1, 0), (1, 0), (0, -1), (0, 1)]);
    let square = [
        (-1, -1),
        (-1, 0),
        (-1, 1),
        (0, -1),
        (0, 1),
        (1, -1),
        (1, 0),
        (1, 1),
    ];
    assert_eq!(blast_offsets(Charge, 1), square);
    assert_eq!(blast_offsets(Demolition, 1), square);
    let t = BlastTuning::default();
    assert_eq!(t.of(Breach), (1, true));
    assert_eq!(t.of(Charge), (1, true));
    assert_eq!(t.of(Demolition), (1, false));
}

#[test]
fn a_radius_two_breach_destroys_its_diamond_and_nothing_else() {
    let t = tuned(|t| t.breach.radius = 2);
    let blast = resolve_blast(
        &around7(G, BrickClass::Explosive(Breach)),
        cell(3, 3),
        Breach,
        &t,
    );
    let mut hit: Vec<BrickCell> = blast.hits.keys().copied().collect();
    hit.sort();
    let mut expected: Vec<BrickCell> = (0..7)
        .flat_map(|r| (0..7).map(move |c| cell(r, c)))
        .filter(|c| {
            let d = c.row.abs_diff(3) + c.col.abs_diff(3);
            (1..=2).contains(&d)
        })
        .collect();
    expected.sort();
    assert_eq!(hit, expected, "Manhattan distance ≤ 2: 12 cells");
    assert_eq!(expected.len(), 12);
    assert!(
        blast.hits.values().all(|h| h.left == 0),
        "breach destroys outright"
    );
}

#[test]
fn a_radius_two_charge_hits_the_whole_five_by_five_square() {
    let t = tuned(|t| t.charge.radius = 2);
    let blast = resolve_blast(
        &around7(T, BrickClass::Explosive(Charge)),
        cell(3, 3),
        Charge,
        &t,
    );
    let mut hit: Vec<BrickCell> = blast.hits.keys().copied().collect();
    hit.sort();
    let mut expected: Vec<BrickCell> = (1..=5)
        .flat_map(|r| (1..=5).map(move |c| cell(r, c)))
        .filter(|&c| c != cell(3, 3))
        .collect();
    expected.sort();
    assert_eq!(
        hit, expected,
        "Chebyshev distance ≤ 2: the 24 cells round it"
    );
    assert!(
        blast.hits.values().all(|h| h.removed == 1),
        "charge still hits once"
    );
}

#[test]
fn demolition_chains_only_when_tuned_to() {
    // A demolition with a charge next to it.
    let g = grid(&[&[
        Some(BrickClass::Explosive(Demolition)),
        Some(BrickClass::Explosive(Charge)),
        Some(C),
    ]]);
    let contained = resolve_blast(&g, cell(0, 0), Demolition, &BlastTuning::default());
    assert_eq!(
        contained.explosions,
        [cell(0, 0)],
        "default: the charge is destroyed, not set off"
    );
    assert!(
        !contained.hits.contains_key(&cell(0, 2)),
        "so its own blast never reaches (0, 2)"
    );

    let chaining = resolve_blast(
        &g,
        cell(0, 0),
        Demolition,
        &tuned(|t| t.demolition.chains = true),
    );
    assert_eq!(
        chaining.explosions,
        [cell(0, 0), cell(0, 1)],
        "now it goes off"
    );
    assert_eq!(
        chaining.hits[&cell(0, 2)].left,
        0,
        "and its blast destroys (0, 2)"
    );

    let quiet_charge = resolve_blast(
        &grid(&[&[
            Some(BrickClass::Explosive(Charge)),
            Some(BrickClass::Explosive(Charge)),
        ]]),
        cell(0, 0),
        Charge,
        &tuned(|t| t.charge.chains = false),
    );
    assert_eq!(
        quiet_charge.explosions,
        [cell(0, 0)],
        "charge can be made not to chain"
    );
}
