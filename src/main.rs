mod game_state;
mod powerups;
mod script_manager;
mod spawner;

use avian2d::prelude::*;
use bevy::color::palettes::basic::{BLUE, GREEN, RED, WHITE, YELLOW};
use bevy::color::palettes::css::ORANGE;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use game_state::{AppState, GameOutcome, GameStatePlugin, PlayState};

// Game constants
const WINDOW_WIDTH: f32 = 900.0;
const WINDOW_HEIGHT: f32 = 650.0;
const WALL_THICKNESS: f32 = 40.0;
const PADDLE_WIDTH: f32 = 120.0;
const PADDLE_HEIGHT: f32 = 20.0;
const PADDLE_MASS: f32 = 3.0;
const PADDLE_FORCE: f32 = 7000.0;
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
const BALL_MIN_VERTICAL_FRACTION: f32 = 0.3;
const BRICK_WIDTH: f32 = 80.0;
const BRICK_HEIGHT: f32 = 30.0;
const BRICK_ROWS: usize = 6;
const BRICK_COLS: usize = 10;
/// Lives at the start of every run, including the first.
const STARTING_LIVES: i32 = 3;

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
struct OverlayText;

#[derive(Resource, Default)]
struct Score(i32);

#[derive(Resource, Default)]
struct Lives(i32);

/// Broadcast at the start of every run (entering [`AppState::InGame`]). Each
/// subsystem that has its own state to reset (currently just power-ups)
/// registers an observer on this instead of `start_run` reaching into every
/// subsystem by hand — a future obstacles or brick-respawn subsystem resets
/// itself the same way, with no changes needed here.
#[derive(Event)]
struct RestartGame;

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

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Breakout".into(),
            resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(PhysicsPlugins::default())
    .add_plugins(script_manager::ScriptPlugin)
    .insert_resource(ClearColor(Color::BLACK));
    add_game(&mut app);
    app.run();
}

/// Everything game-specific, on top of the engine plugins (`DefaultPlugins`,
/// Avian, scripting) that `main` adds. Split out so tests can run the real
/// game logic on a headless `MinimalPlugins` app.
fn add_game(app: &mut App) {
    app.add_plugins(GameStatePlugin)
        .insert_resource(Gravity(Vec2::new(0.0, 0.8)))
        .init_resource::<Score>()
        .insert_resource(Lives(STARTING_LIVES))
        .init_resource::<BallCollisionSignals>()
        .add_observer(on_ball_collision)
        .add_plugins(powerups::PowerUpsPlugin)
        .add_systems(Startup, setup_level)
        .add_systems(OnEnter(AppState::InGame), start_run)
        .add_systems(
            Update,
            (
                (paddle_movement.in_set(PaddleMovementSet), ball_movement)
                    .chain()
                    .run_if(in_state(PlayState::Playing)),
                restart_from_game_over.run_if(in_state(AppState::GameOver)),
                update_hud.run_if(in_state(AppState::InGame)),
                update_overlay,
            )
                .chain(),
        );
}

/// Starts a fresh run: resets the counters this module owns, spawns the
/// run's entities (all scoped to [`AppState::InGame`], so leaving the run
/// despawns them), and broadcasts [`RestartGame`] for every other subsystem.
fn start_run(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut lives: ResMut<Lives>,
    mut signals: ResMut<BallCollisionSignals>,
) {
    score.0 = 0;
    lives.0 = STARTING_LIVES;
    *signals = BallCollisionSignals::default();
    spawn_run_entities(&mut commands);
    commands.trigger(RestartGame);
}

fn spawn_run_entities(commands: &mut Commands) {
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Sprite::from_color(RED, Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
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
        DespawnOnExit(AppState::InGame),
    ));

    spawn_bricks(commands);

    commands.spawn((
        DespawnOnExit(AppState::InGame),
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
        DespawnOnExit(AppState::InGame),
        Text2d::new(format!("Lives: {STARTING_LIVES}")),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(WHITE.into()),
        Anchor::TOP_LEFT,
        Transform::from_xyz(-WINDOW_WIDTH / 2.0 + 20.0, WINDOW_HEIGHT / 2.0 - 40.0, 1.0),
        LivesText,
    ));
}

fn setup_level(mut commands: Commands) {
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

    // Pause / game-over / win message. Global rather than run-scoped so it
    // stays up on the game-over screen after the run's entities are gone.
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: FontSize::Px(32.0),
            ..default()
        },
        TextColor(YELLOW.into()),
        Anchor::CENTER,
        Transform::from_xyz(0.0, 0.0, 1.0),
        OverlayText,
    ));
}

fn spawn_bricks(commands: &mut Commands) {
    let colors = [RED, ORANGE, YELLOW, GREEN, BLUE];

    for row in 0..BRICK_ROWS {
        for col in 0..BRICK_COLS {
            let x =
                col as f32 * (BRICK_WIDTH + 5.0) + 40.0 + BRICK_WIDTH / 2.0 - WINDOW_WIDTH / 2.0;
            let y = WINDOW_HEIGHT / 2.0
                - (row as f32 * (BRICK_HEIGHT + 5.0) + 50.0 + BRICK_HEIGHT / 2.0);

            commands.spawn((
                Sprite::from_color(
                    colors[row % colors.len()],
                    Vec2::new(BRICK_WIDTH, BRICK_HEIGHT),
                ),
                Transform::from_xyz(x, y, 0.0),
                RigidBody::Static,
                Collider::rectangle(BRICK_WIDTH, BRICK_HEIGHT),
                Brick,
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

fn paddle_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut paddle_query: Query<&mut ConstantForce, With<Paddle>>,
) {
    let Ok(mut force) = paddle_query.single_mut() else {
        return;
    };

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
/// `on.collider1` is always the ball here. No state check is needed: the
/// physics clock only runs while `InGame/Playing`, so no collisions fire
/// outside it.
fn on_ball_collision(
    on: On<CollisionStart>,
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut signals: ResMut<BallCollisionSignals>,
    brick_query: Query<(), With<Brick>>,
    paddle_query: Query<&Transform, With<Paddle>>,
) {
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
/// magnitude rather than letting raw momentum transfer drift it. Ends the
/// run (switches to [`AppState::GameOver`]) on a win or on losing the last
/// life; the physics clock stops with it, so nothing needs zeroing here.
fn ball_movement(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut lives: ResMut<Lives>,
    mut signals: ResMut<BallCollisionSignals>,
    paddle_query: Query<(&Transform, &Paddle), Without<Ball>>,
    brick_query: Query<(), With<Brick>>,
    mut ball_query: Query<(&mut Transform, &mut LinearVelocity), With<Ball>>,
) {
    let broke_brick = signals.broke_brick;
    let paddle_hit_x = signals.paddle_hit_x;
    *signals = BallCollisionSignals::default();

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
        end_run(&mut commands, &mut next_state, GameOutcome::Won);
        return;
    }

    // Ball fell off the bottom (no physical wall there, so this stays a
    // plain position check rather than a collision).
    if ball_transform.translation.y < -WINDOW_HEIGHT / 2.0 {
        lives.0 -= 1;
        if lives.0 <= 0 {
            end_run(&mut commands, &mut next_state, GameOutcome::Lost);
        } else {
            ball_transform.translation.x = 0.0;
            ball_transform.translation.y = 0.0;
            ball_velocity.0 = Vec2::new(BALL_SPEED, -BALL_SPEED);
        }
    }
}

fn end_run(commands: &mut Commands, next_state: &mut NextState<AppState>, outcome: GameOutcome) {
    commands.insert_resource(outcome);
    next_state.set(AppState::GameOver);
}

/// R on the game-over/win screen starts a new run; entering
/// [`AppState::InGame`] runs [`start_run`], which does the actual resetting.
fn restart_from_game_over(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        next_state.set(AppState::InGame);
    }
}

fn update_hud(
    score: Res<Score>,
    lives: Res<Lives>,
    mut score_text: Query<&mut Text2d, (With<ScoreText>, Without<LivesText>)>,
    mut lives_text: Query<&mut Text2d, (With<LivesText>, Without<ScoreText>)>,
) {
    if let Ok(mut text) = score_text.single_mut() {
        text.0 = format!("Score: {}", score.0);
    }
    if let Ok(mut text) = lives_text.single_mut() {
        text.0 = format!("Lives: {}", lives.0);
    }
}

fn update_overlay(
    app_state: Res<State<AppState>>,
    play_state: Option<Res<State<PlayState>>>,
    outcome: Option<Res<GameOutcome>>,
    mut overlay: Query<&mut Text2d, With<OverlayText>>,
) {
    let Ok(mut text) = overlay.single_mut() else {
        return;
    };
    let message = match (app_state.get(), play_state.as_deref().map(State::get)) {
        (AppState::InGame, Some(PlayState::Paused)) => "Paused - Press P or Esc to Resume",
        (AppState::GameOver, _) => match outcome.as_deref() {
            Some(GameOutcome::Won) => "YOU WIN! - Press R to Restart",
            Some(GameOutcome::Lost) | None => "GAME OVER - Press R to Restart",
        },
        _ => "",
    };
    if text.0 != message {
        text.0 = message.to_string();
    }
}

/// Headless app running the real game logic (no window, renderer or
/// scripting), with a fixed 100 ms step per `update()` and keyboard input
/// driven by hand via [`tap`].
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    pub(crate) fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Time<Physics>>();
        add_game(&mut app);
        // Startup + the initial OnEnter(InGame).
        app.update();
        app
    }

    /// Presses and releases `key`, then runs one more frame so a state
    /// change requested by that key press is applied.
    pub(crate) fn tap(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(key);
        input.clear();
        app.update();
    }

    pub(crate) fn count<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), F>()
            .iter(app.world())
            .count()
    }

    pub(crate) fn play_state(app: &App) -> Option<PlayState> {
        app.world()
            .get_resource::<State<PlayState>>()
            .map(|s| *s.get())
    }

    pub(crate) fn app_state(app: &App) -> AppState {
        *app.world().resource::<State<AppState>>().get()
    }

    pub(crate) fn physics_paused(app: &App) -> bool {
        app.world().resource::<Time<Physics>>().is_paused()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    fn text<M: Component>(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&Text2d, With<M>>()
            .single(app.world())
            .map(|t| t.0.clone())
            .unwrap_or_default()
    }

    fn move_ball_below_screen(app: &mut App) {
        let mut ball = app
            .world_mut()
            .query_filtered::<&mut Transform, With<Ball>>()
            .single_mut(app.world_mut())
            .expect("a run has exactly one ball");
        ball.translation.y = -WINDOW_HEIGHT;
    }

    #[test]
    fn first_launch_starts_a_playing_run() {
        let mut app = app();
        app.update();

        assert_eq!(app_state(&app), AppState::InGame);
        assert_eq!(play_state(&app), Some(PlayState::Playing));
        assert!(!physics_paused(&app));
        assert_eq!(count::<With<Ball>>(&mut app), 1);
        assert_eq!(count::<With<Paddle>>(&mut app), 1);
        assert_eq!(count::<With<Brick>>(&mut app), BRICK_ROWS * BRICK_COLS);
        assert_eq!(text::<LivesText>(&mut app), "Lives: 3");
        assert_eq!(text::<ScoreText>(&mut app), "Score: 0");
        assert_eq!(text::<OverlayText>(&mut app), "");
    }

    #[test]
    fn p_and_esc_toggle_pause_and_the_physics_clock() {
        let mut app = app();

        tap(&mut app, KeyCode::KeyP);
        assert_eq!(play_state(&app), Some(PlayState::Paused));
        assert!(physics_paused(&app));
        assert!(text::<OverlayText>(&mut app).starts_with("Paused"));

        tap(&mut app, KeyCode::Escape);
        assert_eq!(play_state(&app), Some(PlayState::Playing));
        assert!(!physics_paused(&app));
        assert_eq!(text::<OverlayText>(&mut app), "");

        tap(&mut app, KeyCode::Escape);
        assert_eq!(play_state(&app), Some(PlayState::Paused));
        tap(&mut app, KeyCode::KeyP);
        assert_eq!(play_state(&app), Some(PlayState::Playing));
    }

    #[test]
    fn losing_the_last_life_ends_the_run_and_r_starts_a_fresh_one() {
        let mut app = app();
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
        assert!(text::<OverlayText>(&mut app).starts_with("GAME OVER"));

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
        assert_eq!(text::<LivesText>(&mut app), "Lives: 3");
        assert_eq!(text::<OverlayText>(&mut app), "");
    }

    #[test]
    fn losing_a_life_that_is_not_the_last_keeps_playing() {
        let mut app = app();
        move_ball_below_screen(&mut app);
        app.update();
        app.update();

        assert_eq!(app_state(&app), AppState::InGame);
        assert_eq!(app.world().resource::<Lives>().0, STARTING_LIVES - 1);
        assert_eq!(text::<LivesText>(&mut app), "Lives: 2");
    }

    #[test]
    fn breaking_the_last_brick_wins() {
        let mut app = app();
        let bricks: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<Brick>>()
            .iter(app.world())
            .collect();
        // Leave one brick standing and report it broken, as if the ball's
        // collision had just despawned it this frame.
        for brick in &bricks[1..] {
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
        assert!(text::<OverlayText>(&mut app).starts_with("YOU WIN"));

        tap(&mut app, KeyCode::KeyR);
        assert_eq!(app_state(&app), AppState::InGame);
        assert_eq!(count::<With<Brick>>(&mut app), BRICK_ROWS * BRICK_COLS);
    }
}
