use super::super::test_helpers::*;
use crate::ball::Ball;
use crate::bricks::grid::{Brick, BRICK_COLS, BRICK_ROWS};
use crate::collision::BallCollisionSignals;
use crate::game_state::{AppState, PlayState};
use crate::run::{Lives, Score, STARTING_LIVES};
use crate::test_support::*;
use crate::world::PLAYFIELD_HEIGHT;
use bevy::prelude::*;

fn lose(app: &mut App, score: i32) {
    // Serve first: an anchored ball rides the paddle and can't fall.
    tap(app, KeyCode::Space);
    app.world_mut().resource_mut::<Score>().0 = score;
    app.world_mut().resource_mut::<Lives>().0 = 1;
    let world = app.world_mut();
    let mut ball = world
        .query_filtered::<&mut Transform, With<Ball>>()
        .single_mut(world)
        .expect("a run has exactly one ball");
    ball.translation.y = -PLAYFIELD_HEIGHT;
    app.update();
    app.update();
    assert_eq!(app_state(app), AppState::GameOver);
}

fn win(app: &mut App, score: i32) {
    // Serve first: `ball_movement` only checks for a win with a ball in flight.
    tap(app, KeyCode::Space);
    app.world_mut().resource_mut::<Score>().0 = score;
    let bricks: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<Brick>>()
        .iter(app.world())
        .collect();
    // Clear every brick and report the last one broken, as the collision
    // observer would have.
    for brick in &bricks {
        app.world_mut().despawn(*brick);
    }
    app.world_mut()
        .resource_mut::<BallCollisionSignals>()
        .broke_brick = true;
    app.update();
    app.update();
    assert_eq!(app_state(app), AppState::GameOver);
}

fn assert_fresh_run(app: &mut App) {
    assert_eq!(app_state(app), AppState::InGame);
    assert_eq!(play_state(app), Some(PlayState::Playing));
    assert_eq!(app.world().resource::<Score>().0, 0);
    assert_eq!(app.world().resource::<Lives>().0, STARTING_LIVES);
    assert_eq!(count::<With<Brick>>(app), BRICK_ROWS * BRICK_COLS);
    assert!(button_labels(app).is_empty());
}

#[test]
fn losing_shows_game_over_with_the_final_score() {
    let mut app = app();
    lose(&mut app, 120);

    let shown = texts(&mut app);
    assert!(shown.contains(&"GAME OVER".to_string()));
    assert!(shown.contains(&"Score: 120".to_string()));
    assert!(!shown.contains(&"YOU WIN!".to_string()));
    assert_eq!(button_labels(&mut app), ["Play again", "Main menu"]);
    assert_eq!(focused_label(&mut app), "Play again");
}

#[test]
fn winning_shows_you_win_with_the_final_score() {
    let mut app = app();
    win(&mut app, 750);

    let shown = texts(&mut app);
    assert!(shown.contains(&"YOU WIN!".to_string()));
    assert!(shown.contains(&"Score: 750".to_string()));
    assert!(!shown.contains(&"GAME OVER".to_string()));
}

#[test]
fn play_again_by_click_enter_or_r_starts_a_fresh_run() {
    let mut app = app();
    lose(&mut app, 10);
    press(&mut app, "Play again");
    assert_fresh_run(&mut app);

    lose(&mut app, 10);
    tap(&mut app, KeyCode::Enter);
    assert_fresh_run(&mut app);

    lose(&mut app, 10);
    tap(&mut app, KeyCode::KeyR);
    assert_fresh_run(&mut app);
}

#[test]
fn main_menu_returns_to_the_title_screen() {
    let mut app = app();
    win(&mut app, 10);
    tap(&mut app, KeyCode::ArrowDown);
    tap(&mut app, KeyCode::Enter);

    assert_eq!(app_state(&app), AppState::MainMenu);
    assert!(texts(&mut app).contains(&"BREAKOUT".to_string()));
    assert_eq!(button_labels(&mut app), ["Start", "Settings", "Quit"]);
}
