use super::*;
use crate::bricks::grid::{BrickHealth, BrickMaxHits};
use crate::bricks::regen::RegenTimer;
use crate::bricks::BrickClass;
use crate::levels::{parse_level, MAX_ROWS};
use crate::powerups::PowerUpBrick;
use crate::test_support::*;
use crate::world::PLAYFIELD_HEIGHT;

fn cell(row: usize, col: usize) -> BrickCell {
    BrickCell { row, col }
}

fn level_app(grid: &str) -> App {
    app_with_level(parse_level(grid).unwrap_or_else(|e| panic!("{e}")))
}

/// Every brick and its cell.
fn cells(app: &mut App) -> Vec<(Entity, BrickCell)> {
    app.world_mut()
        .query_filtered::<(Entity, &BrickCell), With<Brick>>()
        .iter(app.world())
        .map(|(e, c)| (e, *c))
        .collect()
}

fn brick_in(app: &mut App, at: BrickCell) -> Entity {
    cells(app)
        .into_iter()
        .find(|(_, c)| *c == at)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("no brick at {at:?}"))
}

fn collect_collapse(app: &mut App) {
    app.world_mut().trigger(PowerUpCollected {
        kind: PowerUpKind::Collapse,
    });
    app.world_mut().flush();
}

/// Enough updates (100 ms each) for any collapse to finish.
fn settle(app: &mut App) {
    for _ in 0..10 {
        app.update();
    }
}

#[derive(Resource, Default)]
struct Landings(usize);

fn count_landings(app: &mut App) {
    app.init_resource::<Landings>()
        .add_observer(|_: On<BrickLanded>, mut n: ResMut<Landings>| n.0 += 1);
}

fn ball_entity(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<Ball>>()
        .single(app.world())
        .unwrap()
}

fn place_ball(app: &mut App, at: Vec3) {
    let ball = ball_entity(app);
    app.world_mut()
        .get_mut::<Transform>(ball)
        .unwrap()
        .translation = at;
}

#[test]
fn collapse_moves_stack_each_column_on_the_bottom_row_in_order() {
    // CC.T
    // .T.T
    // G..T
    // ...T
    let board = [
        cell(0, 0),
        cell(0, 1),
        cell(0, 3),
        cell(1, 1),
        cell(1, 3),
        cell(2, 0),
        cell(2, 3),
        cell(3, 3),
    ];
    assert_eq!(
        collapse_moves(&board, 4),
        [
            (cell(0, 0), cell(2, 0)),
            (cell(2, 0), cell(3, 0)),
            (cell(0, 1), cell(2, 1)),
            (cell(1, 1), cell(3, 1)),
        ]
    );
    // Empty bottom rows count: the grid is 3 rows tall.
    assert_eq!(collapse_moves(&[cell(0, 0)], 3), [(cell(0, 0), cell(2, 0))]);
    // Already packed: nothing moves.
    assert!(collapse_moves(&[cell(1, 0), cell(2, 0)], 3).is_empty());
}

#[test]
fn even_the_tallest_collapse_finishes_in_time_and_lower_bricks_start_first() {
    let rows = MAX_ROWS;
    // The worst column: one brick in every row but the bottom, plus gaps.
    for board in [
        (0..rows - 1).map(|r| cell(r, 0)).collect::<Vec<_>>(),
        (0..rows - 1).step_by(2).map(|r| cell(r, 0)).collect(),
        vec![cell(0, 0)],
    ] {
        let moves = collapse_moves(&board, rows);
        let lowest = moves.iter().map(|(f, _)| f.row).max().unwrap();
        let total = moves
            .iter()
            .map(|(f, t)| fall_delay(f.row, lowest) + fall_secs(t.row - f.row))
            .fold(0.0, f32::max)
            + BOUNCE_SECS;
        assert!(total <= MAX_COLLAPSE_SECS, "{total} s for {board:?}");
    }
    assert_eq!(fall_delay(5, 5), 0.0);
    assert!(fall_delay(2, 5) > fall_delay(4, 5));
    assert!(fall_secs(8) >= fall_secs(1));
}

#[test]
fn collecting_collapse_compacts_each_column_to_the_grid_bottom() {
    let mut app = level_app("grid:\nCC.T\n.T.T\nG..T\n...T");
    count_landings(&mut app);
    let moved = [
        (brick_in(&mut app, cell(0, 0)), cell(2, 0)),
        (brick_in(&mut app, cell(2, 0)), cell(3, 0)),
        (brick_in(&mut app, cell(0, 1)), cell(2, 1)),
        (brick_in(&mut app, cell(1, 1)), cell(3, 1)),
    ];
    let packed: Vec<_> = (0..4).map(|r| brick_in(&mut app, cell(r, 3))).collect();

    collect_collapse(&mut app);
    for (brick, to) in moved {
        assert_eq!(
            app.world().get::<BrickCell>(brick),
            Some(&to),
            "cell set at once"
        );
        assert!(
            app.world().get::<ColliderDisabled>(brick).is_some(),
            "not solid while falling"
        );
        assert!(app.world().get::<Falling>(brick).is_some());
    }
    for (row, &brick) in packed.iter().enumerate() {
        assert_eq!(app.world().get::<BrickCell>(brick), Some(&cell(row, 3)));
        assert!(app.world().get::<Falling>(brick).is_none());
        assert!(app.world().get::<ColliderDisabled>(brick).is_none());
    }

    settle(&mut app);
    assert_eq!(cells(&mut app).len(), 8, "no brick lost");
    for (brick, at) in cells(&mut app) {
        assert_eq!(translation(&app, brick), brick_translation(at, 4), "{at:?}");
        assert!(app.world().get::<Falling>(brick).is_none());
        assert!(
            app.world().get::<ColliderDisabled>(brick).is_none(),
            "{at:?} solid again"
        );
    }
    assert_eq!(app.world().resource::<Landings>().0, 4);
}

#[test]
fn other_power_ups_do_not_collapse_the_board() {
    let mut app = level_app("grid:\nC.\n.C");
    let before = cells(&mut app);
    app.world_mut().trigger(PowerUpCollected {
        kind: PowerUpKind::SuperSizer,
    });
    app.world_mut().flush();
    settle(&mut app);
    let mut after = cells(&mut app);
    let mut before = before;
    before.sort_by_key(|(e, _)| *e);
    after.sort_by_key(|(e, _)| *e);
    assert_eq!(before, after);
}

#[test]
fn a_collapse_keeps_each_bricks_class_health_and_power_up() {
    let mut app = level_app("legend:\nh = titanium hits=4\ngrid:\nPhRC\n....");
    let reactor = brick_in(&mut app, cell(0, 0));
    let heavy = brick_in(&mut app, cell(0, 1));
    let regen = brick_in(&mut app, cell(0, 2));
    for brick in [reactor, heavy, regen] {
        hit_moving(&mut app, brick, Vec2::new(0.0, -450.0));
    }
    type Snapshot = (BrickClass, u8, u8, bool, bool);
    let snapshot = |app: &App, brick: Entity| -> Snapshot {
        let e = app.world().entity(brick);
        (
            *e.get::<BrickClass>().unwrap(),
            e.get::<BrickHealth>().unwrap().0,
            e.get::<BrickMaxHits>().unwrap().0,
            e.contains::<PowerUpBrick>(),
            e.contains::<RegenTimer>(),
        )
    };
    let all = cells(&mut app);
    let before: Vec<Snapshot> = all.iter().map(|(e, _)| snapshot(&app, *e)).collect();
    assert_eq!(
        snapshot(&app, heavy),
        (BrickClass::Titanium, 3, 4, false, false)
    );
    assert!(snapshot(&app, regen).4, "the hit regen brick is healing");
    let reactor_kind = app.world().get::<PowerUpBrick>(reactor).unwrap().kind;

    collect_collapse(&mut app);
    settle(&mut app);
    let after: Vec<Snapshot> = all.iter().map(|(e, _)| snapshot(&app, *e)).collect();
    assert_eq!(before, after);
    assert_eq!(
        app.world().get::<PowerUpBrick>(reactor).unwrap().kind,
        reactor_kind
    );
    assert!(
        cells(&mut app).iter().all(|(_, c)| c.row == 1),
        "all on row 1"
    );
}

#[test]
fn a_brick_landing_on_the_ball_waits_until_the_ball_leaves() {
    let mut app = level_app("grid:\nC\n.\n.");
    tap(&mut app, KeyCode::Space); // served: no longer held on the paddle
    let brick = brick_in(&mut app, cell(0, 0));
    let landing = brick_translation(cell(2, 0), 1);
    place_ball(&mut app, landing);

    collect_collapse(&mut app);
    for _ in 0..10 {
        place_ball(&mut app, landing); // no physics here: hold it in place
        app.update();
    }
    assert_eq!(translation(&app, brick), landing, "it landed");
    assert!(
        app.world().get::<ColliderDisabled>(brick).is_some(),
        "not solid on the ball"
    );
    assert!(app.world().get::<AwaitingClearance>(brick).is_some());

    place_ball(&mut app, Vec3::new(0.0, -PLAYFIELD_HEIGHT / 4.0, 0.0));
    app.update();
    assert!(
        app.world().get::<ColliderDisabled>(brick).is_none(),
        "solid once clear"
    );
    assert!(app.world().get::<AwaitingClearance>(brick).is_none());
}

#[test]
fn a_brick_lands_solid_when_the_ball_is_elsewhere() {
    let mut app = level_app("grid:\nC\n.\n.");
    tap(&mut app, KeyCode::Space);
    place_ball(&mut app, Vec3::new(0.0, -PLAYFIELD_HEIGHT / 4.0, 0.0));
    let brick = brick_in(&mut app, cell(0, 0));
    collect_collapse(&mut app);
    for _ in 0..10 {
        place_ball(&mut app, Vec3::new(0.0, -PLAYFIELD_HEIGHT / 4.0, 0.0));
        app.update();
    }
    assert!(app.world().get::<ColliderDisabled>(brick).is_none());
    assert!(app.world().get::<AwaitingClearance>(brick).is_none());
}

#[test]
fn pausing_freezes_a_collapse_mid_fall() {
    let mut app = level_app("grid:\nC\n.\n.\n.");
    let brick = brick_in(&mut app, cell(0, 0));
    collect_collapse(&mut app);
    app.update();
    app.update();
    tap(&mut app, KeyCode::KeyP);
    let at = translation(&app, brick);
    let elapsed = app.world().get::<Falling>(brick).unwrap().elapsed;
    assert_ne!(at, brick_translation(cell(3, 0), 1), "still mid-fall");
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(translation(&app, brick), at);
    assert_eq!(app.world().get::<Falling>(brick).unwrap().elapsed, elapsed);

    tap(&mut app, KeyCode::KeyP);
    settle(&mut app);
    assert_eq!(translation(&app, brick), brick_translation(cell(3, 0), 1));
    assert!(app.world().get::<Falling>(brick).is_none());
}
