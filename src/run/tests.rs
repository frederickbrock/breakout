use super::*;
use crate::bricks::grid::{Brick, BRICK_COLS, BRICK_ROWS};
use crate::game_state::PlayState;
use crate::paddle::Paddle;
use crate::test_support::*;
use crate::world::{PLAYFIELD_WIDTH, WORLD_HEIGHT, WORLD_WIDTH};

fn text<M: Component>(app: &mut App) -> String {
    app.world_mut()
        .query_filtered::<&TextSpan, With<M>>()
        .single(app.world())
        .map(|t| t.0.clone())
        .unwrap_or_default()
}

#[test]
fn leaving_the_menu_for_a_run_starts_playing() {
    let mut app = app();
    app.update();

    assert_eq!(app_state(&app), AppState::InGame);
    assert_eq!(play_state(&app), Some(PlayState::Playing));
    assert!(!physics_paused(&app));
    assert_eq!(count::<With<Ball>>(&mut app), 1);
    assert_eq!(count::<With<Paddle>>(&mut app), 1);
    assert_eq!(count::<With<Brick>>(&mut app), BRICK_ROWS * BRICK_COLS);
    assert_eq!(text::<LivesText>(&mut app), "3");
    assert_eq!(text::<ScoreText>(&mut app), "0");
}

#[test]
fn p_and_esc_toggle_pause_and_the_physics_clock() {
    let mut app = app();

    tap(&mut app, KeyCode::KeyP);
    assert_eq!(play_state(&app), Some(PlayState::Paused));
    assert!(physics_paused(&app));

    tap(&mut app, KeyCode::Escape);
    assert_eq!(play_state(&app), Some(PlayState::Playing));
    assert!(!physics_paused(&app));

    tap(&mut app, KeyCode::Escape);
    assert_eq!(play_state(&app), Some(PlayState::Paused));
    tap(&mut app, KeyCode::KeyP);
    assert_eq!(play_state(&app), Some(PlayState::Playing));
}

#[test]
fn losing_the_last_life_ends_the_run_and_r_starts_a_fresh_one() {
    let mut app = app();
    tap(&mut app, KeyCode::Space);
    app.world_mut().resource_mut::<Score>().0 = 120;
    app.world_mut().resource_mut::<Lives>().0 = 1;
    move_ball_below_screen(&mut app);
    app.update();
    app.update();

    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(play_state(&app), None);
    assert_eq!(
        app.world().get_resource::<GameOutcome>(),
        Some(&GameOutcome::Lost)
    );
    assert!(physics_paused(&app));
    assert_eq!(count::<With<Ball>>(&mut app), 0);
    assert_eq!(count::<With<Brick>>(&mut app), 0);
    assert_eq!(count::<With<LivesText>>(&mut app), 0);

    // P/Esc do nothing outside a run.
    tap(&mut app, KeyCode::KeyP);
    assert_eq!(app_state(&app), AppState::GameOver);

    tap(&mut app, KeyCode::KeyR);
    assert_eq!(app_state(&app), AppState::InGame);
    assert_eq!(play_state(&app), Some(PlayState::Playing));
    assert!(!physics_paused(&app));
    assert_eq!(app.world().resource::<Score>().0, 0);
    assert_eq!(app.world().resource::<Lives>().0, STARTING_LIVES);
    assert_eq!(count::<With<Ball>>(&mut app), 1);
    assert_eq!(count::<With<Brick>>(&mut app), BRICK_ROWS * BRICK_COLS);
    assert_eq!(text::<LivesText>(&mut app), "3");
}

#[test]
fn losing_a_life_that_is_not_the_last_keeps_playing() {
    let mut app = app();
    tap(&mut app, KeyCode::Space);
    move_ball_below_screen(&mut app);
    app.update();
    app.update();

    assert_eq!(app_state(&app), AppState::InGame);
    assert_eq!(app.world().resource::<Lives>().0, STARTING_LIVES - 1);
    assert_eq!(text::<LivesText>(&mut app), "2");
}

#[test]
fn breaking_the_last_brick_wins() {
    let mut app = app();
    tap(&mut app, KeyCode::Space);
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

    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(
        app.world().get_resource::<GameOutcome>(),
        Some(&GameOutcome::Won)
    );
    assert!(physics_paused(&app));

    tap(&mut app, KeyCode::KeyR);
    assert_eq!(app_state(&app), AppState::InGame);
    assert_eq!(count::<With<Brick>>(&mut app), BRICK_ROWS * BRICK_COLS);
}

#[test]
fn the_hud_shows_uppercase_labels_and_ink_values() {
    let mut app = app();
    let world = app.world_mut();
    let mut labels: Vec<(String, Color)> = world
        .query::<(&Text2d, &TextColor)>()
        .iter(world)
        .map(|(t, c)| (t.0.clone(), c.0))
        .collect();
    labels.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        labels,
        [
            ("LIVES\n".to_string(), theme::LABEL),
            ("SCORE\n".to_string(), theme::LABEL)
        ]
    );
    let values: Vec<Color> = world
        .query_filtered::<&TextColor, With<TextSpan>>()
        .iter(world)
        .map(|c| c.0)
        .collect();
    assert_eq!(values, [theme::INK, theme::INK]);
}

#[test]
fn the_hud_sits_in_the_left_side_panel() {
    let mut app = app();
    let world = app.world_mut();
    let mut blocks: Vec<(f32, f32, Anchor, f32)> = world
        .query_filtered::<(&Transform, &TextFont, &Anchor), With<Text2d>>()
        .iter(world)
        .map(|(t, f, a)| {
            let FontSize::Px(size) = f.font_size else {
                panic!("HUD font size in px");
            };
            (t.translation.x, t.translation.y, *a, size)
        })
        .collect();
    assert_eq!(blocks.len(), 2);
    // Widest line is 6 glyphs ("SCORE", a value); monospace ~0.6 em each.
    let widest = 6.0 * 0.6 * HUD_FONT_SIZE;
    for &(x, _, anchor, size) in &blocks {
        assert_eq!(anchor, Anchor::TOP_LEFT);
        assert_eq!(size, HUD_FONT_SIZE);
        assert!(x >= -WORLD_WIDTH / 2.0);
        assert!(x + widest <= -PLAYFIELD_WIDTH / 2.0);
    }
    // Each two-line block (~1.2 em line height) ends before the next.
    blocks.sort_by(|a, b| b.1.total_cmp(&a.1));
    assert!(blocks[0].1 <= WORLD_HEIGHT / 2.0);
    assert!(blocks[0].1 - blocks[1].1 >= 2.0 * 1.2 * HUD_FONT_SIZE);
}
