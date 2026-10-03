use super::*;
use crate::bricks::grid::{BrickHealth, BRICK_COLS, BRICK_ROWS};
use crate::bricks::REACTOR_BRICKS;
use crate::test_support::*;
use crate::theme;

fn spawn_falling_power_up(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            Transform::from_xyz(0.0, 200.0, 0.5),
            PowerUp {
                kind: PowerUpKind::SuperSizer,
                velocity: Vec2::ZERO,
                gravity: BASE_GRAVITY,
            },
            DespawnOnExit(AppState::InGame),
        ))
        .id()
}

fn effect_remaining(app: &App) -> Option<std::time::Duration> {
    app.world()
        .resource::<ActiveEffects>()
        .0
        .first()
        .map(|effect| effect.timer.remaining())
}

#[test]
fn pausing_freezes_falling_power_ups_and_effect_timers() {
    let mut app = app();
    let power_up = spawn_falling_power_up(&mut app);
    app.world_mut()
        .resource_mut::<ActiveEffects>()
        .refresh_or_insert(PowerUpKind::SuperSizer, 5.0);

    tap(&mut app, KeyCode::KeyP);
    let y_paused = app
        .world()
        .get::<Transform>(power_up)
        .unwrap()
        .translation
        .y;
    let remaining_paused = effect_remaining(&app);
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(
        app.world()
            .get::<Transform>(power_up)
            .unwrap()
            .translation
            .y,
        y_paused
    );
    assert_eq!(effect_remaining(&app), remaining_paused);

    tap(&mut app, KeyCode::KeyP);
    app.update();
    assert!(
        app.world()
            .get::<Transform>(power_up)
            .unwrap()
            .translation
            .y
            < y_paused
    );
    assert!(effect_remaining(&app) < remaining_paused);
}

#[test]
fn a_new_run_clears_power_ups_and_active_effects() {
    let mut app = app();
    tap(&mut app, KeyCode::Space);
    spawn_falling_power_up(&mut app);
    app.world_mut()
        .resource_mut::<ActiveEffects>()
        .refresh_or_insert(PowerUpKind::SuperSizer, 5.0);
    app.world_mut().resource_mut::<crate::run::Lives>().0 = 1;
    let mut ball = app
        .world_mut()
        .query_filtered::<&mut Transform, With<crate::ball::Ball>>()
        .single_mut(app.world_mut())
        .unwrap();
    ball.translation.y = -PLAYFIELD_HEIGHT;
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(count::<With<PowerUp>>(&mut app), 0);

    tap(&mut app, KeyCode::KeyR);
    assert_eq!(app_state(&app), AppState::InGame);
    assert!(app.world().resource::<ActiveEffects>().0.is_empty());
    assert_eq!(count::<With<PowerUp>>(&mut app), 0);
}

fn power_up_bricks(app: &mut App) -> Vec<Entity> {
    app.world_mut()
        .query_filtered::<Entity, With<PowerUpBrick>>()
        .iter(app.world())
        .collect()
}

fn power_up_brick_positions(app: &mut App) -> Vec<(i32, i32)> {
    let mut positions: Vec<(i32, i32)> = app
        .world_mut()
        .query_filtered::<&Transform, With<PowerUpBrick>>()
        .iter(app.world())
        .map(|t| (t.translation.x as i32, t.translation.y as i32))
        .collect();
    positions.sort();
    positions
}

fn position(app: &App, entity: Entity) -> Vec2 {
    app.world()
        .get::<Transform>(entity)
        .unwrap()
        .translation
        .truncate()
}

/// Every falling power-up: (entity, position, kind, gravity).
fn falling(app: &mut App) -> Vec<(Entity, Vec2, PowerUpKind, f32)> {
    app.world_mut()
        .query::<(Entity, &Transform, &PowerUp)>()
        .iter(app.world())
        .map(|(e, t, p)| (e, t.translation.truncate(), p.kind, p.gravity))
        .collect()
}

fn break_brick(app: &mut App, brick: Entity) {
    hit(app, brick);
    hit(app, brick);
    assert!(app.world().get_entity(brick).is_err());
}

#[test]
fn every_reactor_brick_and_only_those_carry_a_power_up() {
    let mut app = app();
    assert_eq!(power_up_bricks(&mut app).len(), REACTOR_BRICKS);
    assert_eq!(bricks(&mut app).len(), BRICK_ROWS * BRICK_COLS);

    for brick in bricks(&mut app) {
        let entity = app.world().entity(brick);
        let health = entity.get::<BrickHealth>().unwrap().0;
        let class = *entity.get::<BrickClass>().unwrap();
        match entity.get::<PowerUpBrick>() {
            Some(power_up_brick) => {
                assert_eq!(class, BrickClass::Reactor);
                assert_eq!(health, 2);
                assert_eq!(entity.get::<Sprite>().unwrap().color, theme::REACTOR);
                assert!(power_up_brick.kind == PowerUpKind::SuperSizer);
            }
            None => {
                assert_ne!(class, BrickClass::Reactor);
                assert_eq!(health, class.max_hits());
            }
        }
    }
}

#[test]
fn power_up_bricks_are_rechosen_every_run() {
    let mut app = app();
    let first = power_up_brick_positions(&mut app);

    tap(&mut app, KeyCode::Space);
    app.world_mut().resource_mut::<crate::run::Lives>().0 = 1;
    let ball = app
        .world_mut()
        .query_filtered::<Entity, With<crate::ball::Ball>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .get_mut::<Transform>(ball)
        .unwrap()
        .translation
        .y = -PLAYFIELD_HEIGHT;
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::GameOver);
    tap(&mut app, KeyCode::KeyR);

    let second = power_up_brick_positions(&mut app);
    assert_eq!(second.len(), REACTOR_BRICKS);
    // Same six by chance: 1 in C(70, 6) ≈ 1.3e8.
    assert_ne!(first, second);
}

#[test]
fn a_single_hit_only_cracks_a_power_up_brick() {
    let mut app = app();
    let brick = power_up_bricks(&mut app)[0];
    hit(&mut app, brick);
    for _ in 0..5 {
        app.update();
    }

    let entity = app.world().entity(brick);
    assert_eq!(entity.get::<BrickHealth>().unwrap().0, 1);
    assert_eq!(entity.get::<Sprite>().unwrap().color, theme::REACTOR);
    assert_eq!(app.world().resource::<crate::run::Score>().0, 10);
    assert_eq!(count::<With<PowerUp>>(&mut app), 0);
}

#[test]
fn breaking_a_power_up_brick_drops_its_power_up_where_it_stood() {
    let mut app = app();
    let [first, second] = power_up_bricks(&mut app)[..2] else {
        unreachable!()
    };
    let at = position(&app, first);

    break_brick(&mut app, first);
    assert_eq!(app.world().resource::<crate::run::Score>().0, 20);
    let dropped = falling(&mut app);
    assert_eq!(dropped.len(), 1);
    let (_, pos, kind, gravity) = dropped[0];
    assert_eq!(pos, at);
    assert!(kind == PowerUpKind::SuperSizer);
    assert_eq!(gravity, BASE_GRAVITY);

    // The next drop this run is heavier.
    break_brick(&mut app, second);
    let gravities: Vec<f32> = falling(&mut app).iter().map(|f| f.3).collect();
    assert!(gravities.contains(&(BASE_GRAVITY + GRAVITY_STEP)));

    // A normal (ceramic, one-hit) brick drops nothing.
    let normal = brick_of(&mut app, BrickClass::Ceramic);
    hit(&mut app, normal);
    assert!(app.world().get_entity(normal).is_err());
    assert_eq!(count::<With<PowerUp>>(&mut app), 2);
}

#[test]
fn catching_a_dropped_power_up_widens_the_paddle() {
    let mut app = app();
    let brick = power_up_bricks(&mut app)[0];
    break_brick(&mut app, brick);
    let (power_up, ..) = falling(&mut app)[0];
    let paddle = app
        .world_mut()
        .query_filtered::<Entity, With<Paddle>>()
        .single(app.world())
        .unwrap();
    let paddle_at = app.world().get::<Transform>(paddle).unwrap().translation;
    app.world_mut()
        .get_mut::<Transform>(power_up)
        .unwrap()
        .translation = paddle_at;
    app.update();
    app.update();

    assert_eq!(count::<With<PowerUp>>(&mut app), 0);
    assert!(app
        .world()
        .resource::<ActiveEffects>()
        .is_active(PowerUpKind::SuperSizer));
    assert!(app.world().get::<Paddle>(paddle).unwrap().width > crate::paddle::PADDLE_WIDTH);
}

#[test]
fn no_power_ups_appear_without_breaking_a_power_up_brick() {
    let mut app = app();
    // 30 s at the test app's 100 ms step; the old timer fired every 7-10 s.
    for _ in 0..300 {
        app.update();
    }
    assert_eq!(app_state(&app), AppState::InGame);
    assert_eq!(count::<With<PowerUp>>(&mut app), 0);
}

#[test]
fn abandoning_a_paused_run_for_the_main_menu_clears_power_ups() {
    let mut app = app();
    spawn_falling_power_up(&mut app);
    app.world_mut()
        .resource_mut::<ActiveEffects>()
        .refresh_or_insert(PowerUpKind::SuperSizer, 5.0);

    tap(&mut app, KeyCode::Escape);
    crate::menu::test_helpers::press(&mut app, "Main menu");
    assert_eq!(app_state(&app), AppState::MainMenu);
    assert_eq!(count::<With<PowerUp>>(&mut app), 0);

    crate::menu::test_helpers::press(&mut app, "Start");
    assert_eq!(app_state(&app), AppState::InGame);
    assert!(app.world().resource::<ActiveEffects>().0.is_empty());
    assert_eq!(count::<With<PowerUp>>(&mut app), 0);
}

#[test]
fn flagged_and_reactor_bricks_are_equipped_once() {
    let def = match crate::levels::parse_level("legend:\nv = reactor powerup\ngrid:\nPvC") {
        Ok(def) => def,
        Err(e) => panic!("{e}"),
    };
    let mut app = app_with_level(def);
    assert_eq!(power_up_bricks(&mut app).len(), 2);
    let kinds = |app: &mut App| -> Vec<(Entity, PowerUpKind)> {
        let mut kinds: Vec<_> = app
            .world_mut()
            .query::<(Entity, &PowerUpBrick)>()
            .iter(app.world())
            .map(|(e, b)| (e, b.kind))
            .collect();
        kinds.sort_by_key(|(e, _)| *e);
        kinds
    };
    let before = kinds(&mut app);
    // A second equip pass skips bricks that already carry a power-up.
    app.world_mut().trigger(LevelStarted { index: 0 });
    app.world_mut().flush();
    app.update();
    assert!(before == kinds(&mut app));
    assert_eq!(power_up_bricks(&mut app).len(), 2);
}
