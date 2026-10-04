use super::*;
use crate::controls::{ControlSettings, PaddleControl};
use crate::game_state::PlayState;
use crate::paddle::PADDLE_WIDTH;
use crate::run::{Lives, STARTING_LIVES};
use crate::test_support::*;
use crate::theme;

fn is_anchored(app: &mut App) -> bool {
    let ball = ball(app);
    let entity = app.world().entity(ball);
    let anchored = entity.contains::<Anchored>();
    // The marker and the physics opt-outs always travel together.
    assert_eq!(entity.contains::<RigidBodyDisabled>(), anchored);
    assert_eq!(entity.contains::<ColliderDisabled>(), anchored);
    anchored
}

fn ball_velocity(app: &mut App) -> Vec2 {
    let ball = ball(app);
    app.world().get::<LinearVelocity>(ball).unwrap().0
}

fn set_paddle_vx(app: &mut App, vx: f32) {
    let paddle = paddle(app);
    app.world_mut()
        .entity_mut(paddle)
        .insert(LinearVelocity(Vec2::new(vx, 0.0)));
}

fn assert_resting_on_paddle(app: &mut App) {
    let (ball, paddle) = (ball(app), paddle(app));
    let (b, p) = (translation(app, ball), translation(app, paddle));
    assert_eq!(b.x, p.x);
    assert_eq!(
        b.y,
        p.y + PADDLE_HEIGHT / 2.0 + BALL_ANCHOR_GAP + BALL_SIZE / 2.0
    );
    assert_eq!(ball_velocity(app), Vec2::ZERO);
}

#[test]
fn a_new_run_starts_with_the_ball_anchored_on_the_paddle() {
    let mut app = app();
    app.update();

    assert!(is_anchored(&mut app));
    assert_resting_on_paddle(&mut app);
}

#[test]
fn the_anchored_ball_follows_the_paddle() {
    let mut app = app();
    set_paddle_x(&mut app, -150.0);
    app.update();
    assert_resting_on_paddle(&mut app);
    let ball = ball(&mut app);
    assert_eq!(translation(&app, ball).x, -150.0);

    // A wider paddle (Super-Sizer) keeps the ball centred.
    let paddle = paddle(&mut app);
    app.world_mut().get_mut::<Paddle>(paddle).unwrap().width = PADDLE_WIDTH * 1.5;
    set_paddle_x(&mut app, 90.0);
    app.update();
    assert_resting_on_paddle(&mut app);
}

#[test]
fn space_launches_the_ball_up_and_right_from_a_still_paddle() {
    let mut app = app();
    tap(&mut app, KeyCode::Space);

    assert!(!is_anchored(&mut app));
    let v = ball_velocity(&mut app);
    assert!((v.length() - BALL_SPEED).abs() < 1e-3);
    assert!(v.y > 0.0);
    assert!((v.x - v.y).abs() < 1e-3, "45° to the right, got {v:?}");
}

#[test]
fn left_click_launches_toward_the_way_the_paddle_is_moving() {
    let mut app = app();
    set_paddle_vx(&mut app, -200.0);
    click(&mut app);

    assert!(!is_anchored(&mut app));
    let v = ball_velocity(&mut app);
    assert!(v.y > 0.0);
    assert!((v.x + v.y).abs() < 1e-3, "45° to the left, got {v:?}");
}

#[test]
fn launch_input_does_nothing_while_the_ball_is_in_flight() {
    let mut app = app();
    tap(&mut app, KeyCode::Space);
    let in_flight = Vec2::new(-120.0, 250.0);
    let ball = ball(&mut app);
    app.world_mut().get_mut::<LinearVelocity>(ball).unwrap().0 = in_flight;

    set_paddle_vx(&mut app, 300.0);
    tap(&mut app, KeyCode::Space);
    click(&mut app);

    assert!(!is_anchored(&mut app));
    let v = ball_velocity(&mut app);
    assert!(
        (v.normalize() - in_flight.normalize()).length() < 1e-3,
        "direction unchanged, got {v:?}"
    );
}

#[test]
fn launch_is_ignored_while_paused_and_the_ball_stays_anchored() {
    let mut app = app();
    tap(&mut app, KeyCode::KeyP);
    assert_eq!(play_state(&app), Some(PlayState::Paused));

    // A click that isn't on a pause-menu button does nothing.
    click(&mut app);
    assert_eq!(play_state(&app), Some(PlayState::Paused));
    assert!(is_anchored(&mut app));

    // Space on the pause menu activates the focused Resume button: the
    // game resumes, but that same press doesn't also serve the ball.
    tap(&mut app, KeyCode::Space);
    assert_eq!(play_state(&app), Some(PlayState::Playing));
    assert!(is_anchored(&mut app));
    assert_resting_on_paddle(&mut app);

    // The next press serves.
    tap(&mut app, KeyCode::Space);
    assert!(!is_anchored(&mut app));
}

#[test]
fn losing_a_life_re_anchors_the_ball_on_the_paddle() {
    let mut app = app();
    tap(&mut app, KeyCode::Space);
    set_paddle_x(&mut app, 200.0);
    move_ball_below_screen(&mut app);
    app.update();
    app.update();

    assert_eq!(app.world().resource::<Lives>().0, STARTING_LIVES - 1);
    assert!(is_anchored(&mut app));
    assert_resting_on_paddle(&mut app);
    let ball = ball(&mut app);
    assert_eq!(translation(&app, ball).x, 200.0);

    // Served again from there.
    tap(&mut app, KeyCode::Space);
    assert!(!is_anchored(&mut app));
}

#[test]
fn the_ball_approach_is_recorded_before_each_physics_step() {
    let mut app = app();
    tap(&mut app, KeyCode::Space);
    let ball = ball(&mut app);
    app.world_mut().get_mut::<LinearVelocity>(ball).unwrap().0 = Vec2::new(100.0, -280.0);
    app.update();
    let approach = app.world().get::<BallApproach>(ball).unwrap().0;
    assert!(
        approach.y < 0.0,
        "recorded the downward velocity, got {approach:?}"
    );
}

#[test]
fn the_ball_is_a_round_steel_mesh() {
    let mut app = app();
    let ball = ball(&mut app);
    let entity = app.world().entity(ball);
    assert!(entity.contains::<Mesh2d>());
    assert!(!entity.contains::<Sprite>());
    let material = entity
        .get::<MeshMaterial2d<ColorMaterial>>()
        .unwrap()
        .0
        .clone();
    let color = app
        .world()
        .resource::<Assets<ColorMaterial>>()
        .get(&material)
        .unwrap()
        .color;
    assert_eq!(color, theme::STEEL);
}

#[test]
fn in_mouse_mode_a_paddle_edge_hit_still_spins_the_ball() {
    let mut app = app();
    assert_eq!(
        app.world().resource::<ControlSettings>().paddle,
        PaddleControl::Mouse
    );
    tap(&mut app, KeyCode::Space);
    aim_mouse_at(&mut app, 0.0);
    let (ball, paddle) = (ball(&mut app), paddle(&mut app));
    // Coming down onto the paddle's right edge.
    {
        let paddle_at = translation(&app, paddle);
        let mut transform = app.world_mut().get_mut::<Transform>(ball).unwrap();
        transform.translation.x = paddle_at.x + PADDLE_WIDTH * 0.45;
    }
    app.world_mut().get_mut::<LinearVelocity>(ball).unwrap().0 = Vec2::new(0.0, -BALL_SPEED);
    app.world_mut().trigger(CollisionStart {
        collider1: ball,
        collider2: paddle,
        body1: Some(ball),
        body2: Some(paddle),
    });
    app.update();

    let v = ball_velocity(&mut app);
    assert!(v.y > 0.0, "bounced up, got {v:?}");
    // A centre hit goes straight up; near the edge the ball leaves at a
    // clearly angled side trajectory (over 25° off vertical).
    assert!(
        v.x / v.y > 25f32.to_radians().tan(),
        "near the edge the ball leaves at a side angle, got {v:?}"
    );
}

#[test]
fn the_ball_speed_resource_drives_serve_and_renormalisation() {
    assert_eq!(BallSpeed::default().0, BALL_SPEED);
    let mut app = app();
    app.world_mut().insert_resource(BallSpeed(500.0));
    tap(&mut app, KeyCode::Space);
    let v = ball_velocity(&mut app);
    assert!((v.length() - 500.0).abs() < 1e-3, "served at {v:?}");

    let ball = ball(&mut app);
    app.world_mut().get_mut::<LinearVelocity>(ball).unwrap().0 = Vec2::new(10.0, -900.0);
    app.update();
    let v = ball_velocity(&mut app);
    assert!((v.length() - 500.0).abs() < 1e-3, "renormalised to {v:?}");
}

#[test]
fn the_speed_factor_ramps_per_round_and_caps() {
    for (round, factor) in [(1, 1.8), (2, 1.9), (5, 2.2), (8, 2.5), (20, 2.5)] {
        assert!(
            (speed_factor(round) - factor).abs() < 1e-5,
            "round {round}: {}",
            speed_factor(round)
        );
    }
    assert_eq!(
        speed_factor(1),
        BALL_SPEED_SCALE,
        "round 1 is today's speed"
    );
    assert!((BallSpeed::from_factor(speed_factor(1)).0 - BALL_SPEED).abs() < 1e-3);
    assert!((BallSpeed::from_factor(speed_factor(8)).0 - 750.0).abs() < 1e-3);
}

#[test]
fn a_levels_speed_factor_overrides_the_ramp() {
    let ramped = crate::levels::parse_level("grid:\nC").unwrap();
    let fixed = crate::levels::parse_level("speed_factor: 2.0\ngrid:\nC").unwrap();
    assert!((BallSpeed::for_level(&ramped, 2).0 - 570.0).abs() < 1e-3);
    assert!((BallSpeed::for_level(&ramped, 1).0 - 540.0).abs() < 1e-3);
    for round in [1, 2, 9] {
        assert!((BallSpeed::for_level(&fixed, round).0 - 600.0).abs() < 1e-3);
    }
}
