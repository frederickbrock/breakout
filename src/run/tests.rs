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
            ("SCORE\n".to_string(), theme::LABEL),
            ("SECTOR\n".to_string(), theme::LABEL)
        ]
    );
    let values: Vec<Color> = world
        .query_filtered::<&TextColor, With<TextSpan>>()
        .iter(world)
        .map(|c| c.0)
        .collect();
    assert_eq!(values, [theme::INK, theme::INK, theme::INK]);
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
        .filter(|&(x, ..)| x < 0.0)
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

#[test]
fn each_hud_plate_covers_its_text_and_stays_in_the_left_panel() {
    use crate::frame::FRAME_Z;
    use crate::plate::PLATE_Z;
    let tops = [
        PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN,
        PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN - HUD_BLOCK_SPACING,
    ];
    for y in tops {
        let text = hud_text_rect(HUD_X, y);
        let plate = hud_plate_rect(HUD_X, y);
        assert_eq!(plate.union(text), plate, "covers the text");
        assert!(
            plate.min.x < text.min.x && plate.max.y > text.max.y,
            "padded"
        );
        // A 6-digit score fits (monospace ~0.6 em per glyph).
        assert!(text.width() >= HUD_MAX_DIGITS as f32 * 0.6 * HUD_FONT_SIZE - 1e-3);
        assert!(
            text.height() >= 2.0 * HUD_FONT_SIZE,
            "label and value lines"
        );
        assert!(plate.min.x > -WORLD_WIDTH / 2.0);
        assert!(plate.max.x < -PLAYFIELD_WIDTH / 2.0);
    }
    let (score, lives) = (
        hud_plate_rect(HUD_X, tops[0]),
        hud_plate_rect(HUD_X, tops[1]),
    );
    assert!(score.intersect(lives).is_empty(), "plates don't overlap");
    // Frame < plate < HUD text (z 1).
    const { assert!(FRAME_Z < PLATE_Z && PLATE_Z < 1.0) };
}

/// Each HUD backing plate: (centre, size, has its mesh).
fn hud_plates(app: &mut App) -> Vec<(Vec3, Vec2, bool)> {
    let world = app.world_mut();
    let mut plates: Vec<_> = world
        .query::<(&crate::plate::BackingPlate, &Transform, Has<Mesh2d>)>()
        .iter(world)
        .map(|(p, t, mesh)| (t.translation, p.size, mesh))
        .collect();
    // Left panel first, then top to bottom.
    plates.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(b.0.y.total_cmp(&a.0.y)));
    plates
}

#[test]
fn the_hud_sits_on_plates_that_come_and_go_with_the_run() {
    let mut app = app();
    app.update();
    let plates = hud_plates(&mut app);
    let top = PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN;
    let blocks = [
        (HUD_X, top),
        (HUD_X, top - HUD_BLOCK_SPACING),
        (HUD_RIGHT_X, top),
    ];
    assert_eq!(plates.len(), 3, "SCORE, LIVES and SECTOR");
    for ((centre, size, meshed), (x, y)) in plates.into_iter().zip(blocks) {
        let rect = hud_plate_rect(x, y);
        assert_eq!(centre, rect.center().extend(crate::plate::PLATE_Z));
        assert_eq!(size, rect.size());
        assert!(meshed);
    }

    tap(&mut app, KeyCode::Escape);
    crate::menu::test_helpers::press(&mut app, "Main menu");
    assert!(hud_plates(&mut app).is_empty(), "gone with the run");
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::InGame);
    app.update();
    app.update();
    assert_eq!(hud_plates(&mut app).len(), 3, "back on the next Start");
}

#[test]
fn the_sector_hud_sits_top_right_above_the_capsules() {
    use crate::powerups::capsules::{capsule_plate_rect, slot_y};
    let plate = hud_plate_rect(HUD_RIGHT_X, PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN);
    assert!(plate.min.x > PLAYFIELD_WIDTH / 2.0);
    assert!(plate.max.x < WORLD_WIDTH / 2.0);
    assert!(plate.max.y <= WORLD_HEIGHT / 2.0);
    assert!(
        plate.min.y > slot_y(0) + capsule_plate_rect().max.y,
        "capsule 0's plate is entirely below the sector plate"
    );
}

#[test]
fn a_fresh_run_shows_sector_01_in_ink() {
    let mut app = app();
    let world = app.world_mut();
    let sector: Vec<(String, Color)> = world
        .query_filtered::<(&TextSpan, &TextColor), With<SectorText>>()
        .iter(world)
        .map(|(t, c)| (t.0.clone(), c.0))
        .collect();
    assert_eq!(sector, [("01".to_string(), theme::INK)]);
}

// --- sim-w60.3 (human-code kata): format_score ---------------------------

#[test]
fn format_score_leaves_numbers_under_a_thousand_alone() {
    assert_eq!(format_score(0), "0");
    assert_eq!(format_score(7), "7");
    assert_eq!(format_score(999), "999");
}

#[test]
fn format_score_puts_a_comma_before_the_last_three_digits() {
    assert_eq!(format_score(1000), "1,000");
    assert_eq!(format_score(12340), "12,340");
    assert_eq!(format_score(999_999), "999,999");
}

#[test]
fn format_score_groups_every_three_digits() {
    assert_eq!(format_score(1_000_000), "1,000,000");
    assert_eq!(format_score(1_234_567), "1,234,567");
    assert_eq!(format_score(i32::MAX), "2,147,483,647");
}

#[test]
fn format_score_keeps_a_minus_sign_outside_the_groups() {
    assert_eq!(format_score(-5), "-5");
    assert_eq!(format_score(-1234), "-1,234");
    assert_eq!(format_score(-999_999), "-999,999");
    assert_eq!(format_score(i32::MIN), "-2,147,483,648");
}
