use super::*;
use crate::ball::{
    BALL_ANCHOR_GAP, BALL_MIN_VERTICAL_FRACTION, BALL_SIZE, BALL_SPEED, BALL_SPEED_SCALE,
    PADDLE_STILL_SPEED,
};
use crate::bricks::grid::{BRICK_GAP, BRICK_HEIGHT, BRICK_TOP_MARGIN};
use crate::paddle::{
    PADDLE_FORCE, PADDLE_HEIGHT, PADDLE_LINEAR_DAMPING, PADDLE_MARGIN_BOTTOM, PADDLE_MASS,
    PADDLE_WIDTH, PRONG_WIDTH,
};

#[test]
fn walls_sit_on_the_playfield_edges() {
    assert_eq!(SIDE_PANEL_WIDTH, 240.0);
    assert_eq!(PLAYFIELD_WIDTH + 2.0 * SIDE_PANEL_WIDTH, WORLD_WIDTH);
    assert_eq!(PLAYFIELD_HEIGHT, WORLD_HEIGHT);
    let [left, right, top] = wall_specs();
    assert_eq!(left.0.x + left.1.x / 2.0, -PLAYFIELD_WIDTH / 2.0);
    assert_eq!(right.0.x - right.1.x / 2.0, PLAYFIELD_WIDTH / 2.0);
    assert_eq!(top.0.y - top.1.y / 2.0, PLAYFIELD_HEIGHT / 2.0);
    // The well is centred and the walls close its corners.
    assert_eq!(left.0.x, -right.0.x);
    assert_eq!(top.0.x, 0.0);
    assert_eq!(
        left.1,
        Vec2::new(WALL_THICKNESS, PLAYFIELD_HEIGHT + 2.0 * WALL_THICKNESS)
    );
    assert_eq!(
        top.1,
        Vec2::new(PLAYFIELD_WIDTH + 2.0 * WALL_THICKNESS, WALL_THICKNESS)
    );
}

#[test]
fn gameplay_sizes_are_the_old_design_times_game_scale() {
    assert_eq!(GAME_SCALE, 1.5);
    for (scaled, design) in [
        (WALL_THICKNESS, 40.0),
        (PADDLE_WIDTH, 120.0),
        (PADDLE_HEIGHT, 20.0),
        (PADDLE_FORCE, 7000.0),
        (PADDLE_MARGIN_BOTTOM, 10.0),
        (PRONG_WIDTH, 27.0),
        (BALL_SIZE, 15.0),
        (BALL_ANCHOR_GAP, 2.0),
        (PADDLE_STILL_SPEED, 1.0),
        (BRICK_GAP, 5.0),
        (BRICK_TOP_MARGIN, 50.0),
        (BRICK_HEIGHT, 30.0),
    ] {
        assert_eq!(scaled, design * GAME_SCALE);
    }
    // The ball's speed is the exception: its own tunable scale.
    assert_eq!(BALL_SPEED_SCALE, 1.8);
    assert_eq!(BALL_SPEED, 300.0 * BALL_SPEED_SCALE);
    // Unit-free tuning stays put.
    assert_eq!(PADDLE_MASS, 3.0);
    assert_eq!(PADDLE_LINEAR_DAMPING, 4.0);
    assert_eq!(BALL_MIN_VERTICAL_FRACTION, 0.3);
}
