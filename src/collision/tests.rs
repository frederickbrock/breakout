use super::*;
use crate::ball::BALL_SPEED;
use crate::bricks::grid::{BRICK_COLS, BRICK_ROWS};
use crate::game_state::{AppState, GameOutcome};
use crate::test_support::*;
use crate::theme;

fn score(app: &App) -> i32 {
    app.world().resource::<Score>().0
}

fn classes() -> [BrickClass; 9] {
    use crate::bricks::ExplosiveKind::*;
    [
        BrickClass::Ceramic,
        BrickClass::Titanium,
        BrickClass::Tungsten,
        BrickClass::Reactor,
        BrickClass::Regen,
        BrickClass::Shield,
        BrickClass::Explosive(Charge),
        BrickClass::Explosive(Breach),
        BrickClass::Explosive(Demolition),
    ]
}

/// A ball moving down onto a brick (the only way shield glass breaks).
const FROM_ABOVE: Vec2 = Vec2::new(60.0, -BALL_SPEED);

#[derive(Resource, Default)]
struct Seen {
    damaged: Vec<(Entity, Vec2, BrickClass)>,
    destroyed: Vec<(Entity, Vec2, BrickClass, bool)>,
}

#[test]
fn each_class_breaks_after_its_hit_count() {
    for class in classes() {
        let mut app = app();
        let brick = brick_of(&mut app, class);
        let max = class.max_hits();
        for i in 1..max {
            hit_moving(&mut app, brick, FROM_ABOVE);
            assert_eq!(
                app.world().get::<BrickHealth>(brick).unwrap().0,
                max - i,
                "{class:?}"
            );
            assert_eq!(
                app.world().get::<Sprite>(brick).unwrap().color,
                theme::brick_color(class),
                "{class:?} keeps its colour (damage shows as particles)"
            );
            assert_eq!(score(&app), 10 * i as i32);
            assert!(!app.world().resource::<BallCollisionSignals>().broke_brick);
            // One contact is one hit: nothing more happens on later frames.
            for _ in 0..3 {
                app.update();
            }
            assert_eq!(app.world().get::<BrickHealth>(brick).unwrap().0, max - i);
        }
        hit_moving(&mut app, brick, FROM_ABOVE);
        assert!(app.world().get_entity(brick).is_err(), "{class:?} broke");
        // An explosive also blasts its neighbours (bricks::explosive tests).
        if !matches!(class, BrickClass::Explosive(_)) {
            assert_eq!(score(&app), 10 * max as i32, "{class:?}");
            assert_eq!(bricks(&mut app).len(), BRICK_ROWS * BRICK_COLS - 1);
        }
    }
}

#[test]
fn clearing_every_brick_including_multi_hit_ones_wins() {
    let mut app = app();
    tap(&mut app, KeyCode::Space);
    let a = brick_of(&mut app, BrickClass::Ceramic);
    let b = brick_of(&mut app, BrickClass::Titanium);
    for brick in bricks(&mut app) {
        if brick != a && brick != b {
            app.world_mut().despawn(brick);
        }
    }

    hit(&mut app, a);
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::InGame, "one brick is left");

    hit(&mut app, b);
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::InGame, "a damaged brick is left");

    hit(&mut app, b);
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(
        app.world().get_resource::<GameOutcome>(),
        Some(&GameOutcome::Won)
    );
}

#[derive(Resource, Default)]
struct Deflects(Vec<Vec2>);

#[test]
fn shield_glass_hit_from_below_or_the_side_only_deflects() {
    let mut app = app();
    app.init_resource::<Deflects>().add_observer(
        |on: On<ShieldDeflected>, mut seen: ResMut<Deflects>| {
            seen.0.push(on.position);
        },
    );
    let shield = brick_of(&mut app, BrickClass::Shield);
    let ball = ball(&mut app);
    let ball_at = translation(&app, ball).truncate();

    hit_moving(&mut app, shield, Vec2::new(0.0, BALL_SPEED));
    assert_eq!(app.world().get::<BrickHealth>(shield).unwrap().0, 1);
    assert_eq!(score(&app), 0);
    assert!(!app.world().resource::<BallCollisionSignals>().broke_brick);
    // No flat flash any more: the glass keeps its colour.
    assert_eq!(
        app.world().get::<Sprite>(shield).unwrap().color,
        theme::SHIELD
    );

    // A flat side hit: no damage either.
    hit_moving(&mut app, shield, Vec2::new(BALL_SPEED, 0.0));
    assert!(app.world().get_entity(shield).is_ok());
    assert_eq!(score(&app), 0);
    // Each deflect is reported at the contact point (the ball).
    assert_eq!(app.world().resource::<Deflects>().0, [ball_at, ball_at]);
}

#[test]
fn shield_glass_hit_by_a_ball_moving_down_breaks() {
    let mut app = app();
    app.init_resource::<Deflects>().add_observer(
        |on: On<ShieldDeflected>, mut seen: ResMut<Deflects>| {
            seen.0.push(on.position);
        },
    );
    let shield = brick_of(&mut app, BrickClass::Shield);
    hit_moving(&mut app, shield, FROM_ABOVE);
    assert!(app.world().resource::<Deflects>().0.is_empty());
    assert!(app.world().get_entity(shield).is_err());
    assert_eq!(score(&app), 10);
    assert!(app.world().resource::<BallCollisionSignals>().broke_brick);
}

#[test]
fn the_run_is_won_only_once_the_last_shield_breaks_from_above() {
    let mut app = app();
    tap(&mut app, KeyCode::Space);
    let ceramic = brick_of(&mut app, BrickClass::Ceramic);
    let shield = brick_of(&mut app, BrickClass::Shield);
    for brick in bricks(&mut app) {
        if brick != ceramic && brick != shield {
            app.world_mut().despawn(brick);
        }
    }
    hit(&mut app, ceramic);
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::InGame);

    hit_moving(&mut app, shield, Vec2::new(0.0, BALL_SPEED));
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::InGame, "the shield survived");

    hit_moving(&mut app, shield, FROM_ABOVE);
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(
        app.world().get_resource::<GameOutcome>(),
        Some(&GameOutcome::Won)
    );
}

#[test]
fn brick_damage_and_break_events_carry_class_and_position() {
    let mut app = app();
    app.init_resource::<Seen>()
        .add_observer(|on: On<BrickDamaged>, mut seen: ResMut<Seen>| {
            seen.damaged.push((on.brick, on.position, on.class));
        })
        .add_observer(|on: On<BrickDestroyed>, mut seen: ResMut<Seen>| {
            seen.destroyed
                .push((on.brick, on.position, on.class, on.by_blast));
        });
    let titanium = brick_of(&mut app, BrickClass::Titanium);
    let centre = translation(&app, titanium).truncate();
    let ball = ball(&mut app);
    let ball_at = translation(&app, ball).truncate();

    hit(&mut app, titanium);
    hit(&mut app, titanium);
    let seen = app.world().resource::<Seen>();
    // The surviving hit: at the contact point (the ball), with its class.
    assert_eq!(seen.damaged, [(titanium, ball_at, BrickClass::Titanium)]);
    // The break: at the brick's centre, by the ball.
    assert_eq!(
        seen.destroyed,
        [(titanium, centre, BrickClass::Titanium, false)]
    );
}
