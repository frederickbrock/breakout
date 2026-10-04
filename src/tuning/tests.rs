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

// ---- consumers (sim-dj6.3): bricks and power-ups read Tuning ----

/// A fresh run of `levels` (or the fallback board) started with `tuning`.
fn run_tuned(tuning: Tuning, levels: Option<Vec<crate::levels::LevelDef>>) -> App {
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

fn max_hits_of(app: &mut App, class: BrickClass) -> Vec<u8> {
    use crate::bricks::grid::BrickMaxHits;
    app.world_mut()
        .query::<(&BrickClass, &BrickMaxHits)>()
        .iter(app.world())
        .filter(|(c, _)| **c == class)
        .map(|(_, h)| h.0)
        .collect()
}

#[test]
fn tuned_hits_apply_to_a_new_board_and_a_level_override_still_wins() {
    let mut tuning = Tuning::default();
    tuning.bricks.hits.titanium = 4;
    let level = crate::levels::parse_level("grid:\nTT").unwrap();
    let mut app = run_tuned(tuning.clone(), Some(vec![level]));
    assert_eq!(max_hits_of(&mut app, BrickClass::Titanium), [4, 4]);

    let level = crate::levels::parse_level("legend:\nk = titanium hits=2\ngrid:\nk").unwrap();
    let mut app = run_tuned(tuning, Some(vec![level]));
    assert_eq!(
        max_hits_of(&mut app, BrickClass::Titanium),
        [2],
        "hits= wins"
    );
}

#[test]
fn fill_weights_change_the_mix_and_the_defaults_are_todays_table() {
    use crate::levels::{build_board, LevelDef};
    use rand::{rngs::StdRng, SeedableRng};
    assert_eq!(
        FillWeights::default().table(),
        crate::bricks::FILL_WEIGHTS,
        "default weights walk the rng exactly as before"
    );
    let def = LevelDef::fallback();
    let ceramics = |tuning: &BrickTuning| {
        build_board(&def, &mut StdRng::seed_from_u64(7), tuning)
            .iter()
            .filter(|b| b.class == BrickClass::Ceramic)
            .count()
    };
    let default = ceramics(&BrickTuning::default());
    let heavy = BrickTuning {
        fill_weights: FillWeights {
            ceramic: 1000,
            titanium: 1,
            tungsten: 1,
            explosive_charge: 1,
            explosive_breach: 1,
            explosive_demolition: 1,
            regen: 1,
            shield: 1,
        },
        ..BrickTuning::default()
    };
    let tuned = ceramics(&heavy);
    assert!(tuned > default + 20, "{tuned} vs {default}");
}

#[test]
fn the_fallback_boards_power_up_count_is_tunable() {
    use crate::powerups::PowerUpBrick;
    let mut tuning = Tuning::default();
    tuning.bricks.reactor_bricks = 2;
    let mut app = run_tuned(tuning, None);
    assert_eq!(
        crate::test_support::count::<With<PowerUpBrick>>(&mut app),
        2
    );
}

#[test]
fn a_tuned_heal_time_heals_a_regen_brick_after_that_long() {
    use crate::bricks::grid::BrickHealth;
    let mut tuning = Tuning::default();
    tuning.bricks.regen_heal_secs = 2.0;
    let level = crate::levels::parse_level("grid:\nRC").unwrap();
    let mut app = run_tuned(tuning, Some(vec![level]));
    let regen = crate::test_support::brick_of(&mut app, BrickClass::Regen);
    crate::test_support::hit(&mut app, regen);
    let health = |app: &App| app.world().get::<BrickHealth>(regen).unwrap().0;
    assert_eq!(health(&app), 1);
    for _ in 0..15 {
        app.update(); // 1.5 s
    }
    assert_eq!(health(&app), 1, "not yet");
    for _ in 0..6 {
        app.update(); // past 2 s
    }
    assert_eq!(health(&app), 2, "healed");
}

#[test]
fn super_sizer_duration_and_multiplier_apply_on_the_next_pickup() {
    use crate::paddle::{Paddle, PADDLE_WIDTH};
    use crate::powerups::{PowerUpCollected, PowerUpKind};
    let mut app = run_tuned(Tuning::default(), None);
    {
        let mut tuning = app.world_mut().resource_mut::<Tuning>();
        tuning.powerups.super_sizer.duration = 1.0;
        tuning.powerups.super_sizer.width_multiplier = 2.0;
    }
    app.world_mut().trigger(PowerUpCollected {
        kind: PowerUpKind::SuperSizer,
    });
    app.update();
    let paddle = crate::test_support::paddle(&mut app);
    let width = |app: &App| app.world().get::<Paddle>(paddle).unwrap().width;
    assert_eq!(width(&app), PADDLE_WIDTH * 2.0);
    for _ in 0..11 {
        app.update(); // past the tuned 1 s
    }
    assert_eq!(
        width(&app),
        PADDLE_WIDTH,
        "expired after the tuned duration"
    );
}

#[test]
fn the_super_sizer_weight_follows_tuning() {
    use crate::powerups::PowerUpSpawner;
    let mut app = run_tuned(Tuning::default(), None);
    assert!(app.world().resource::<PowerUpSpawner>().pick().is_some());
    app.world_mut()
        .resource_mut::<Tuning>()
        .powerups
        .super_sizer
        .weight = 0.0;
    app.update();
    assert!(
        app.world().resource::<PowerUpSpawner>().pick().is_none(),
        "Super-Sizer is the only kind, now at weight 0"
    );
}
