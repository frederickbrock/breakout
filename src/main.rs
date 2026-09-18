use bevy::color::palettes::basic::{BLUE, GREEN, RED, WHITE, YELLOW};
use bevy::color::palettes::css::ORANGE;
use bevy::prelude::*;
use bevy::sprite::Anchor;

// Game constants
const WINDOW_WIDTH: f32 = 900.0;
const WINDOW_HEIGHT: f32 = 650.0;
const PADDLE_WIDTH: f32 = 120.0;
const PADDLE_HEIGHT: f32 = 20.0;
const PADDLE_SPEED: f32 = 500.0;
const PADDLE_MARGIN_BOTTOM: f32 = 10.0;
const BALL_SIZE: f32 = 15.0;
const BALL_SPEED: f32 = 300.0;
const BRICK_WIDTH: f32 = 80.0;
const BRICK_HEIGHT: f32 = 30.0;
const BRICK_ROWS: usize = 5;
const BRICK_COLS: usize = 10;

#[derive(Component)]
struct Paddle;

#[derive(Component)]
struct Ball {
    velocity: Vec2,
}

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

#[derive(Resource)]
struct Lives(i32);

#[derive(Resource, Default, PartialEq, Clone, Copy)]
enum GameStatus {
    #[default]
    Playing,
    Lost,
    Won,
}

fn main() {
    // WSLg's Wayland compositor combined with the llvmpipe software Vulkan
    // renderer hits a surface-lost bug on window creation here. X11 (also
    // provided by WSLg) works reliably, and an empty value is treated the
    // same as unset by winit's backend auto-detection.
    // SAFETY: called at the very start of main, before any other thread
    // could read the environment.
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
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<Score>()
        .insert_resource(Lives(3))
        .init_resource::<GameStatus>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (restart_game, paddle_movement, ball_movement, update_ui).chain(),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.spawn((
        Sprite::from_color(WHITE, Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
        Transform::from_xyz(
            0.0,
            -WINDOW_HEIGHT / 2.0 + PADDLE_HEIGHT / 2.0 + PADDLE_MARGIN_BOTTOM,
            0.0,
        ),
        Paddle,
    ));

    commands.spawn((
        Sprite::from_color(WHITE, Vec2::splat(BALL_SIZE)),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Ball {
            velocity: Vec2::new(BALL_SPEED, -BALL_SPEED),
        },
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
                Brick,
            ));
        }
    }
}

fn paddle_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    status: Res<GameStatus>,
    mut paddle_query: Query<&mut Transform, With<Paddle>>,
) {
    if *status != GameStatus::Playing {
        return;
    }
    let Ok(mut transform) = paddle_query.single_mut() else {
        return;
    };

    let mut dx = 0.0;
    if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
        dx -= PADDLE_SPEED * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
        dx += PADDLE_SPEED * time.delta_secs();
    }

    let half_range = WINDOW_WIDTH / 2.0 - PADDLE_WIDTH / 2.0;
    transform.translation.x = (transform.translation.x + dx).clamp(-half_range, half_range);
}

fn ball_movement(
    mut commands: Commands,
    time: Res<Time>,
    mut status: ResMut<GameStatus>,
    mut score: ResMut<Score>,
    mut lives: ResMut<Lives>,
    paddle_query: Query<&Transform, (With<Paddle>, Without<Ball>)>,
    mut ball_query: Query<(&mut Transform, &mut Ball)>,
    brick_query: Query<(Entity, &Transform), (With<Brick>, Without<Ball>, Without<Paddle>)>,
) {
    if *status != GameStatus::Playing {
        return;
    }
    let Ok((mut ball_transform, mut ball)) = ball_query.single_mut() else {
        return;
    };

    let dt = time.delta_secs();
    ball_transform.translation.x += ball.velocity.x * dt;
    ball_transform.translation.y += ball.velocity.y * dt;

    // Wall collisions
    let half_w = WINDOW_WIDTH / 2.0 - BALL_SIZE / 2.0;
    let half_h = WINDOW_HEIGHT / 2.0 - BALL_SIZE / 2.0;

    if ball_transform.translation.x <= -half_w || ball_transform.translation.x >= half_w {
        ball.velocity.x *= -1.0;
        ball_transform.translation.x = ball_transform.translation.x.clamp(-half_w, half_w);
    }
    if ball_transform.translation.y >= half_h {
        ball.velocity.y *= -1.0;
        ball_transform.translation.y = half_h;
    }

    // Paddle collision
    if let Ok(paddle_transform) = paddle_query.single() {
        let paddle_top = paddle_transform.translation.y + PADDLE_HEIGHT / 2.0;
        let paddle_left = paddle_transform.translation.x - PADDLE_WIDTH / 2.0;
        let paddle_right = paddle_transform.translation.x + PADDLE_WIDTH / 2.0;
        let ball_bottom = ball_transform.translation.y - BALL_SIZE / 2.0;

        if ball.velocity.y < 0.0
            && ball_bottom <= paddle_top
            && ball_transform.translation.y >= paddle_transform.translation.y
            && ball_transform.translation.x >= paddle_left
            && ball_transform.translation.x <= paddle_right
        {
            ball.velocity.y *= -1.0;

            // Add spin based on where the ball hit the paddle
            let hit_pos = (ball_transform.translation.x - paddle_left) / PADDLE_WIDTH;
            ball.velocity.x = (hit_pos - 0.5) * BALL_SPEED * 2.0;
        }
    }

    // Ball fell off the bottom
    if ball_transform.translation.y < -WINDOW_HEIGHT / 2.0 {
        lives.0 -= 1;
        if lives.0 <= 0 {
            *status = GameStatus::Lost;
        } else {
            ball_transform.translation.x = 0.0;
            ball_transform.translation.y = 0.0;
            ball.velocity = Vec2::new(BALL_SPEED, -BALL_SPEED);
        }
        return;
    }

    // Ball collision with bricks
    let total_bricks = brick_query.iter().count();
    let mut hit = false;
    for (entity, brick_transform) in &brick_query {
        let dx = (ball_transform.translation.x - brick_transform.translation.x).abs();
        let dy = (ball_transform.translation.y - brick_transform.translation.y).abs();

        if dx <= (BRICK_WIDTH + BALL_SIZE) / 2.0 && dy <= (BRICK_HEIGHT + BALL_SIZE) / 2.0 {
            commands.entity(entity).despawn();
            ball.velocity.y *= -1.0;
            score.0 += 10;
            hit = true;
            break;
        }
    }

    if hit && total_bricks == 1 {
        *status = GameStatus::Won;
    }
}

fn restart_game(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut status: ResMut<GameStatus>,
    mut score: ResMut<Score>,
    mut lives: ResMut<Lives>,
    mut paddle_query: Query<&mut Transform, (With<Paddle>, Without<Ball>)>,
    mut ball_query: Query<(&mut Transform, &mut Ball), Without<Paddle>>,
    brick_query: Query<Entity, With<Brick>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyR) {
        return;
    }

    *status = GameStatus::Playing;
    score.0 = 0;
    lives.0 = 3;

    if let Ok(mut paddle_transform) = paddle_query.single_mut() {
        paddle_transform.translation.x = 0.0;
    }
    if let Ok((mut ball_transform, mut ball)) = ball_query.single_mut() {
        ball_transform.translation.x = 0.0;
        ball_transform.translation.y = 0.0;
        ball.velocity = Vec2::new(BALL_SPEED, -BALL_SPEED);
    }
    for entity in &brick_query {
        commands.entity(entity).despawn();
    }
    spawn_bricks(&mut commands);
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
