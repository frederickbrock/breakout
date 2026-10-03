use super::*;
use crate::ball::{BallSpeed, BALL_SPEED, BALL_SPEED_SCALE};
use crate::bricks::grid::{
    brick_translation, Brick, BrickHealth, BrickMaxHits, CarriesPowerUp, BRICK_WIDTH, SIDE_CHANNEL,
};
use crate::bricks::BrickCell;
use crate::game_state::{AppState, GameOutcome};
use crate::powerups::{PowerUp, PowerUpBrick};
use crate::run::{Lives, Score};
use crate::test_support::*;
use crate::world::PLAYFIELD_WIDTH;
use avian2d::prelude::LinearVelocity;

fn level(text: &str) -> LevelDef {
    match parse_level(text) {
        Ok(def) => def,
        Err(e) => panic!("{e}"),
    }
}

/// Every brick's cell and class, sorted.
fn layout(app: &mut App) -> Vec<(BrickCell, BrickClass)> {
    let mut bricks: Vec<_> = app
        .world_mut()
        .query_filtered::<(&BrickCell, &BrickClass), With<Brick>>()
        .iter(app.world())
        .map(|(cell, class)| (*cell, *class))
        .collect();
    bricks.sort_by_key(|(cell, _)| *cell);
    bricks
}

/// The brick in `cell`.
fn brick_at(app: &mut App, row: usize, col: usize) -> Entity {
    app.world_mut()
        .query::<(Entity, &BrickCell)>()
        .iter(app.world())
        .find(|(_, c)| **c == BrickCell { row, col })
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("no brick at ({row},{col})"))
}

/// Loses the run's last life, then presses R for a new run.
fn restart(app: &mut App) {
    app.world_mut().resource_mut::<Lives>().0 = 1;
    tap(app, KeyCode::Space);
    move_ball_below_screen(app);
    app.update();
    app.update();
    assert_eq!(app_state(app), AppState::GameOver);
    tap(app, KeyCode::KeyR);
    assert_eq!(app_state(app), AppState::InGame);
}

#[test]
fn the_shipped_random_level_is_the_fallback_board() {
    assert_eq!(
        parse_level(include_str!("../../assets/levels/01-random.level")),
        Ok(LevelDef::fallback())
    );
}

#[test]
fn the_shipped_campaign_starts_with_the_random_level() {
    let campaign = parse_campaign(include_str!("../../assets/levels/campaign.txt"));
    assert_eq!(
        campaign.levels.first().map(String::as_str),
        Some("01-random.level")
    );
}

#[test]
fn the_default_ball_speed_is_the_ball_speed_constant() {
    assert_eq!(
        BallSpeed::from_factor(BALL_SPEED_SCALE),
        BallSpeed::default()
    );
    assert_eq!(BallSpeed::default().0, 540.0);
    assert_eq!(BallSpeed::from_factor(2.0).0, 600.0);
}

#[test]
fn a_hand_written_shape_spawns_exactly_that_shape_centred() {
    let rows = ["...CC...", "..TXXT..", ".CSSSSC.", "..TXXT..", "...CC..."];
    let def = level(&format!("grid:\n{}", rows.join("\n")));
    let mut expected = Vec::new();
    for (row, line) in rows.iter().enumerate() {
        for (col, symbol) in line.chars().enumerate() {
            let class = match symbol {
                'C' => BrickClass::Ceramic,
                'T' => BrickClass::Titanium,
                'X' => BrickClass::Explosive(crate::bricks::ExplosiveKind::Charge),
                'S' => BrickClass::Shield,
                _ => continue,
            };
            expected.push((BrickCell { row, col }, class));
        }
    }
    let mut app = app_with_level(def);
    assert_eq!(layout(&mut app), expected);

    let world = app.world_mut();
    let (mut left, mut right) = (f32::INFINITY, f32::NEG_INFINITY);
    for (cell, transform) in world
        .query_filtered::<(&BrickCell, &Transform), With<Brick>>()
        .iter(world)
    {
        assert_eq!(transform.translation, brick_translation(*cell, 8));
        left = left.min(transform.translation.x - BRICK_WIDTH / 2.0);
        right = right.max(transform.translation.x + BRICK_WIDTH / 2.0);
    }
    // The outermost bricks are in columns 1 and 6 of 8, so the shape is
    // centred too.
    let left_channel = left + PLAYFIELD_WIDTH / 2.0;
    let right_channel = PLAYFIELD_WIDTH / 2.0 - right;
    assert!((left_channel - right_channel).abs() <= 0.5);
    assert!(left_channel >= SIDE_CHANNEL);
}

#[test]
fn a_hits_override_needs_that_many_hits() {
    let mut app = app_with_level(level("legend:\nk = titanium hits=4\ngrid:\nk"));
    let brick = brick_at(&mut app, 0, 0);
    assert_eq!(app.world().get::<BrickHealth>(brick).map(|h| h.0), Some(4));
    assert_eq!(app.world().get::<BrickMaxHits>(brick).map(|m| m.0), Some(4));
    tap(&mut app, KeyCode::Space);
    for left in [3, 2, 1] {
        hit(&mut app, brick);
        assert_eq!(
            app.world().get::<BrickHealth>(brick).map(|h| h.0),
            Some(left)
        );
    }
    hit(&mut app, brick);
    assert!(app.world().get_entity(brick).is_err());
    assert_eq!(app.world().resource::<Score>().0, 40);
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(
        app.world().get_resource::<GameOutcome>(),
        Some(&GameOutcome::Won)
    );
}

#[test]
fn a_powerup_flagged_ceramic_drops_a_power_up() {
    let mut app = app_with_level(level("legend:\nv = ceramic powerup\ngrid:\nvC"));
    let (v, c) = (brick_at(&mut app, 0, 0), brick_at(&mut app, 0, 1));
    assert_eq!(app.world().get::<BrickClass>(v), Some(&BrickClass::Ceramic));
    assert!(app.world().entity(v).contains::<CarriesPowerUp>());
    assert!(app.world().entity(v).contains::<PowerUpBrick>());
    assert!(!app.world().entity(c).contains::<CarriesPowerUp>());
    assert!(!app.world().entity(c).contains::<PowerUpBrick>());
    tap(&mut app, KeyCode::Space);
    hit(&mut app, v);
    assert!(app.world().get_entity(v).is_err());
    assert_eq!(count::<With<PowerUp>>(&mut app), 1);
}

#[test]
fn a_level_speed_factor_sets_the_serve_speed() {
    let mut app = app_with_level(level("speed_factor: 2.0\ngrid:\nC"));
    let speed = 600.0;
    assert_eq!(app.world().resource::<BallSpeed>().0, speed);
    tap(&mut app, KeyCode::Space);
    let ball = ball(&mut app);
    let v = app.world().get::<LinearVelocity>(ball).map(|v| v.0);
    assert!(
        v.is_some_and(|v| (v.length() - speed).abs() < 1e-2),
        "{v:?}"
    );
}

#[test]
fn without_a_level_the_run_uses_the_fallback() {
    let mut app = app();
    assert!(app.world().get_resource::<CurrentLevel>().is_none());
    assert_eq!(count::<With<Brick>>(&mut app), 70);
    assert_eq!(count::<With<PowerUpBrick>>(&mut app), 6);
    assert_eq!(count::<With<CarriesPowerUp>>(&mut app), 0);
    assert_eq!(app.world().resource::<BallSpeed>().0, BALL_SPEED);
}

#[test]
fn a_changed_level_applies_at_the_next_run_not_mid_board() {
    let mut app = app_with_level(level("speed_factor: 1.0\ngrid:\nCC"));
    app.insert_resource(CurrentLevel(level("speed_factor: 2.0\ngrid:\nCCC")));
    app.update();
    assert_eq!(count::<With<Brick>>(&mut app), 2, "not mid-board");
    assert_eq!(app.world().resource::<BallSpeed>().0, 300.0);
    restart(&mut app);
    assert_eq!(count::<With<Brick>>(&mut app), 3);
    assert_eq!(app.world().resource::<BallSpeed>().0, 600.0);
}

#[test]
fn removing_the_current_level_falls_back_to_the_random_board() {
    let mut app = app_with_level(level("grid:\nCC"));
    app.world_mut().remove_resource::<CurrentLevel>();
    restart(&mut app);
    assert_eq!(count::<With<Brick>>(&mut app), 70);
    assert_eq!(count::<With<PowerUpBrick>>(&mut app), 6);
}
