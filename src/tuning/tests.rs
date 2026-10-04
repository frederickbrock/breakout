use super::*;
use crate::test_support::launch_with;
use std::time::{Duration, Instant};

#[test]
fn the_shipped_tuning_file_is_the_compiled_defaults() {
    let text = include_str!("../../assets/game.tuning.ron");
    assert_eq!(parse_tuning(text), Ok(Tuning::default()));
}

#[test]
fn the_defaults_are_todays_values() {
    let t = Tuning::default();
    assert_eq!(
        t.ball.speed_factor * t.ball.speed_per_factor,
        crate::ball::BALL_SPEED
    );
    assert_eq!(t.ball.lives, 3);
    assert_eq!(t.bricks.hits.tungsten, 3);
    assert_eq!(t.bricks.fill_weights.ceramic, 90);
    assert_eq!(t.bricks.reactor_bricks, 6);
    assert_eq!(t.powerups.super_sizer.duration, 7.0);
}

#[test]
fn a_partial_file_keeps_the_defaults_for_what_it_leaves_out() {
    let tuning = parse_tuning("(ball: (lives: 5), bricks: (hits: (titanium: 4)))").unwrap();
    let mut expected = Tuning::default();
    expected.ball.lives = 5;
    expected.bricks.hits.titanium = 4;
    assert_eq!(tuning, expected);
    assert_eq!(parse_tuning("()").unwrap(), Tuning::default());
}

#[test]
fn a_broken_file_is_an_error() {
    assert!(parse_tuning("(ball: (lives: \"many\"))").is_err());
    assert!(parse_tuning("(ball: ").is_err());
}

/// A launched app with the tuning plugin, so it really loads `path` from
/// `assets/` through the asset server.
fn app_loading(path: &'static str) -> App {
    let mut app = launch_with(|app| {
        app.add_plugins(TuningPlugin);
    });
    let handle = app.world().resource::<AssetServer>().load(path);
    app.world_mut().resource_mut::<TuningHandle>().0 = handle;
    app
}

/// Updates until the tuning file has loaded or failed (asset IO runs on
/// other threads), then once more so `sync_tuning` sees the result.
fn wait_for_tuning(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let handle = app.world().resource::<TuningHandle>().0.clone();
        let state = app.world().resource::<AssetServer>().load_state(&handle);
        if state.is_loaded() || state.is_failed() {
            break;
        }
        assert!(Instant::now() < deadline, "the tuning file never settled");
        std::thread::sleep(Duration::from_millis(5));
    }
    app.update();
}

#[test]
fn the_shipped_file_loads_into_the_resource() {
    let mut app = app_loading(TUNING_PATH);
    // Start from something else, to see the load land.
    app.world_mut().resource_mut::<Tuning>().ball.lives = 99;
    wait_for_tuning(&mut app);
    assert_eq!(*app.world().resource::<Tuning>(), Tuning::default());
}

#[test]
fn a_missing_file_keeps_the_values_and_the_game_runs() {
    let mut app = app_loading("missing.tuning.ron");
    wait_for_tuning(&mut app);
    let handle = app.world().resource::<TuningHandle>().0.clone();
    assert!(app
        .world()
        .resource::<AssetServer>()
        .load_state(&handle)
        .is_failed());
    assert_eq!(*app.world().resource::<Tuning>(), Tuning::default());
    app.update();
}

#[test]
fn editing_the_loaded_file_updates_the_resource() {
    let mut app = app_loading(TUNING_PATH);
    wait_for_tuning(&mut app);
    // What a hot reload does: the asset changes, Bevy sends `Modified`.
    let handle = app.world().resource::<TuningHandle>().0.clone();
    app.world_mut()
        .resource_mut::<Assets<TuningAsset>>()
        .get_mut(&handle)
        .unwrap()
        .0
        .paddle
        .width = 240.0;
    // Bevy sends the event at the end of this update; the sync reads it next.
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Tuning>().paddle.width, 240.0);
}

// ---- consumers (sim-dj6.2): ball, paddle and lives read Tuning ----

/// A fresh run of `levels` (or the fallback board) started with `tuning`.
fn run_with(tuning: Tuning, levels: Option<Vec<crate::levels::LevelDef>>) -> App {
    let mut app = launch_with(move |app| {
        app.insert_resource(tuning);
        if let Some(levels) = levels {
            app.insert_resource(crate::levels::CampaignLevels(levels));
        }
    });
    app.world_mut()
        .resource_mut::<NextState<crate::game_state::AppState>>()
        .set(crate::game_state::AppState::InGame);
    app.update();
    app
}

fn tuning_mut(app: &mut App) -> Mut<'_, Tuning> {
    app.world_mut().resource_mut::<Tuning>()
}

#[test]
fn the_paddle_force_follows_tuning_on_the_next_frame() {
    use avian2d::prelude::ConstantForce;
    let mut app = run_with(Tuning::default(), None);
    let paddle = crate::test_support::paddle(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ArrowLeft);
    app.update();
    let force = |app: &App| app.world().get::<ConstantForce>(paddle).unwrap().0.x;
    assert_eq!(force(&app), -Tuning::default().paddle.force);
    tuning_mut(&mut app).paddle.force = 1234.0;
    app.update();
    assert_eq!(force(&app), -1234.0);
}

#[test]
fn paddle_mass_and_damping_are_reapplied_when_tuning_changes() {
    use avian2d::prelude::{LinearDamping, Mass};
    let mut tuning = Tuning::default();
    tuning.paddle.mass = 7.0;
    let mut app = run_with(tuning, None);
    let paddle = crate::test_support::paddle(&mut app);
    assert_eq!(
        app.world().get::<Mass>(paddle).unwrap().0,
        7.0,
        "spawned tuned"
    );
    {
        let mut t = tuning_mut(&mut app);
        t.paddle.mass = 9.0;
        t.paddle.linear_damping = 1.5;
    }
    app.update();
    assert_eq!(app.world().get::<Mass>(paddle).unwrap().0, 9.0);
    assert_eq!(app.world().get::<LinearDamping>(paddle).unwrap().0, 1.5);
}

#[test]
fn the_paddle_width_follows_tuning_with_and_without_super_sizer() {
    use crate::paddle::Paddle;
    use crate::powerups::super_sizer::WIDTH_MULTIPLIER;
    use crate::powerups::{PowerUpCollected, PowerUpKind};
    use avian2d::prelude::Collider;
    let mut app = run_with(Tuning::default(), None);
    let paddle = crate::test_support::paddle(&mut app);
    let width = |app: &App| app.world().get::<Paddle>(paddle).unwrap().width;
    let collider_half_x = |app: &App| {
        app.world()
            .get::<Collider>(paddle)
            .unwrap()
            .shape()
            .as_cuboid()
            .unwrap()
            .half_extents
            .x
    };
    tuning_mut(&mut app).paddle.width = 240.0;
    app.update();
    assert_eq!(width(&app), 240.0);
    assert_eq!(collider_half_x(&app), 120.0);

    app.world_mut().trigger(PowerUpCollected {
        kind: PowerUpKind::SuperSizer,
    });
    app.update();
    assert_eq!(width(&app), 240.0 * WIDTH_MULTIPLIER);
    assert_eq!(collider_half_x(&app), 120.0 * WIDTH_MULTIPLIER);
}

#[test]
fn the_tuned_start_factor_sets_the_speed_unless_the_level_sets_one() {
    use crate::ball::BallSpeed;
    let mut tuning = Tuning::default();
    tuning.ball.speed_factor = 2.0;
    let level = crate::levels::parse_level("grid:\nC").unwrap();
    let app = run_with(tuning.clone(), Some(vec![level]));
    assert_eq!(app.world().resource::<BallSpeed>().0, 600.0);

    let fixed = crate::levels::parse_level("speed_factor: 1.0\ngrid:\nC").unwrap();
    let app = run_with(tuning, Some(vec![fixed]));
    assert_eq!(
        app.world().resource::<BallSpeed>().0,
        300.0,
        "the level wins"
    );
}

#[test]
fn tuned_lives_start_the_run_and_show_in_the_hud() {
    use crate::run::{Lives, LivesText};
    let mut tuning = Tuning::default();
    tuning.ball.lives = 5;
    let mut app = run_with(tuning, None);
    app.update();
    assert_eq!(app.world().resource::<Lives>().0, 5);
    let shown: Vec<String> = app
        .world_mut()
        .query_filtered::<&TextSpan, With<LivesText>>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .collect();
    assert_eq!(shown, ["5"]);
}
