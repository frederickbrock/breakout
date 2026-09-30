use super::*;
use crate::powerups;
use crate::test_support::*;
use crate::world::PLAYFIELD_WIDTH;

fn paddle_state(app: &mut App) -> (f32, Vec2, Vec2) {
    let paddle = paddle(app);
    let entity = app.world().entity(paddle);
    (
        entity.get::<Transform>().unwrap().translation.x,
        entity.get::<LinearVelocity>().unwrap().0,
        entity.get::<ConstantForce>().unwrap().0,
    )
}

fn hold(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
}

fn paddle_look(app: &mut App) -> (Vec<f32>, f32, f32) {
    let paddle = paddle(app);
    let width = app.world().get::<Paddle>(paddle).unwrap().width;
    let world = app.world_mut();
    let mut prongs: Vec<f32> = world
        .query_filtered::<&Transform, With<PaddleProng>>()
        .iter(world)
        .map(|t| t.translation.x)
        .collect();
    prongs.sort_by(f32::total_cmp);
    let field = world
        .query_filtered::<&Sprite, With<PaddleField>>()
        .single(world)
        .unwrap()
        .custom_size
        .unwrap()
        .x;
    (prongs, field, width)
}

#[test]
fn in_mouse_mode_the_paddle_heads_for_the_cursor() {
    let mut app = app();
    aim_mouse_at(&mut app, 200.0);
    app.update();
    let (_, v, force) = paddle_state(&mut app);
    assert!(v.x > 0.0, "moving right toward the cursor, got {v:?}");
    assert_eq!(force, Vec2::ZERO);

    set_paddle_x(&mut app, 300.0);
    aim_mouse_at(&mut app, -200.0);
    app.update();
    assert!(paddle_state(&mut app).1.x < 0.0);
}

#[test]
fn the_mouse_target_is_clamped_to_the_walls() {
    let mut app = app();
    let edge = (PLAYFIELD_WIDTH - PADDLE_WIDTH) / 2.0;
    set_paddle_x(&mut app, edge);
    aim_mouse_at(&mut app, 10_000.0);
    app.update();
    assert_eq!(paddle_state(&mut app).1.x, 0.0, "already at the wall");
}

#[test]
fn in_keyboard_mode_the_mouse_does_not_move_the_paddle() {
    let mut app = app();
    app.world_mut().resource_mut::<ControlSettings>().paddle = PaddleControl::Keyboard;
    aim_mouse_at(&mut app, 300.0);
    app.update();
    assert_eq!(paddle_state(&mut app).1.x, 0.0);

    hold(&mut app, KeyCode::KeyD);
    assert_eq!(paddle_state(&mut app).2.x, PADDLE_FORCE);
}

#[test]
fn in_mouse_mode_keys_still_move_the_paddle_and_take_over() {
    let mut app = app();
    aim_mouse_at(&mut app, 300.0);
    hold(&mut app, KeyCode::ArrowLeft);

    assert_eq!(paddle_state(&mut app).2.x, -PADDLE_FORCE);
    assert_eq!(app.world().resource::<PaddleTarget>().x, None);
}

#[test]
fn the_paddle_prongs_stay_at_its_ends_when_it_widens() {
    let mut app = app();
    let prong_xs = |app: &mut App| {
        let mut xs: Vec<f32> = app
            .world_mut()
            .query_filtered::<&Transform, With<PaddleProng>>()
            .iter(app.world())
            .map(|t| t.translation.x)
            .collect();
        xs.sort_by(f32::total_cmp);
        xs
    };
    let edge = (PADDLE_WIDTH - PRONG_WIDTH) / 2.0;
    assert_eq!(prong_xs(&mut app), [-edge, edge]);

    // Super-Sizer widens the paddle (it recomputes the width every frame).
    app.world_mut().trigger(powerups::PowerUpCollected {
        kind: powerups::PowerUpKind::SuperSizer,
    });
    app.update();
    let paddle = paddle(&mut app);
    let width = app.world().get::<Paddle>(paddle).unwrap().width;
    assert!(width > PADDLE_WIDTH);
    let wide_edge = (width - PRONG_WIDTH) / 2.0;
    assert_eq!(prong_xs(&mut app), [-wide_edge, wide_edge]);
}

#[test]
fn paddle_pieces_fit_seamlessly_at_normal_and_super_sized_widths() {
    for (width, offset, field) in [
        (PADDLE_WIDTH, 46.5 * GAME_SCALE, 66.0 * GAME_SCALE),
        (PADDLE_WIDTH * 1.25, 61.5 * GAME_SCALE, 96.0 * GAME_SCALE),
    ] {
        let pieces = paddle_pieces(width);
        assert_eq!(
            pieces,
            PaddlePieces {
                prong_offset: offset,
                field_width: field
            }
        );
        // Outer prong edge on the paddle's edge; inner edge meets the field.
        assert_eq!(pieces.prong_offset + PRONG_WIDTH / 2.0, width / 2.0);
        assert_eq!(
            pieces.prong_offset - PRONG_WIDTH / 2.0,
            pieces.field_width / 2.0
        );
    }
}

#[test]
fn the_paddle_is_two_prongs_and_a_field_that_follow_super_sizer() {
    let mut app = app();
    let paddle = paddle(&mut app);
    assert!(
        !app.world().entity(paddle).contains::<Sprite>(),
        "drawn by its pieces"
    );
    assert_eq!(
        paddle_look(&mut app),
        (
            vec![-46.5 * GAME_SCALE, 46.5 * GAME_SCALE],
            66.0 * GAME_SCALE,
            PADDLE_WIDTH
        )
    );

    app.world_mut().trigger(powerups::PowerUpCollected {
        kind: powerups::PowerUpKind::SuperSizer,
    });
    app.update();
    assert_eq!(
        paddle_look(&mut app),
        (
            vec![-61.5 * GAME_SCALE, 61.5 * GAME_SCALE],
            96.0 * GAME_SCALE,
            PADDLE_WIDTH * 1.25
        )
    );

    // The effect runs out (7 s at the test app's 100 ms step).
    for _ in 0..80 {
        app.update();
    }
    assert_eq!(
        paddle_look(&mut app),
        (
            vec![-46.5 * GAME_SCALE, 46.5 * GAME_SCALE],
            66.0 * GAME_SCALE,
            PADDLE_WIDTH
        )
    );
}
