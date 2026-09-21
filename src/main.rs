mod powerups;
mod spawner;

use avian2d::prelude::*;
use bevy::color::palettes::basic::{BLUE, GREEN, RED, WHITE, YELLOW};
use bevy::color::palettes::css::ORANGE;
use bevy::prelude::*;
use bevy::sprite::Anchor;

// Game constants
const WINDOW_WIDTH: f32 = 900.0;
const WINDOW_HEIGHT: f32 = 650.0;
const WALL_THICKNESS: f32 = 40.0;
const PADDLE_WIDTH: f32 = 120.0;
const PADDLE_HEIGHT: f32 = 20.0;
const PADDLE_MASS: f32 = 5.0;
const PADDLE_FORCE: f32 = 4000.0;
const PADDLE_LINEAR_DAMPING: f32 = 4.0;
const PADDLE_MARGIN_BOTTOM: f32 = 10.0;
const BALL_SIZE: f32 = 15.0;
const BALL_SPEED: f32 = 300.0;
// Guards against a real failure mode observed in testing: a wall bounce only
// inverts the velocity component perpendicular to the wall, so a ball that
// ends up moving near-perfectly horizontally between the side walls (below
// the bricks, above the paddle) can get permanently stuck bouncing
// side-to-side forever, since nothing left in that lane can ever touch its Y
// velocity again. Keeping a minimum vertical fraction guarantees the ball
// always keeps drifting toward the bricks or the paddle.
const BALL_MIN_VERTICAL_FRACTION: f32 = 0.2;
const BRICK_WIDTH: f32 = 80.0;
const BRICK_HEIGHT: f32 = 30.0;
const BRICK_ROWS: usize = 5;
const BRICK_COLS: usize = 10;

#[derive(Component)]
struct Paddle {
    width: f32,
}

#[derive(Component)]
struct Ball;

#[derive(Component)]
struct Brick;

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct LivesText;

#[derive(Component)]
struct GameOverText;

#[derive(Resource, Default)]
struct Score(i32);

#[derive(Resource, Default)]
struct Lives(i32);

#[derive(Resource, Default, PartialEq, Clone, Copy)]
enum GameStatus {
    #[default]
    Playing,
    Lost,
    Won,
}

/// Broadcast when the player presses R to restart. Each subsystem that has
/// its own state to reset (currently just power-ups) registers an observer
/// on this instead of `restart_game` reaching into every subsystem by hand —
/// a future obstacles or brick-respawn subsystem resets itself the same way,
/// with no changes needed here.
#[derive(Event)]
struct RestartGame;


/// Broadcast when the player presses Q to quit the game. Each subsystem
/// that has its own state shutdowns
#[derive(Event)]
struct QuitGamme;

/// Lets other systems (e.g. a power-up that changes paddle width) declare
/// they must run before paddle movement each frame, without `main.rs` having
/// to manually interleave their systems into its own `Update` chain.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct PaddleMovementSet;

fn main() {
    // WSLg's Wayland compositor combined with the llvmpipe software Vulkan
    // renderer hits a surface-lost bug on window creation here. X11 (also
    // provided by WSLg) works reliably, and an empty value is treated the
    // same as unset by winit's backend auto-detection.
    // SAFETY: called at the very start of main, before any other thread
    // could read the environment.
    #[cfg(not(target_arch = "wasm32"))]
    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", "");
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Breakout".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PhysicsPlugins::default())
        .insert_resource(Gravity(Vec2::new(0.0,0.1)))
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<Score>()
        .insert_resource(Lives(5))
        .init_resource::<GameStatus>()
        .init_resource::<BallCollisionSignals>()
        .add_observer(on_ball_collision)
        .add_plugins(powerups::PowerUpsPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                restart_game,
                paddle_movement.in_set(PaddleMovementSet),
                ball_movement,
                update_ui,
            )
                .chain(),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Static walls the ball (and paddle) physically bounce off, instead of
    // manual clamp/reflect code. No bottom wall — a ball reaching the bottom
    // is a life lost, checked separately from physics.
    let wall_specs = [
        // left
        (
            -WINDOW_WIDTH / 2.0 - WALL_THICKNESS / 2.0,
            0.0,
            WALL_THICKNESS,
            WINDOW_HEIGHT + WALL_THICKNESS * 2.0,
        ),
        // right
        (
            WINDOW_WIDTH / 2.0 + WALL_THICKNESS / 2.0,
            0.0,
            WALL_THICKNESS,
            WINDOW_HEIGHT + WALL_THICKNESS * 2.0,
        ),
        // top
        (
            0.0,
            WINDOW_HEIGHT / 2.0 + WALL_THICKNESS / 2.0,
            WINDOW_WIDTH + WALL_THICKNESS * 2.0,
            WALL_THICKNESS,
        ),
    ];
    for (x, y, w, h) in wall_specs {
        commands.spawn((
            RigidBody::Static,
            Collider::rectangle(w, h),
            Transform::from_xyz(x, y, 0.0),
        ));
    }

    commands.spawn((
        Sprite::from_color(WHITE, Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
        Transform::from_xyz(
            0.0,
            -WINDOW_HEIGHT / 2.0 + PADDLE_HEIGHT / 2.0 + PADDLE_MARGIN_BOTTOM,
            0.0,
        ),
        RigidBody::Dynamic,
        Collider::rectangle(PADDLE_WIDTH, PADDLE_HEIGHT),
        Mass(PADDLE_MASS),
        LockedAxes::new().lock_translation_y().lock_rotation(),
        LinearDamping(PADDLE_LINEAR_DAMPING),
        Restitution::ZERO,
        ConstantForce(Vec2::ZERO),
        Paddle {
            width: PADDLE_WIDTH,
        },
    ));

    commands.spawn((
        Sprite::from_color(WHITE, Vec2::splat(BALL_SIZE)),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RigidBody::Dynamic,
        Collider::circle(BALL_SIZE / 2.0),
        LinearVelocity(Vec2::new(BALL_SPEED, -BALL_SPEED)),
        LockedAxes::ROTATION_LOCKED,
        Restitution::new(1.0),
        Friction::ZERO,
        CollisionEventsEnabled,
        Ball,
    ));

    spawn_bricks(&mut commands);

    commands.spawn((
        Text2d::new("Score: 0"),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(WHITE.into()),
        Anchor::TOP_LEFT,
        Transform::from_xyz(-WINDOW_WIDTH / 2.0 + 20.0, WINDOW_HEIGHT / 2.0 - 10.0, 1.0),
        ScoreText,
    ));

    commands.spawn((
        Text2d::new("Lives: 3"),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(WHITE.into()),
        Anchor::TOP_LEFT,
        Transform::from_xyz(-WINDOW_WIDTH / 2.0 + 20.0, WINDOW_HEIGHT / 2.0 - 40.0, 1.0),
        LivesText,
    ));

    commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: FontSize::Px(32.0),
            ..default()
        },
        TextColor(YELLOW.into()),
        Anchor::CENTER,
        Transform::from_xyz(0.0, 0.0, 1.0),
        GameOverText,
    ));
}

fn spawn_bricks(commands: &mut Commands) {
    let colors = [RED, ORANGE, YELLOW, GREEN, BLUE];

    for row in 0..BRICK_ROWS {
        for col in 0..BRICK_COLS {
            let x = col as f32 * (BRICK_WIDTH + 5.0) + 40.0 + BRICK_WIDTH / 2.0 - WINDOW_WIDTH / 2.0;
            let y = WINDOW_HEIGHT / 2.0
                - (row as f32 * (BRICK_HEIGHT + 5.0) + 50.0 + BRICK_HEIGHT / 2.0);

            commands.spawn((
                Sprite::from_color(colors[row % colors.len()], Vec2::new(BRICK_WIDTH, BRICK_HEIGHT)),
                Transform::from_xyz(x, y, 0.0),
                RigidBody::Static,
                Collider::rectangle(BRICK_WIDTH, BRICK_HEIGHT),
                Brick,
            ));
        }
    }
}

fn paddle_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    status: Res<GameStatus>,
    mut paddle_query: Query<&mut ConstantForce, With<Paddle>>,
) {
    let Ok(mut force) = paddle_query.single_mut() else {
        return;
    };
    if *status != GameStatus::Playing {
        force.0 = Vec2::ZERO;
        return;
    }

    let mut fx = 0.0;
    if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
        fx -= PADDLE_FORCE;
    }
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
        fx += PADDLE_FORCE;
    }
    force.0 = Vec2::new(fx, 0.0);
}

/// Avian's `CollisionStart`/`CollisionEnd` are dispatched purely through
/// `World::trigger` (an observer notification), never written to a message
/// queue — so despite `CollisionStart` deriving `Message`, a
/// `MessageReader<CollisionStart>` never receives anything. This observer is
/// the real way to react to it. Only the ball has `CollisionEventsEnabled`,
/// and Avian guarantees the enabled side always ends up as `collider1`, so
/// `on.collider1` is always the ball here.
fn on_ball_collision(
    on: On<CollisionStart>,
    mut commands: Commands,
    status: Res<GameStatus>,
    mut score: ResMut<Score>,
    mut signals: ResMut<BallCollisionSignals>,
    brick_query: Query<(), With<Brick>>,
    paddle_query: Query<&Transform, With<Paddle>>
) {
    if *status != GameStatus::Playing {
        return;
    }
    let other = on.collider2;
    if brick_query.get(other).is_ok() {
        commands.entity(other).despawn();
        score.0 += 10;
        signals.broke_brick = true;
    } else if let Ok(paddle_transform) = paddle_query.get(other) {
        signals.paddle_hit_x = Some(paddle_transform.translation.x);
    }
}

/// What happened in this frame's ball collisions, recorded by
/// [`on_ball_collision`] and consumed once per frame by [`ball_movement`].
#[derive(Resource, Default)]
struct BallCollisionSignals {
    broke_brick: bool,
    paddle_hit_x: Option<f32>,
}

/// Avian resolves the actual collision physics (detection + bounce angle);
/// this reacts to what [`on_ball_collision`] recorded (score, the paddle-hit
/// "spin" feel) and keeps the ball's speed at a controlled, designed
/// magnitude rather than letting raw momentum transfer drift it.

fn ball_movement(
    mut status: ResMut<GameStatus>,
    mut lives: ResMut<Lives>,
    mut signals: ResMut<BallCollisionSignals>,
    paddle_query: Query<(&Transform, &Paddle), (Without<Ball>, Without<Brick>)>,
    brick_query: Query<(), (With<Brick>, Without<Ball>, Without<Paddle>)>,
    mut ball_query: Query<(&mut Transform, &mut LinearVelocity), (With<Ball>, Without<Paddle>, Without<Brick>)>,
) {
    let broke_brick = signals.broke_brick;
    let paddle_hit_x = signals.paddle_hit_x;
    *signals = BallCollisionSignals::default();

    if *status != GameStatus::Playing {
        return;
    }
    let Ok((mut ball_transform, mut ball_velocity)) = ball_query.single_mut() else {
        return;
    };

    // Reapply the arcade "spin based on where it hit the paddle" feel —
    // Avian's own contact response doesn't know about this custom rule.
    if let Some(paddle_x) = paddle_hit_x {
        if let Ok((_, paddle)) = paddle_query.single() {
            let paddle_left = paddle_x - paddle.width / 2.0;
            let hit_pos = (ball_transform.translation.x - paddle_left) / paddle.width;
            ball_velocity.0.x = (hit_pos - 0.5) * BALL_SPEED * 2.0;
            ball_velocity.0.y = ball_velocity.0.y.abs();
        }
    }

    // Keep the ball's speed at a controlled magnitude instead of whatever
    // Avian's momentum transfer produced, and enforce a minimum vertical
    // component so it can't get stuck in a purely horizontal bounce loop.
    if ball_velocity.0 != Vec2::ZERO {
        let mut v = ball_velocity.0.normalize() * BALL_SPEED;
        let min_y = BALL_SPEED * BALL_MIN_VERTICAL_FRACTION;
        if v.y.abs() < min_y {
            let y_sign = if v.y < 0.0 { -1.0 } else { 1.0 };
            let x_sign = if v.x < 0.0 { -1.0 } else { 1.0 };
            v.y = min_y * y_sign;
            let remaining_x = (BALL_SPEED * BALL_SPEED - v.y * v.y).max(0.0).sqrt();
            v.x = remaining_x * x_sign;
        }
        ball_velocity.0 = v;
    }

    // `<= 1` rather than `== 0` covers both cases: the despawn command from
    // `on_ball_collision` may or may not have been applied yet by the time
    // this system runs this same frame.
    if broke_brick && brick_query.iter().count() <= 1 {
        *status = GameStatus::Won;
        // Avian keeps simulating regardless of our GameStatus, so freeze the
        // ball in place ourselves once the game is over, or it'll keep
        // sailing off-screen after we stop reacting to it.
        ball_velocity.0 = Vec2::ZERO;
    }

    // Ball fell off the bottom (no physical wall there, so this stays a
    // plain position check rather than a collision).
    if ball_transform.translation.y < -WINDOW_HEIGHT / 2.0 {
        lives.0 -= 1;
        if lives.0 <= 0 {
            *status = GameStatus::Lost;
            ball_velocity.0 = Vec2::ZERO;
        } else {
            ball_transform.translation.x = 0.0;
            ball_transform.translation.y = 0.0;
            ball_velocity.0 = Vec2::new(BALL_SPEED, -BALL_SPEED);
        }
    }
}

fn restart_game(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut status: ResMut<GameStatus>,
    mut score: ResMut<Score>,
    mut lives: ResMut<Lives>,
    mut paddle_query: Query<(&mut Transform, &mut LinearVelocity), (With<Paddle>, Without<Ball>)>,
    mut ball_query: Query<(&mut Transform, &mut LinearVelocity), (With<Ball>, Without<Paddle>)>,
    brick_query: Query<Entity, With<Brick>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyR) {
        return;
    }

    *status = GameStatus::Playing;
    score.0 = 0;
    lives.0 = 3;

    if let Ok((mut paddle_transform, mut paddle_velocity)) = paddle_query.single_mut() {
        paddle_transform.translation.x = 0.0;
        paddle_velocity.0 = Vec2::ZERO;
    }
    if let Ok((mut ball_transform, mut ball_velocity)) = ball_query.single_mut() {
        ball_transform.translation.x = 0.0;
        ball_transform.translation.y = 0.0;
        ball_velocity.0 = Vec2::new(BALL_SPEED, -BALL_SPEED);
    }
    for entity in &brick_query {
        commands.entity(entity).despawn();
    }
    spawn_bricks(&mut commands);
    commands.trigger(RestartGame);
}

fn update_ui(
    score: Res<Score>,
    lives: Res<Lives>,
    status: Res<GameStatus>,
    mut score_text: Query<&mut Text2d, (With<ScoreText>, Without<LivesText>, Without<GameOverText>)>,
    mut lives_text: Query<&mut Text2d, (With<LivesText>, Without<ScoreText>, Without<GameOverText>)>,
    mut game_over_text: Query<&mut Text2d, (With<GameOverText>, Without<ScoreText>, Without<LivesText>)>,
) {
    if let Ok(mut text) = score_text.single_mut() {
        text.0 = format!("Score: {}", score.0);
    }
    if let Ok(mut text) = lives_text.single_mut() {
        text.0 = format!("Lives: {}", lives.0);
    }
    if let Ok(mut text) = game_over_text.single_mut() {
        text.0 = match *status {
            GameStatus::Playing => String::new(),
            GameStatus::Lost => "GAME OVER - Press R to Restart".to_string(),
            GameStatus::Won => "YOU WIN! - Press R to Restart".to_string(),
        };
    }
}
