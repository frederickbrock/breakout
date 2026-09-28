mod game_state;
mod menu;
mod powerups;
mod script_manager;
mod spawner;
mod sprites;

use avian2d::prelude::*;
use bevy::asset::AssetMetaCheck;
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
/// Gap between the anchored ball and the paddle, so the launch doesn't start
/// in contact with the paddle (which would trigger the paddle-hit spin rule
/// and override the 45° serve).
const BALL_ANCHOR_GAP: f32 = 2.0;
/// Below this horizontal paddle speed the paddle counts as still, and the
/// serve goes right.
const PADDLE_STILL_SPEED: f32 = 1.0;
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
/// How much a multi-hit brick darkens (`Luminance::darker`) each time it
/// survives a hit.
const CRACKED_DARKEN: f32 = 0.3;
/// Lives at the start of every run, including the first.
const STARTING_LIVES: i32 = 3;

#[derive(Component)]
struct Paddle {
    width: f32,
}

#[derive(Component)]
struct Ball;

/// The ball is resting on the paddle waiting to be served (start of a run and
/// after every lost life). While anchored it's out of the simulation — see
/// [`anchored`] — and [`follow_paddle`] carries it along; Space or a left click
/// launches it ([`launch_ball`]).
#[derive(Component)]
struct Anchored;

type FlyingBall = (With<Ball>, Without<Anchored>);
type AnchoredBall = (With<Ball>, With<Anchored>);

#[derive(Component)]
struct Brick;

/// Hits a brick still takes before it breaks. Every brick spawns with 1;
/// other subsystems (power-up bricks) raise it at the start of a run.
#[derive(Component)]
struct BrickHealth(u8);

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct LivesText;

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

/// Fired by [`on_ball_collision`] when a brick takes its last hit, *before*
/// the brick is despawned, so observers can still read its other components.
/// `powerups` observes it to drop a power-up brick's power-up; this module
/// knows nothing about power-ups.
#[derive(Event)]
struct BrickDestroyed {
    brick: Entity,
    position: Vec2,
}

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
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: window_title(),
                    resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                    ..default()
                }),
                ..default()
            })
            // No `.meta` files ship with the assets; without this the browser
            // build requests one per asset and trunk's server answers 404.
            .set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..default()
            }),
    )
    .add_plugins(PhysicsPlugins::default())
    .add_plugins(script_manager::ScriptPlugin)
    .add_plugins(sprites::SpritesPlugin)
    .insert_resource(ClearColor(Color::BLACK));
    add_game(&mut app);
    app.run();
}

/// Env var `scripts/native-run.sh` sets so each agent's window has a unique,
/// targetable title. Native only; the browser has no window title to set.
#[cfg(not(target_arch = "wasm32"))]
const WINDOW_TITLE_ENV: &str = "BREAKOUT_WINDOW_TITLE";

fn window_title() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    let custom = std::env::var(WINDOW_TITLE_ENV).ok();
    #[cfg(target_arch = "wasm32")]
    let custom = None;
    title_or_default(custom)
}

/// The window title: `custom` if set and non-blank, else "Breakout".
fn title_or_default(custom: Option<String>) -> String {
    custom
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "Breakout".to_string())
}

/// Everything game-specific, on top of the engine plugins (`DefaultPlugins`,
/// Avian, scripting) that `main` adds. Split out so tests can run the real
/// game logic on a headless `MinimalPlugins` app.
fn add_game(app: &mut App) {
    app.add_plugins((GameStatePlugin, menu::MenuPlugin))
        .insert_resource(Gravity(Vec2::new(0.0, 0.8)))
        .init_resource::<ButtonInput<MouseButton>>()
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
                (
                    paddle_movement.in_set(PaddleMovementSet),
                    ball_movement,
                    follow_paddle,
                    launch_ball,
                )
                    .chain()
                    .run_if(in_state(PlayState::Playing)),
                restart_from_game_over.run_if(in_state(AppState::GameOver)),
                update_hud.run_if(in_state(AppState::InGame)),
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
    let paddle_start = Vec3::new(
        0.0,
        -WINDOW_HEIGHT / 2.0 + PADDLE_HEIGHT / 2.0 + PADDLE_MARGIN_BOTTOM,
        0.0,
    );
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Sprite::from_color(RED, Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
        Transform::from_translation(paddle_start),
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
        Transform::from_translation(anchor_position(paddle_start)),
        RigidBody::Dynamic,
        Collider::circle(BALL_SIZE / 2.0),
        LinearVelocity::ZERO,
        anchored(),
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
                BrickHealth(1),
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
    mut brick_query: Query<(&Transform, &mut BrickHealth, &mut Sprite), With<Brick>>,
    paddle_query: Query<&Transform, With<Paddle>>,
) {
    let other = on.collider2;
    if let Ok((transform, mut health, mut sprite)) = brick_query.get_mut(other) {
        // Already broken by an earlier contact; its despawn is still queued.
        if health.0 == 0 {
            return;
        }
        score.0 += 10;
        health.0 -= 1;
        if health.0 == 0 {
            // Trigger before the despawn so observers can still read the brick.
            commands.trigger(BrickDestroyed {
                brick: other,
                position: transform.translation.truncate(),
            });
            commands.entity(other).despawn();
            signals.broke_brick = true;
        } else {
            sprite.color = sprite.color.darker(CRACKED_DARKEN);
        }
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
    mut ball_query: Query<(Entity, &mut Transform, &mut LinearVelocity), FlyingBall>,
) {
    let broke_brick = signals.broke_brick;
    let paddle_hit_x = signals.paddle_hit_x;
    *signals = BallCollisionSignals::default();

    // An anchored ball isn't moving, can't have hit anything and can't fall.
    let Ok((ball, mut ball_transform, mut ball_velocity)) = ball_query.single_mut() else {
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

    // `on_ball_collision`'s despawn is already applied by now (Avian
    // triggers collisions from an exclusive system in FixedPostUpdate, whose
    // commands flush before Update), so the run is won only once no brick is
    // left at all, cracked multi-hit bricks included.
    if broke_brick && brick_query.is_empty() {
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
            // Back on the paddle for the next serve.
            ball_velocity.0 = Vec2::ZERO;
            if let Ok((paddle_transform, _)) = paddle_query.single() {
                ball_transform.translation = anchor_position(paddle_transform.translation);
            }
            commands.entity(ball).insert(anchored());
        }
    }
}

/// Components that take the ball out of the simulation while it waits on the
/// paddle: no velocity integration, no contact response and no collision
/// events. Removed together by [`launch_ball`].
fn anchored() -> (Anchored, RigidBodyDisabled, ColliderDisabled) {
    (Anchored, RigidBodyDisabled, ColliderDisabled)
}

/// Where an anchored ball sits: centred on top of a paddle at `paddle`.
fn anchor_position(paddle: Vec3) -> Vec3 {
    Vec3::new(
        paddle.x,
        paddle.y + PADDLE_HEIGHT / 2.0 + BALL_ANCHOR_GAP + BALL_SIZE / 2.0,
        0.0,
    )
}

/// Keeps an anchored ball on top of the paddle as it moves. Centred, so a
/// paddle-width change (Super-Sizer) doesn't move it.
fn follow_paddle(
    paddle: Query<&Transform, (With<Paddle>, Without<Ball>)>,
    mut ball: Query<&mut Transform, AnchoredBall>,
) {
    let (Ok(paddle), Ok(mut ball)) = (paddle.single(), ball.single_mut()) else {
        return;
    };
    let target = anchor_position(paddle.translation);
    if ball.translation != target {
        ball.translation = target;
    }
}

/// Space or a left click serves an anchored ball: upward at [`BALL_SPEED`],
/// 45° toward the side the paddle is moving (right if it's still). Does
/// nothing once the ball is in flight; gated to `Playing` like all gameplay
/// input, so it's ignored while paused.
fn launch_ball(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    paddle: Query<&LinearVelocity, (With<Paddle>, Without<Ball>)>,
    mut ball: Query<(Entity, &mut LinearVelocity), AnchoredBall>,
) {
    if !keyboard.just_pressed(KeyCode::Space) && !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok((entity, mut velocity)) = ball.single_mut() else {
        return;
    };
    let paddle_vx = paddle.single().map_or(0.0, |v| v.0.x);
    let side = if paddle_vx < -PADDLE_STILL_SPEED {
        -1.0
    } else {
        1.0
    };
    velocity.0 = Vec2::new(side, 1.0).normalize() * BALL_SPEED;
    commands
        .entity(entity)
        .remove::<(Anchored, RigidBodyDisabled, ColliderDisabled)>();
}

fn end_run(commands: &mut Commands, next_state: &mut NextState<AppState>, outcome: GameOutcome) {
    commands.insert_resource(outcome);
    next_state.set(AppState::GameOver);
}

/// R on the game-over/win screen starts a new run (a shortcut for its Play
/// again button); entering
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

/// Headless app running the real game logic (no window, renderer or
/// scripting), with a fixed 100 ms step per `update()` and keyboard input
/// driven by hand via [`tap`].
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// A headless app as just launched: sitting on the main menu.
    pub(crate) fn launch() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Time<Physics>>();
        add_game(&mut app);
        // Startup + the initial OnEnter(MainMenu).
        app.update();
        app
    }

    /// A headless app that has left the main menu and is in a fresh run.
    pub(crate) fn app() -> App {
        let mut app = launch();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
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

    /// Left-clicks (press + release), then runs one more frame, like [`tap`].
    pub(crate) fn click(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let mut input = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        input.release(MouseButton::Left);
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

    /// Every brick entity currently in the world.
    pub(crate) fn bricks(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<Brick>>()
            .iter(app.world())
            .collect()
    }

    /// Fakes one ball contact with `brick` the way Avian does it
    /// (`trigger_collision_events` triggers `CollisionStart` on the world),
    /// then applies the observer's commands.
    pub(crate) fn hit(app: &mut App, brick: Entity) {
        let ball = app
            .world_mut()
            .query_filtered::<Entity, With<Ball>>()
            .single(app.world())
            .expect("a run has exactly one ball");
        app.world_mut().trigger(CollisionStart {
            collider1: ball,
            collider2: brick,
            body1: Some(ball),
            body2: Some(brick),
        });
        app.world_mut().flush();
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
    fn leaving_the_menu_for_a_run_starts_playing() {
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
        assert_eq!(text::<LivesText>(&mut app), "Lives: 3");
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
        assert_eq!(text::<LivesText>(&mut app), "Lives: 2");
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

    fn ball(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<Ball>>()
            .single(app.world())
            .expect("a run has exactly one ball")
    }

    fn paddle(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<Paddle>>()
            .single(app.world())
            .expect("a run has exactly one paddle")
    }

    fn is_anchored(app: &mut App) -> bool {
        let ball = ball(app);
        let entity = app.world().entity(ball);
        let anchored = entity.contains::<Anchored>();
        // The marker and the physics opt-outs always travel together.
        assert_eq!(entity.contains::<RigidBodyDisabled>(), anchored);
        assert_eq!(entity.contains::<ColliderDisabled>(), anchored);
        anchored
    }

    fn ball_velocity(app: &mut App) -> Vec2 {
        let ball = ball(app);
        app.world().get::<LinearVelocity>(ball).unwrap().0
    }

    fn translation(app: &App, entity: Entity) -> Vec3 {
        app.world().get::<Transform>(entity).unwrap().translation
    }

    fn set_paddle_x(app: &mut App, x: f32) {
        let paddle = paddle(app);
        app.world_mut()
            .get_mut::<Transform>(paddle)
            .unwrap()
            .translation
            .x = x;
    }

    fn set_paddle_vx(app: &mut App, vx: f32) {
        let paddle = paddle(app);
        app.world_mut()
            .entity_mut(paddle)
            .insert(LinearVelocity(Vec2::new(vx, 0.0)));
    }

    fn assert_resting_on_paddle(app: &mut App) {
        let (ball, paddle) = (ball(app), paddle(app));
        let (b, p) = (translation(app, ball), translation(app, paddle));
        assert_eq!(b.x, p.x);
        assert_eq!(
            b.y,
            p.y + PADDLE_HEIGHT / 2.0 + BALL_ANCHOR_GAP + BALL_SIZE / 2.0
        );
        assert_eq!(ball_velocity(app), Vec2::ZERO);
    }

    #[test]
    fn a_new_run_starts_with_the_ball_anchored_on_the_paddle() {
        let mut app = app();
        app.update();

        assert!(is_anchored(&mut app));
        assert_resting_on_paddle(&mut app);
    }

    #[test]
    fn the_anchored_ball_follows_the_paddle() {
        let mut app = app();
        set_paddle_x(&mut app, -150.0);
        app.update();
        assert_resting_on_paddle(&mut app);
        let ball = ball(&mut app);
        assert_eq!(translation(&app, ball).x, -150.0);

        // A wider paddle (Super-Sizer) keeps the ball centred.
        let paddle = paddle(&mut app);
        app.world_mut().get_mut::<Paddle>(paddle).unwrap().width = PADDLE_WIDTH * 1.5;
        set_paddle_x(&mut app, 90.0);
        app.update();
        assert_resting_on_paddle(&mut app);
    }

    #[test]
    fn space_launches_the_ball_up_and_right_from_a_still_paddle() {
        let mut app = app();
        tap(&mut app, KeyCode::Space);

        assert!(!is_anchored(&mut app));
        let v = ball_velocity(&mut app);
        assert!((v.length() - BALL_SPEED).abs() < 1e-3);
        assert!(v.y > 0.0);
        assert!((v.x - v.y).abs() < 1e-3, "45° to the right, got {v:?}");
    }

    #[test]
    fn left_click_launches_toward_the_way_the_paddle_is_moving() {
        let mut app = app();
        set_paddle_vx(&mut app, -200.0);
        click(&mut app);

        assert!(!is_anchored(&mut app));
        let v = ball_velocity(&mut app);
        assert!(v.y > 0.0);
        assert!((v.x + v.y).abs() < 1e-3, "45° to the left, got {v:?}");
    }

    #[test]
    fn launch_input_does_nothing_while_the_ball_is_in_flight() {
        let mut app = app();
        tap(&mut app, KeyCode::Space);
        let in_flight = Vec2::new(-120.0, 250.0);
        let ball = ball(&mut app);
        app.world_mut().get_mut::<LinearVelocity>(ball).unwrap().0 = in_flight;

        set_paddle_vx(&mut app, 300.0);
        tap(&mut app, KeyCode::Space);
        click(&mut app);

        assert!(!is_anchored(&mut app));
        let v = ball_velocity(&mut app);
        assert!(
            (v.normalize() - in_flight.normalize()).length() < 1e-3,
            "direction unchanged, got {v:?}"
        );
    }

    #[test]
    fn launch_is_ignored_while_paused_and_the_ball_stays_anchored() {
        let mut app = app();
        tap(&mut app, KeyCode::KeyP);
        assert_eq!(play_state(&app), Some(PlayState::Paused));

        // A click that isn't on a pause-menu button does nothing.
        click(&mut app);
        assert_eq!(play_state(&app), Some(PlayState::Paused));
        assert!(is_anchored(&mut app));

        // Space on the pause menu activates the focused Resume button: the
        // game resumes, but that same press doesn't also serve the ball.
        tap(&mut app, KeyCode::Space);
        assert_eq!(play_state(&app), Some(PlayState::Playing));
        assert!(is_anchored(&mut app));
        assert_resting_on_paddle(&mut app);

        // The next press serves.
        tap(&mut app, KeyCode::Space);
        assert!(!is_anchored(&mut app));
    }

    #[test]
    fn losing_a_life_re_anchors_the_ball_on_the_paddle() {
        let mut app = app();
        tap(&mut app, KeyCode::Space);
        set_paddle_x(&mut app, 200.0);
        move_ball_below_screen(&mut app);
        app.update();
        app.update();

        assert_eq!(app.world().resource::<Lives>().0, STARTING_LIVES - 1);
        assert!(is_anchored(&mut app));
        assert_resting_on_paddle(&mut app);
        let ball = ball(&mut app);
        assert_eq!(translation(&app, ball).x, 200.0);

        // Served again from there.
        tap(&mut app, KeyCode::Space);
        assert!(!is_anchored(&mut app));
    }

    fn score(app: &App) -> i32 {
        app.world().resource::<Score>().0
    }

    #[test]
    fn a_normal_brick_breaks_in_one_hit() {
        let mut app = app();
        let brick = bricks(&mut app)[0];
        app.world_mut().get_mut::<BrickHealth>(brick).unwrap().0 = 1;

        hit(&mut app, brick);
        assert!(app.world().get_entity(brick).is_err());
        assert_eq!(score(&app), 10);
        assert_eq!(bricks(&mut app).len(), BRICK_ROWS * BRICK_COLS - 1);
    }

    #[test]
    fn a_two_hit_brick_cracks_then_breaks() {
        let mut app = app();
        let brick = bricks(&mut app)[0];
        app.world_mut().get_mut::<BrickHealth>(brick).unwrap().0 = 2;
        let intact = app.world().get::<Sprite>(brick).unwrap().color;

        hit(&mut app, brick);
        assert_eq!(app.world().get::<BrickHealth>(brick).unwrap().0, 1);
        assert_ne!(app.world().get::<Sprite>(brick).unwrap().color, intact);
        assert_eq!(score(&app), 10);
        assert!(!app.world().resource::<BallCollisionSignals>().broke_brick);

        // One contact is one hit: nothing more happens on later frames.
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().get::<BrickHealth>(brick).unwrap().0, 1);

        hit(&mut app, brick);
        assert!(app.world().get_entity(brick).is_err());
        assert_eq!(score(&app), 20);
    }

    #[test]
    fn clearing_every_brick_including_two_hit_ones_wins() {
        let mut app = app();
        tap(&mut app, KeyCode::Space);
        let all = bricks(&mut app);
        for brick in &all[2..] {
            app.world_mut().despawn(*brick);
        }
        let (a, b) = (all[0], all[1]);
        app.world_mut().get_mut::<BrickHealth>(a).unwrap().0 = 1;
        app.world_mut().get_mut::<BrickHealth>(b).unwrap().0 = 2;

        hit(&mut app, a);
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::InGame, "one brick is left");

        hit(&mut app, b);
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::InGame, "a cracked brick is left");

        hit(&mut app, b);
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::GameOver);
        assert_eq!(
            app.world().get_resource::<GameOutcome>(),
            Some(&GameOutcome::Won)
        );
    }

    #[test]
    fn the_window_title_defaults_to_breakout() {
        assert_eq!(title_or_default(None), "Breakout");
        assert_eq!(title_or_default(Some("  ".into())), "Breakout");
        assert_eq!(
            title_or_default(Some("Breakout [tester@tester sim-rdl.2]".into())),
            "Breakout [tester@tester sim-rdl.2]"
        );
    }
}
