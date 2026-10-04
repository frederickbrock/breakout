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
