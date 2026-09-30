//! Explosive bricks: when one is destroyed by the ball, its blast hits its
//! neighbours by grid cell, and chains resolve within the same frame.
//!
//! - **Charge:** the 8 surrounding bricks each take 1 hit. Explosives it
//!   destroys explode too.
//! - **Breach:** the 4 orthogonal neighbours are destroyed outright. Explosives
//!   it destroys explode too.
//! - **Demolition:** the 8 surrounding bricks are destroyed outright;
//!   explosives caught in it do *not* explode.
//!
//! The chain is worked out by the pure [`resolve_blast`] on a snapshot of the
//! board, so every brick loses each hit point at most once and nothing is
//! scored twice. [`explode`] then applies the result through the same path as
//! a ball break: `BrickDestroyed` (power-up drops), score, `broke_brick` (the
//! win check), and `BrickDamaged` for survivors (regen's heal timer). Blasts
//! ignore shield glass's "from above" rule.

use super::grid::{Brick, BrickHealth, BRICK_HEIGHT, BRICK_WIDTH};
use super::{BrickCell, BrickClass, ExplosiveKind};
use crate::collision::{BallCollisionSignals, BrickDamaged, BrickDestroyed};
use crate::game_state::{AppState, PlayState};
use crate::run::Score;
use crate::theme;
use bevy::prelude::*;
use std::collections::{BTreeMap, VecDeque};

/// What the board looks like to a blast: each live brick's class and hits
/// left, by cell.
pub type BlastGrid = BTreeMap<BrickCell, (BrickClass, u8)>;

/// What a blast chain does to one brick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlastHit {
    /// Hit points the chain removed (each scores 10).
    pub removed: u8,
    /// Hits left afterwards; 0 means destroyed.
    pub left: u8,
}

/// The outcome of one explosion and everything it chains into.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Blast {
    pub hits: BTreeMap<BrickCell, BlastHit>,
    /// Every cell that exploded, in order: the origin first.
    pub explosions: Vec<BrickCell>,
}

/// The cells a `kind` blast at `cell` reaches.
fn reach(cell: BrickCell, kind: ExplosiveKind) -> Vec<BrickCell> {
    let offsets: &[(i64, i64)] = match kind {
        ExplosiveKind::Breach => &[(-1, 0), (1, 0), (0, -1), (0, 1)],
        ExplosiveKind::Charge | ExplosiveKind::Demolition => &[
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
        ],
    };
    offsets
        .iter()
        .filter_map(|&(dr, dc)| {
            let row = usize::try_from(cell.row as i64 + dr).ok()?;
            let col = usize::try_from(cell.col as i64 + dc).ok()?;
            Some(BrickCell { row, col })
        })
        .collect()
}

/// Resolves the blast of a `kind` explosive destroyed at `origin`, including
/// every chained explosion, on `grid` (which need not contain the origin).
/// Bricks with 0 hits left count as already gone.
pub fn resolve_blast(grid: &BlastGrid, origin: BrickCell, kind: ExplosiveKind) -> Blast {
    let mut grid = grid.clone();
    grid.remove(&origin);
    let mut blast = Blast {
        explosions: vec![origin],
        ..default()
    };
    let mut pending = VecDeque::from([(origin, kind)]);
    while let Some((cell, kind)) = pending.pop_front() {
        for target in reach(cell, kind) {
            let Some((class, left)) = grid.get_mut(&target) else {
                continue;
            };
            if *left == 0 {
                continue;
            }
            let removed = match kind {
                ExplosiveKind::Charge => 1,
                ExplosiveKind::Breach | ExplosiveKind::Demolition => *left,
            };
            *left -= removed;
            let hit = blast.hits.entry(target).or_insert(BlastHit {
                removed: 0,
                left: 0,
            });
            hit.removed += removed;
            hit.left = *left;
            if *left == 0 {
                if let BrickClass::Explosive(next) = *class {
                    // Demolition destroys explosives without setting them off.
                    if kind != ExplosiveKind::Demolition {
                        blast.explosions.push(target);
                        pending.push_back((target, next));
                    }
                }
            }
        }
    }
    blast
}

/// Everything [`explode`] reads and changes on each brick.
type BlastTarget = (
    Entity,
    &'static BrickCell,
    &'static BrickClass,
    &'static mut BrickHealth,
    &'static Transform,
);

/// Fired once per explosion: the ball-destroyed origin, then every explosive
/// its chain sets off, in order. Visuals (the placeholder flash here, later
/// the particles epic) observe this; gameplay effects are already applied.
#[derive(Event, Debug, Clone, Copy, PartialEq)]
pub struct BrickExploded {
    pub cell: BrickCell,
    pub position: Vec2,
    pub kind: ExplosiveKind,
}

/// How long the placeholder burst lasts, and how far it grows.
const FLASH_SECS: f32 = 0.2;
const FLASH_GROWTH: f32 = 1.5;

/// A quick expanding, fading flash where an explosive went off.
#[derive(Component)]
pub struct BlastFlash(Timer);

pub struct ExplosivePlugin;

impl Plugin for ExplosivePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(explode)
            .add_observer(flash_on_explosion)
            .add_systems(
                Update,
                fade_blast_flashes.run_if(in_state(PlayState::Playing)),
            );
    }
}

/// A ball-destroyed explosive sets off its blast chain. Blast-destroyed
/// bricks (`by_blast`) are ignored here: [`resolve_blast`] already chained
/// them.
fn explode(
    on: On<BrickDestroyed>,
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut signals: ResMut<BallCollisionSignals>,
    classes: Query<(&BrickCell, &BrickClass)>,
    mut bricks: Query<BlastTarget, With<Brick>>,
) {
    if on.by_blast {
        return;
    }
    let Ok((&origin, &BrickClass::Explosive(kind))) = classes.get(on.brick) else {
        return;
    };
    let grid: BlastGrid = bricks
        .iter()
        .map(|(_, cell, class, health, _)| (*cell, (*class, health.0)))
        .collect();
    let blast = resolve_blast(&grid, origin, kind);

    let mut positions = BTreeMap::new();
    positions.insert(origin, on.position);
    for (entity, cell, class, mut health, transform) in &mut bricks {
        let position = transform.translation.truncate();
        positions.insert(*cell, position);
        let Some(hit) = blast.hits.get(cell) else {
            continue;
        };
        score.0 += 10 * i32::from(hit.removed);
        health.0 = hit.left;
        if hit.left == 0 {
            commands.trigger(BrickDestroyed {
                brick: entity,
                position,
                class: *class,
                by_blast: true,
            });
            commands.entity(entity).despawn();
            signals.broke_brick = true;
        } else {
            commands.trigger(BrickDamaged {
                brick: entity,
                position,
                class: *class,
            });
        }
    }
    // One event per explosion, the origin first, then each chained one.
    for cell in &blast.explosions {
        let kind = if *cell == origin {
            kind
        } else {
            match grid.get(cell) {
                Some((BrickClass::Explosive(kind), _)) => *kind,
                _ => continue,
            }
        };
        if let Some(&position) = positions.get(cell) {
            commands.trigger(BrickExploded {
                cell: *cell,
                position,
                kind,
            });
        }
    }
}

/// Placeholder burst for each explosion; the particles epic's explosions
/// task replaces it by observing [`BrickExploded`] too.
fn flash_on_explosion(on: On<BrickExploded>, mut commands: Commands) {
    commands.spawn((
        BlastFlash(Timer::from_seconds(FLASH_SECS, TimerMode::Once)),
        Sprite::from_color(theme::BLAST_FLASH, Vec2::new(BRICK_WIDTH, BRICK_HEIGHT)),
        Transform::from_translation(on.position.extend(0.8)),
        DespawnOnExit(AppState::InGame),
    ));
}

fn fade_blast_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut flashes: Query<(Entity, &mut BlastFlash, &mut Transform, &mut Sprite)>,
) {
    for (entity, mut flash, mut transform, mut sprite) in &mut flashes {
        if flash.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let t = flash.0.fraction();
        transform.scale = Vec3::splat(1.0 + t * FLASH_GROWTH);
        sprite.color = theme::BLAST_FLASH.with_alpha(1.0 - t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let blast = resolve_blast(&g, cell(2, 2), Charge);
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
        let blast = resolve_blast(&g, cell(2, 2), Demolition);
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
        let blast = resolve_blast(&g, cell(0, 0), Charge);
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
        let blast = resolve_blast(&g, cell(1, 1), Charge);
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
        let blast = resolve_blast(&g, cell(0, 0), Charge);
        assert_eq!(blast.hits.len(), 2);
        assert!(!blast.hits.contains_key(&cell(1, 1)));
    }

    mod in_game {
        use super::super::*;
        use crate::ball::{Ball, BallApproach};
        use crate::bricks::regen::RegenTimer;
        use crate::bricks::ExplosiveKind::*;
        use crate::game_state::GameOutcome;
        use crate::powerups::{PowerUp, PowerUpBrick};
        use crate::test_support::*;

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
                    },
                    BrickExploded {
                        cell: BrickCell { row: 3, col: 5 },
                        position: breach_at,
                        kind: Breach,
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

        #[test]
        fn each_explosion_flashes_briefly() {
            let mut app = app();
            isolate(&mut app, 3, 4);
            isolate(&mut app, 3, 5);
            let charge = set(&mut app, 3, 4, BrickClass::Explosive(Charge));
            set(&mut app, 3, 5, BrickClass::Explosive(Breach));
            detonate(&mut app, charge);
            app.world_mut().flush();
            assert_eq!(count::<With<BlastFlash>>(&mut app), 2, "one per explosion");
            for _ in 0..3 {
                app.update();
            }
            assert_eq!(count::<With<BlastFlash>>(&mut app), 0);
        }
    }
}
