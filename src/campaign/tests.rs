use super::*;
use crate::ball::{Anchored, Ball};
use crate::bricks::grid::Brick;
use crate::bricks::BrickCell;
use crate::levels::parse_level;
use crate::menu::test_helpers::{press, texts};
use crate::paddle::{Paddle, PADDLE_WIDTH};
use crate::powerups::capsules::Capsule;
use crate::powerups::{test_spawn_power_up, PowerUp, PowerUpBrick, PowerUpCollected, PowerUpKind};
use crate::run::{Lives, Score, SectorText, STARTING_LIVES};
use crate::test_support::*;
use avian2d::prelude::LinearVelocity;

fn level(text: &str) -> LevelDef {
    match parse_level(text) {
        Ok(def) => def,
        Err(e) => panic!("{e}"),
    }
}

/// Two one-brick levels with distinct layouts and speeds: a brick at (0,0)
/// at 300, then one at (0,1) at 600.
fn two_levels() -> Vec<LevelDef> {
    vec![
        level("name: First\nspeed_factor: 1.0\ngrid:\nC"),
        level("name: Second\nspeed_factor: 2.0\ngrid:\n.C"),
    ]
}

/// Serves if needed, breaks every brick, and lets the clear take effect.
fn clear(app: &mut App) {
    if count::<(With<Ball>, With<Anchored>)>(app) > 0 {
        tap(app, KeyCode::Space);
    }
    while let Some(&brick) = bricks(app).first() {
        hit(app, brick);
    }
    app.update();
    app.update();
}

/// Updates until play resumes; returns how many updates that took.
fn wait_out_card(app: &mut App) -> usize {
    for n in 1..=40 {
        app.update();
        if play_state(app) == Some(PlayState::Playing) {
            return n;
        }
    }
    panic!("the sector card never ended");
}

fn sector(app: &mut App) -> String {
    let world = app.world_mut();
    world
        .query_filtered::<&TextSpan, With<SectorText>>()
        .single(world)
        .expect("one sector span")
        .0
        .clone()
}

fn current(app: &App) -> usize {
    app.world().resource::<CurrentLevel>().0
}

fn cells(app: &mut App) -> Vec<BrickCell> {
    app.world_mut()
        .query_filtered::<&BrickCell, With<Brick>>()
        .iter(app.world())
        .copied()
        .collect()
}

fn elapsed(app: &App) -> f32 {
    app.world()
        .resource::<LevelTransition>()
        .timer
        .elapsed_secs()
}

/// The shipped campaign's levels, read from `assets/levels/` in manifest order.
fn shipped_campaign() -> Vec<LevelDef> {
    crate::levels::parse_campaign(include_str!("../../assets/levels/campaign.txt"))
        .levels
        .iter()
        .map(|file| {
            let path = format!("assets/levels/{file}");
            level(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")))
        })
        .collect()
}

/// Like [`clear`], but every hit comes from above, so shield glass breaks
/// instead of deflecting (the random boards include shields).
fn clear_from_above(app: &mut App) {
    if count::<(With<Ball>, With<Anchored>)>(app) > 0 {
        tap(app, KeyCode::Space);
    }
    while let Some(&brick) = bricks(app).first() {
        hit_moving(app, brick, Vec2::new(0.0, -450.0));
    }
    app.update();
    app.update();
}

#[test]
fn the_shipped_campaign_plays_every_planet_in_order_and_wins() {
    let levels = shipped_campaign();
    assert_eq!(levels.len(), 25);
    let mut app = app_with_campaign(levels.clone());
    for (next, def) in levels.iter().enumerate().skip(1) {
        let card = format!("SECTOR {:02} // {}", next + 1, def.name);
        clear_from_above(&mut app);
        assert_eq!(play_state(&app), Some(PlayState::LevelClear), "{card}");
        assert!(texts(&mut app).contains(&card), "{card}");
        wait_out_card(&mut app);
        assert_eq!(current(&app), next);
        assert!(count::<With<Brick>>(&mut app) > 0, "{card}: board spawned");
    }
    clear_from_above(&mut app);
    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(
        app.world().get_resource::<GameOutcome>(),
        Some(&GameOutcome::Won)
    );
}

#[test]
fn clearing_a_level_shows_the_card_then_starts_the_next_and_the_last_wins() {
    let mut app = app_with_campaign(two_levels());
    assert_eq!(current(&app), 0);
    assert_eq!(sector(&mut app), "01");

    clear(&mut app);
    assert_eq!(play_state(&app), Some(PlayState::LevelClear));
    assert!(physics_paused(&app));
    assert!(texts(&mut app).contains(&"SECTOR 02 // Second".to_string()));
    assert_eq!(count::<With<SectorCard>>(&mut app), 1);
    assert_eq!(count::<With<Brick>>(&mut app), 0);

    let updates = wait_out_card(&mut app);
    assert!((18..=22).contains(&updates), "{updates} updates");
    assert_eq!(current(&app), 1);
    assert_eq!(count::<With<SectorCard>>(&mut app), 0);
    assert_eq!(cells(&mut app), [BrickCell { row: 0, col: 1 }]);
    assert_eq!(count::<(With<Ball>, With<Anchored>)>(&mut app), 1);
    assert!(!physics_paused(&app));
    app.update();
    assert_eq!(sector(&mut app), "02");

    clear(&mut app);
    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(
        app.world().get_resource::<GameOutcome>(),
        Some(&GameOutcome::Won)
    );
    let texts = texts(&mut app);
    assert!(texts.contains(&"YOU WIN!".to_string()), "{texts:?}");
    assert!(texts.contains(&"Score: 20".to_string()), "{texts:?}");
}

#[test]
fn score_and_lives_carry_over_and_power_ups_and_speed_reset_per_level() {
    let mut app = app_with_campaign(two_levels());
    app.world_mut().resource_mut::<Lives>().0 = 2;
    app.world_mut().trigger(PowerUpCollected {
        kind: PowerUpKind::SuperSizer,
    });
    app.update();
    let paddle = paddle(&mut app);
    assert!(app.world().get::<Paddle>(paddle).unwrap().width > PADDLE_WIDTH);
    test_spawn_power_up(&mut app);
    assert_eq!(count::<With<PowerUp>>(&mut app), 1);

    clear(&mut app);
    assert_eq!(
        count::<With<PowerUp>>(&mut app),
        1,
        "frozen during the card"
    );
    wait_out_card(&mut app);
    app.update();
    assert_eq!(app.world().resource::<Score>().0, 10);
    assert_eq!(app.world().resource::<Lives>().0, 2);
    assert_eq!(count::<With<PowerUp>>(&mut app), 0);
    assert_eq!(
        app.world().get::<Paddle>(paddle).unwrap().width,
        PADDLE_WIDTH
    );
    assert_eq!(count::<With<Capsule>>(&mut app), 0);
    assert_eq!(app.world().resource::<BallSpeed>().0, 600.0);
    tap(&mut app, KeyCode::Space);
    let ball = ball(&mut app);
    let speed = app.world().get::<LinearVelocity>(ball).unwrap().0.length();
    assert!((speed - 600.0).abs() < 1e-2, "{speed}");
}

/// The served ball's measured speed.
fn served_speed(app: &mut App) -> f32 {
    tap(app, KeyCode::Space);
    let ball = ball(app);
    app.world().get::<LinearVelocity>(ball).unwrap().0.length()
}

#[test]
fn without_a_speed_factor_each_round_serves_faster() {
    let ramped = || vec![level("name: One\ngrid:\nC"), level("name: Two\ngrid:\n.C")];
    let mut app = app_with_campaign(ramped());
    // Round 1: today's 540.
    let speed = served_speed(&mut app);
    assert!((speed - 540.0).abs() < 5.4, "{speed}");

    clear(&mut app);
    wait_out_card(&mut app);
    // Round 2: 570, sizes untouched.
    let speed = served_speed(&mut app);
    assert!((speed - 570.0).abs() < 1e-2, "{speed}");
    let paddle = paddle(&mut app);
    assert_eq!(
        app.world().get::<Paddle>(paddle).unwrap().width,
        PADDLE_WIDTH
    );

    // Play again after a loss: back to round 1's speed.
    app.world_mut().resource_mut::<Lives>().0 = 1;
    move_ball_below_screen(&mut app);
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::GameOver);
    press(&mut app, "Play again");
    app.update();
    assert_eq!(current(&app), 0);
    let speed = served_speed(&mut app);
    assert!((speed - 540.0).abs() < 1e-2, "{speed}");
}

#[test]
fn the_next_levels_reactors_are_equipped() {
    let mut app = app_with_campaign(vec![level("grid:\nC"), level("name: Reactor\ngrid:\nP")]);
    clear(&mut app);
    wait_out_card(&mut app);
    assert_eq!(count::<With<PowerUpBrick>>(&mut app), 1);
}

/// A fresh run of [`two_levels`]: level 1's board, counters reset, no card.
fn assert_fresh_first_level(app: &mut App) {
    assert_eq!(app_state(app), AppState::InGame);
    assert_eq!(current(app), 0);
    assert_eq!(cells(app), [BrickCell { row: 0, col: 0 }]);
    assert_eq!(app.world().resource::<Score>().0, 0);
    assert_eq!(app.world().resource::<Lives>().0, STARTING_LIVES);
    assert_eq!(app.world().resource::<BallSpeed>().0, 300.0);
    assert_eq!(count::<With<SectorCard>>(app), 0);
    app.update();
    assert_eq!(sector(app), "01");
}

#[test]
fn game_over_on_level_two_and_play_again_restarts_at_level_one() {
    let mut app = app_with_campaign(two_levels());
    clear(&mut app);
    wait_out_card(&mut app);
    assert_eq!(current(&app), 1);
    app.world_mut().resource_mut::<Lives>().0 = 1;
    tap(&mut app, KeyCode::Space);
    move_ball_below_screen(&mut app);
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(
        app.world().get_resource::<GameOutcome>(),
        Some(&GameOutcome::Lost)
    );
    assert!(texts(&mut app).contains(&"GAME OVER".to_string()));

    press(&mut app, "Play again");
    app.update();
    assert_fresh_first_level(&mut app);
}

#[test]
fn r_after_winning_restarts_at_level_one() {
    let mut app = app_with_campaign(two_levels());
    clear(&mut app);
    wait_out_card(&mut app);
    clear(&mut app);
    assert_eq!(
        app.world().get_resource::<GameOutcome>(),
        Some(&GameOutcome::Won)
    );
    tap(&mut app, KeyCode::KeyR);
    assert_fresh_first_level(&mut app);
}

#[test]
fn abandoning_a_run_mid_card_leaves_nothing_pending() {
    let mut app = app_with_campaign(two_levels());
    clear(&mut app);
    tap(&mut app, KeyCode::Escape);
    press(&mut app, "Main menu");
    assert_eq!(app_state(&app), AppState::MainMenu);
    press(&mut app, "Start");
    app.update();
    assert_eq!(play_state(&app), Some(PlayState::Playing));
    assert!(app.world().get_resource::<LevelTransition>().is_none());
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(current(&app), 0);
    assert_eq!(cells(&mut app), [BrickCell { row: 0, col: 0 }]);
    assert_eq!(count::<With<SectorCard>>(&mut app), 0);
}

/// Clears level 1, pauses 5 updates into the card with `pause`, checks the
/// card's timer stays frozen, resumes with `resume`, and waits it out.
fn pause_during_card(pause: impl Fn(&mut App), resume: impl Fn(&mut App)) {
    let mut app = app_with_campaign(two_levels());
    clear(&mut app);
    for _ in 0..5 {
        app.update();
    }
    pause(&mut app);
    assert_eq!(play_state(&app), Some(PlayState::Paused));
    assert_eq!(count::<With<SectorCard>>(&mut app), 1);
    let frozen = elapsed(&app);
    assert!(frozen > 0.0 && frozen < SECTOR_CARD_SECS);
    for _ in 0..40 {
        app.update();
    }
    assert_eq!(play_state(&app), Some(PlayState::Paused));
    assert_eq!(elapsed(&app), frozen);
    assert_eq!(current(&app), 0);

    resume(&mut app);
    assert_eq!(play_state(&app), Some(PlayState::LevelClear));
    assert!(physics_paused(&app));
    let left = ((SECTOR_CARD_SECS - elapsed(&app)) / 0.1).round() as usize;
    let updates = wait_out_card(&mut app);
    assert!(updates < 20, "{updates} updates");
    assert!(
        updates.abs_diff(left) <= 2,
        "{updates} updates, {left} left"
    );
    assert_eq!(current(&app), 1);
}

#[test]
fn pausing_during_the_card_freezes_its_timer() {
    pause_during_card(|app| tap(app, KeyCode::KeyP), |app| tap(app, KeyCode::KeyP));
}

#[test]
fn the_resume_button_returns_to_the_card() {
    pause_during_card(|app| tap(app, KeyCode::Escape), |app| press(app, "Resume"));
}
