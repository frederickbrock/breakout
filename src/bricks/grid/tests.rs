use super::*;
use crate::bricks::BrickClass;
use crate::game_state::AppState;
use crate::paddle::{Paddle, PADDLE_HEIGHT};
use crate::run::Lives;
use crate::test_support::*;

fn layout(app: &mut App) -> Vec<(usize, usize, BrickClass)> {
    let mut layout: Vec<(usize, usize, BrickClass)> = app
        .world_mut()
        .query::<(&BrickCell, &BrickClass)>()
        .iter(app.world())
        .map(|(cell, class)| (cell.row, cell.col, *class))
        .collect();
    layout.sort_by_key(|(row, col, _)| (*row, *col));
    layout
}

#[test]
fn every_brick_gets_its_class_look_health_and_cell() {
    let mut app = app();
    assert_eq!(BRICK_ROWS * BRICK_COLS, 70);
    let world = app.world_mut();
    let mut cells = std::collections::HashSet::new();
    for (class, health, max, sprite, transform, cell) in world
        .query_filtered::<(
            &BrickClass,
            &BrickHealth,
            &BrickMaxHits,
            &Sprite,
            &Transform,
            &BrickCell,
        ), With<Brick>>()
        .iter(world)
    {
        assert_eq!(max.0, class.max_hits());
        assert_eq!(health.0, max.0);
        assert_eq!(sprite.color, theme::brick_color(*class));
        assert_eq!(transform.translation, brick_translation(*cell, BRICK_COLS));
        assert!(cell.row < 7 && cell.col < 10);
        assert!(cells.insert(*cell), "duplicate cell {cell:?}");
    }
    assert_eq!(cells.len(), 70);
}

#[test]
fn spawned_bricks_are_brick_sized_with_equal_side_channels() {
    let mut app = app();
    let world = app.world_mut();
    let (mut left, mut right) = (f32::INFINITY, f32::NEG_INFINITY);
    for (sprite, transform) in world
        .query_filtered::<(&Sprite, &Transform), With<Brick>>()
        .iter(world)
    {
        assert_eq!(
            sprite.custom_size,
            Some(Vec2::new(BRICK_WIDTH, BRICK_HEIGHT))
        );
        left = left.min(transform.translation.x - BRICK_WIDTH / 2.0);
        right = right.max(transform.translation.x + BRICK_WIDTH / 2.0);
    }
    let left_channel = left + PLAYFIELD_WIDTH / 2.0;
    let right_channel = PLAYFIELD_WIDTH / 2.0 - right;
    assert!((left_channel - right_channel).abs() <= 0.5);
    assert!(left_channel >= SIDE_CHANNEL - 1e-3);
    assert!(right_channel >= SIDE_CHANNEL - 1e-3);
}

#[test]
fn the_brick_grid_is_centred_with_equal_side_channels() {
    const { assert!(BRICK_WIDTH > 0.0) };
    const { assert!(BRICK_TOP_MARGIN >= 2.0 * BALL_SIZE) };
    for cols in [bricks::BOARD_COLS, bricks::BOARD_COLS - 3, 1] {
        let left = brick_x(0, cols) - BRICK_WIDTH / 2.0 + PLAYFIELD_WIDTH / 2.0;
        let right = PLAYFIELD_WIDTH / 2.0 - (brick_x(cols - 1, cols) + BRICK_WIDTH / 2.0);
        assert!(
            (left - right).abs() <= 0.5,
            "{cols} cols: {left} vs {right}"
        );
        assert!(left >= SIDE_CHANNEL - 1e-3, "{cols} cols: {left}");
        assert!(right >= SIDE_CHANNEL - 1e-3, "{cols} cols: {right}");
        if cols > 1 {
            let pitch = brick_x(1, cols) - brick_x(0, cols);
            assert!((pitch - (BRICK_WIDTH + BRICK_GAP)).abs() < 1e-3);
        }
    }
    // A full board uses exactly the minimum channel.
    let full_left = brick_x(0, bricks::BOARD_COLS) - BRICK_WIDTH / 2.0 + PLAYFIELD_WIDTH / 2.0;
    assert!((full_left - SIDE_CHANNEL).abs() < 1e-2);
    // Rows step down from the top offset.
    assert_eq!(
        brick_y(0) + BRICK_HEIGHT / 2.0,
        PLAYFIELD_HEIGHT / 2.0 - BRICK_TOP_OFFSET
    );
    assert!((brick_y(0) - brick_y(1) - (BRICK_HEIGHT + BRICK_GAP)).abs() < 1e-3);
}

#[test]
fn the_board_layout_changes_between_runs() {
    let mut app = app();
    let first = layout(&mut app);
    tap(&mut app, KeyCode::Space);
    app.world_mut().resource_mut::<Lives>().0 = 1;
    move_ball_below_screen(&mut app);
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::GameOver);
    tap(&mut app, KeyCode::KeyR);
    let second = layout(&mut app);
    assert_eq!(second.len(), 70);
    assert_ne!(first, second);
}

#[test]
fn seven_rows_leave_room_above_the_paddle() {
    let mut app = app();
    let world = app.world_mut();
    let lowest = world
        .query_filtered::<&Transform, With<Brick>>()
        .iter(world)
        .map(|t| t.translation.y)
        .fold(f32::INFINITY, f32::min);
    let rows = world.query::<&BrickCell>().iter(world).map(|c| c.row).max();
    assert_eq!(rows, Some(6));
    let paddle_top = world
        .query_filtered::<&Transform, With<Paddle>>()
        .single(world)
        .unwrap()
        .translation
        .y
        + PADDLE_HEIGHT / 2.0;
    assert!(lowest - BRICK_HEIGHT / 2.0 - paddle_top >= 250.0 * GAME_SCALE);
}

#[test]
fn the_grid_sits_one_brick_height_lower_for_headroom() {
    // The top of row 0 is 120 below the top wall (was 75).
    let top_wall = PLAYFIELD_HEIGHT / 2.0;
    let row0_top = brick_y(0) + BRICK_HEIGHT / 2.0;
    assert!((top_wall - row0_top - 120.0).abs() < 1e-3);
    assert_eq!(BRICK_HEADROOM, BRICK_HEIGHT);
    // Every row moved down by exactly one brick height, spacing unchanged.
    let old_y = |row: usize| {
        top_wall - BRICK_TOP_MARGIN - BRICK_HEIGHT / 2.0 - row as f32 * (BRICK_HEIGHT + BRICK_GAP)
    };
    for row in 0..bricks::BOARD_ROWS {
        assert!(
            (old_y(row) - brick_y(row) - BRICK_HEIGHT).abs() < 1e-3,
            "row {row}"
        );
    }
}

#[test]
fn ten_rows_leave_room_above_the_paddle() {
    use crate::paddle::PADDLE_MARGIN_BOTTOM;
    let paddle_top = -PLAYFIELD_HEIGHT / 2.0 + PADDLE_MARGIN_BOTTOM + PADDLE_HEIGHT;
    let lowest = brick_y(crate::levels::MAX_ROWS - 1) - BRICK_HEIGHT / 2.0;
    assert!(
        lowest - paddle_top >= 250.0 * GAME_SCALE,
        "{lowest} vs {paddle_top}"
    );
}

#[test]
fn board_size_is_the_level_grid_including_empty_rows() {
    let app = app();
    assert_eq!(
        *app.world().resource::<BoardSize>(),
        BoardSize {
            cols: BRICK_COLS,
            rows: BRICK_ROWS
        }
    );
    let level = crate::levels::parse_level("grid:\nCC.\n...\n...").unwrap();
    let app = app_with_level(level);
    assert_eq!(
        *app.world().resource::<BoardSize>(),
        BoardSize { cols: 3, rows: 3 }
    );
}
