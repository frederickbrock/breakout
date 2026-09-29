mod bricks;
mod controls;
mod game_state;
mod menu;
mod particles;
mod powerups;
mod script_manager;
mod spawner;
mod sprites;
mod theme;

use avian2d::prelude::*;
use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bricks::{BrickCell, BrickClass};
use controls::{ControlSettings, PaddleControl, PaddleTarget};
use game_state::{AppState, GameOutcome, GameStatePlugin, PlayState};

// Game constants
/// Every gameplay size and speed is its old 900x650-window design value times
/// this, so the game looks and plays the same in the bigger world.
const GAME_SCALE: f32 = 1.5;
/// The logical world the camera always shows in full (letterboxed to fit the
/// window, see `view`).
const WORLD_WIDTH: f32 = 1920.0;
const WORLD_HEIGHT: f32 = 1080.0;
/// The centred playfield well: the walls sit on its left, right and top
/// edges, and the ball is lost below its bottom edge.
const PLAYFIELD_WIDTH: f32 = 1440.0;
const PLAYFIELD_HEIGHT: f32 = WORLD_HEIGHT;
/// The panel either side of the well (240), home of the HUD.
#[cfg(test)]
const SIDE_PANEL_WIDTH: f32 = (WORLD_WIDTH - PLAYFIELD_WIDTH) / 2.0;
/// The native window's opening size; it is resizable and the world scales.
const WINDOW_START_WIDTH: u32 = 1280;
const WINDOW_START_HEIGHT: u32 = 720;
const WALL_THICKNESS: f32 = 40.0 * GAME_SCALE;
const PADDLE_WIDTH: f32 = 120.0 * GAME_SCALE;
const PADDLE_HEIGHT: f32 = 20.0 * GAME_SCALE;
const PADDLE_MASS: f32 = 3.0;
const PADDLE_FORCE: f32 = 7000.0 * GAME_SCALE;
const PADDLE_LINEAR_DAMPING: f32 = 4.0;
const PADDLE_MARGIN_BOTTOM: f32 = 10.0 * GAME_SCALE;
/// Width of each paddle end prong (the sprite is 54 px at 2x).
const PRONG_WIDTH: f32 = 27.0 * GAME_SCALE;
const BALL_SIZE: f32 = 15.0 * GAME_SCALE;
const BALL_SPEED: f32 = 300.0 * GAME_SCALE;
/// Gap between the anchored ball and the paddle, so the launch doesn't start
/// in contact with the paddle (which would trigger the paddle-hit spin rule
/// and override the 45° serve).
const BALL_ANCHOR_GAP: f32 = 2.0 * GAME_SCALE;
/// Below this horizontal paddle speed the paddle counts as still, and the
/// serve goes right.
const PADDLE_STILL_SPEED: f32 = 1.0 * GAME_SCALE;
// Guards against a real failure mode observed in testing: a wall bounce only
// inverts the velocity component perpendicular to the wall, so a ball that
// ends up moving near-perfectly horizontally between the side walls (below
// the bricks, above the paddle) can get permanently stuck bouncing
// side-to-side forever, since nothing left in that lane can ever touch its Y
// velocity again. Keeping a minimum vertical fraction guarantees the ball
// always keeps drifting toward the bricks or the paddle.
const BALL_MIN_VERTICAL_FRACTION: f32 = 0.3;
/// Minimum clear gap between the outermost brick and each wall, in ball
/// widths. Raising it narrows the (derived) bricks; nothing else changes.
const SIDE_CHANNEL_BALLS: f32 = 3.0;
const SIDE_CHANNEL: f32 = SIDE_CHANNEL_BALLS * BALL_SIZE;
/// Gap between neighbouring bricks, both ways.
const BRICK_GAP: f32 = 5.0 * GAME_SCALE;
/// Distance from the top wall to the top of the first brick row.
const BRICK_TOP_MARGIN: f32 = 50.0 * GAME_SCALE;
const BRICK_HEIGHT: f32 = 30.0 * GAME_SCALE;
/// Derived so a full row fills the well less a [`SIDE_CHANNEL`] each side.
const BRICK_WIDTH: f32 =
    (PLAYFIELD_WIDTH - 2.0 * SIDE_CHANNEL - (bricks::BOARD_COLS as f32 - 1.0) * BRICK_GAP)
        / bricks::BOARD_COLS as f32;
// Board size, for tests across modules (the game itself uses `bricks::BOARD_*`).
#[cfg(test)]
const BRICK_ROWS: usize = bricks::BOARD_ROWS;
#[cfg(test)]
const BRICK_COLS: usize = bricks::BOARD_COLS;
/// Lives at the start of every run, including the first.
const STARTING_LIVES: i32 = 3;

#[derive(Component)]
struct Paddle {
    width: f32,
}

#[derive(Component)]
struct Ball;

/// The ball's velocity at the start of the current physics step, recorded
/// by [`record_ball_approach`]. Avian triggers `CollisionStart` after its
/// solver, when `LinearVelocity` has usually already been reflected, so
/// direction-dependent rules (shield glass) read this instead.
#[derive(Component, Default)]
struct BallApproach(Vec2);

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

/// Hits a brick still takes before it breaks; it spawns at its class's
/// `max_hits()`.
#[derive(Component)]
struct BrickHealth(u8);

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct LivesText;

/// The paddle's end prongs, kept at its ends by [`place_paddle_pieces`] as its
/// width changes (Super-Sizer). `side` is -1 (left) or 1 (right).
#[derive(Component)]
struct PaddleProng {
    side: f32,
}

/// The paddle's glowing field between the prongs, stretched by
/// [`place_paddle_pieces`] to fill the gap.
#[derive(Component)]
struct PaddleField;

/// Where the paddle's visual pieces go for a paddle `width` wide: each prong
/// is centred `prong_offset` either side of the middle, and the field fills
/// the `field_width` between them.
#[derive(Debug, PartialEq)]
struct PaddlePieces {
    prong_offset: f32,
    field_width: f32,
}

fn paddle_pieces(width: f32) -> PaddlePieces {
    PaddlePieces {
        prong_offset: (width - PRONG_WIDTH) / 2.0,
        field_width: (width - 2.0 * PRONG_WIDTH).max(0.0),
    }
}

/// Mesh and material for the round ball, made once at startup.
#[derive(Resource)]
struct BallLook {
    mesh: Handle<Mesh>,
    material: Handle<ColorMaterial>,
}

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
    class: BrickClass,
    /// Destroyed by an explosive's blast rather than the ball. The blast's
    /// whole chain is resolved at once, so this doesn't set off another one.
    by_blast: bool,
}

/// Fired when a brick takes damage but survives (it has hits left).
/// Brick behaviours that react to being hurt (regen's heal timer) observe
/// this instead of being special-cased in [`on_ball_collision`]. Explosions
/// (sim-rdl.7.3) fire it too.
#[derive(Event)]
struct BrickDamaged {
    brick: Entity,
    /// Where the hit landed: the ball's position for a ball hit, the brick's
    /// centre for blast damage.
    position: Vec2,
    class: BrickClass,
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
                    resolution: (WINDOW_START_WIDTH, WINDOW_START_HEIGHT).into(),
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
    .add_plugins(particles::ParticlesPlugin)
    .insert_resource(ClearColor(theme::VOID));
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
    app.add_plugins((
        GameStatePlugin,
        menu::MenuPlugin,
        controls::ControlsPlugin,
        sprites::SkinPlugin,
        bricks::BricksPlugin,
        particles::VfxPlugin,
    ))
    .insert_resource(Gravity(Vec2::new(0.0, 0.8 * GAME_SCALE)))
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<Score>()
    .insert_resource(Lives(STARTING_LIVES))
    .init_resource::<BallCollisionSignals>()
    .add_observer(on_ball_collision)
    .add_plugins(powerups::PowerUpsPlugin)
    .add_systems(Startup, setup_level)
    .add_systems(OnEnter(AppState::InGame), start_run)
    .add_systems(
        FixedPostUpdate,
        record_ball_approach.in_set(PhysicsSystems::First),
    )
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
            place_paddle_pieces,
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
    ball_look: Res<BallLook>,
) {
    score.0 = 0;
    lives.0 = STARTING_LIVES;
    *signals = BallCollisionSignals::default();
    spawn_run_entities(&mut commands, &ball_look);
    commands.trigger(RestartGame);
}

fn spawn_run_entities(commands: &mut Commands, ball_look: &BallLook) {
    let paddle_start = Vec3::new(
        0.0,
        -PLAYFIELD_HEIGHT / 2.0 + PADDLE_HEIGHT / 2.0 + PADDLE_MARGIN_BOTTOM,
        0.0,
    );
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        // No sprite of its own: it's drawn by its three children.
        Visibility::default(),
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
        children![prong(-1.0), paddle_field(), prong(1.0)],
    ));

    commands.spawn((
        Mesh2d(ball_look.mesh.clone()),
        MeshMaterial2d(ball_look.material.clone()),
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
        BallApproach::default(),
        DespawnOnExit(AppState::InGame),
    ));

    spawn_bricks(commands);

    spawn_hud_line(
        commands,
        "SCORE ",
        "0",
        PLAYFIELD_HEIGHT / 2.0 - 10.0,
        ScoreText,
    );
    spawn_hud_line(
        commands,
        "LIVES ",
        &STARTING_LIVES.to_string(),
        PLAYFIELD_HEIGHT / 2.0 - 40.0,
        LivesText,
    );
}

fn setup_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);
    commands.insert_resource(BallLook {
        mesh: meshes.add(Circle::new(BALL_SIZE / 2.0)),
        material: materials.add(theme::STEEL),
    });

    // Static walls the ball (and paddle) physically bounce off, instead of
    // manual clamp/reflect code. No bottom wall — a ball reaching the bottom
    // is a life lost, checked separately from physics.
    for (centre, size) in wall_specs() {
        commands.spawn((
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
            Transform::from_translation(centre.extend(0.0)),
        ));
    }
}

/// The left, right and top walls as (centre, size), with their inner faces
/// exactly on the playfield well's edges.
fn wall_specs() -> [(Vec2, Vec2); 3] {
    let half_w = PLAYFIELD_WIDTH / 2.0;
    let half_h = PLAYFIELD_HEIGHT / 2.0;
    let t = WALL_THICKNESS;
    let side = Vec2::new(t, PLAYFIELD_HEIGHT + 2.0 * t);
    [
        (Vec2::new(-half_w - t / 2.0, 0.0), side),
        (Vec2::new(half_w + t / 2.0, 0.0), side),
        (
            Vec2::new(0.0, half_h + t / 2.0),
            Vec2::new(PLAYFIELD_WIDTH + 2.0 * t, t),
        ),
    ]
}

/// A HUD line: an uppercase label in the label colour followed by a value
/// span in ink. `marker` goes on the value span, which [`update_hud`] writes.
fn spawn_hud_line(
    commands: &mut Commands,
    label: &str,
    value: &str,
    y: f32,
    marker: impl Component,
) {
    let font = TextFont {
        font_size: FontSize::Px(24.0),
        ..default()
    };
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Text2d::new(label),
        font.clone(),
        TextColor(theme::LABEL),
        Anchor::TOP_LEFT,
        Transform::from_xyz(-WORLD_WIDTH / 2.0 + 20.0, y, 1.0),
        children![(TextSpan::new(value), font, TextColor(theme::INK), marker)],
    ));
}

/// One of the paddle's end prongs; positioned by [`place_paddle_pieces`].
fn prong(side: f32) -> impl Bundle {
    let pieces = paddle_pieces(PADDLE_WIDTH);
    (
        PaddleProng { side },
        Sprite::from_color(theme::EMITTER_PRONG, Vec2::new(PRONG_WIDTH, PADDLE_HEIGHT)),
        Transform::from_xyz(side * pieces.prong_offset, 0.0, 0.1),
    )
}

/// The paddle's middle field; sized by [`place_paddle_pieces`].
fn paddle_field() -> impl Bundle {
    let pieces = paddle_pieces(PADDLE_WIDTH);
    (
        PaddleField,
        Sprite::from_color(theme::EMITTER, Vec2::new(pieces.field_width, PADDLE_HEIGHT)),
        Transform::default(),
    )
}

/// Keeps the prongs at the paddle's ends when its width changes.
fn place_paddle_pieces(
    paddles: Query<(&Paddle, &Children), Changed<Paddle>>,
    mut prongs: Query<(&PaddleProng, &mut Transform)>,
    mut fields: Query<&mut Sprite, With<PaddleField>>,
) {
    for (paddle, children) in &paddles {
        let pieces = paddle_pieces(paddle.width);
        for child in children.iter() {
            if let Ok((prong, mut transform)) = prongs.get_mut(child) {
                transform.translation.x = prong.side * pieces.prong_offset;
            }
            if let Ok(mut sprite) = fields.get_mut(child) {
                sprite.custom_size = Some(Vec2::new(pieces.field_width, PADDLE_HEIGHT));
            }
        }
    }
}

/// A fresh random board of brick classes (see [`bricks::generate_board`]),
/// each brick at its class's colour and hit count.
fn spawn_bricks(commands: &mut Commands) {
    let board = bricks::generate_board(&mut rand::rng());
    for (row, classes) in board.iter().enumerate() {
        for (col, &class) in classes.iter().enumerate() {
            let cell = BrickCell { row, col };
            commands.spawn((
                Sprite::from_color(
                    theme::brick_color(class),
                    Vec2::new(BRICK_WIDTH, BRICK_HEIGHT),
                ),
                Transform::from_translation(brick_translation(cell)),
                RigidBody::Static,
                Collider::rectangle(BRICK_WIDTH, BRICK_HEIGHT),
                Brick,
                class,
                cell,
                BrickHealth(class.max_hits()),
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

/// Where the brick in `cell` sits (see [`brick_x`] and [`brick_y`]).
fn brick_translation(cell: BrickCell) -> Vec3 {
    Vec3::new(
        brick_x(cell.col, bricks::BOARD_COLS),
        brick_y(cell.row),
        0.0,
    )
}

/// Centre x of column `col` in a row of `cols` bricks, centred in the well so
/// the left and right channels are equal (fewer columns, wider channels).
fn brick_x(col: usize, cols: usize) -> f32 {
    let grid_width = cols as f32 * BRICK_WIDTH + (cols as f32 - 1.0) * BRICK_GAP;
    -grid_width / 2.0 + BRICK_WIDTH / 2.0 + col as f32 * (BRICK_WIDTH + BRICK_GAP)
}

/// Centre y of row `row` (0 at the top), [`BRICK_TOP_MARGIN`] below the top
/// wall.
fn brick_y(row: usize) -> f32 {
    PLAYFIELD_HEIGHT / 2.0
        - BRICK_TOP_MARGIN
        - BRICK_HEIGHT / 2.0
        - row as f32 * (BRICK_HEIGHT + BRICK_GAP)
}

/// Copies the ball's velocity into [`BallApproach`] at the start of every
/// physics step, before the solver bounces it.
fn record_ball_approach(mut balls: Query<(&LinearVelocity, &mut BallApproach), With<Ball>>) {
    for (velocity, mut approach) in &mut balls {
        approach.0 = velocity.0;
    }
}

/// Arrow keys / A/D push the paddle with a force in both control modes. In
/// Mouse mode, with no movement key held, the paddle's velocity is driven
/// toward the cursor target instead (see `controls`); a held key takes over
/// and clears that target.
fn paddle_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<ControlSettings>,
    mut target: ResMut<PaddleTarget>,
    mut paddle_query: Query<(&Transform, &Paddle, &mut ConstantForce, &mut LinearVelocity)>,
) {
    let Ok((transform, paddle, mut force, mut velocity)) = paddle_query.single_mut() else {
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

    if fx != 0.0 {
        target.x = None;
        return;
    }
    if settings.paddle == PaddleControl::Mouse {
        if let Some(target_x) = target.x {
            let target_x = controls::clamp_paddle_x(target_x, paddle.width);
            velocity.0.x =
                controls::follow_velocity(transform.translation.x, target_x, time.delta_secs());
        }
    }
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
///
/// Each hit on a brick scores 10 and removes one hit point; the last one
/// despawns it (after triggering [`BrickDestroyed`]). Shield glass is the
/// exception: it only takes damage from a ball that was moving downward, read
/// from [`BallApproach`]; any other contact just flashes it.
fn on_ball_collision(
    on: On<CollisionStart>,
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut signals: ResMut<BallCollisionSignals>,
    mut brick_query: Query<(&Transform, &BrickClass, &mut BrickHealth, &mut Sprite), With<Brick>>,
    paddle_query: Query<&Transform, With<Paddle>>,
    ball_query: Query<(&BallApproach, &Transform), With<Ball>>,
) {
    let other = on.collider2;
    if let Ok((transform, &class, mut health, mut sprite)) = brick_query.get_mut(other) {
        // Already broken by an earlier contact; its despawn is still queued.
        if health.0 == 0 {
            return;
        }
        // Shield glass only breaks from above: a ball moving downward at
        // contact. Anything else bounces (Avian already did) and flashes.
        if class == BrickClass::Shield {
            let from_above = ball_query
                .get(on.collider1)
                .is_ok_and(|(approach, _)| approach.0.y < 0.0);
            if !from_above {
                sprite.color = theme::SHIELD_FLASH;
                commands
                    .entity(other)
                    .insert(bricks::ShieldFlash::default());
                return;
            }
        }
        score.0 += 10;
        health.0 -= 1;
        if health.0 == 0 {
            // Trigger before the despawn so observers can still read the brick.
            commands.trigger(BrickDestroyed {
                brick: other,
                position: transform.translation.truncate(),
                class,
                by_blast: false,
            });
            commands.entity(other).despawn();
            signals.broke_brick = true;
        } else {
            let contact = ball_query
                .get(on.collider1)
                .map_or(transform.translation, |(_, ball)| ball.translation);
            commands.trigger(BrickDamaged {
                brick: other,
                position: contact.truncate(),
                class,
            });
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
    // left at all, damaged multi-hit bricks included.
    if broke_brick && brick_query.is_empty() {
        end_run(&mut commands, &mut next_state, GameOutcome::Won);
        return;
    }

    // Ball fell off the bottom (no physical wall there, so this stays a
    // plain position check rather than a collision).
    if ball_transform.translation.y < -PLAYFIELD_HEIGHT / 2.0 {
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
    mut score_text: Query<&mut TextSpan, (With<ScoreText>, Without<LivesText>)>,
    mut lives_text: Query<&mut TextSpan, (With<LivesText>, Without<ScoreText>)>,
) {
    if let Ok(mut text) = score_text.single_mut() {
        text.0 = score.0.to_string();
    }
    if let Ok(mut text) = lives_text.single_mut() {
        text.0 = lives.0.to_string();
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
            .init_resource::<Time<Physics>>()
            // The ball's mesh and material are made at startup.
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<ColorMaterial>();
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

    /// Like [`hit`], with the ball moving at `velocity` when it makes contact.
    pub(crate) fn hit_moving(app: &mut App, brick: Entity, velocity: Vec2) {
        let ball = app
            .world_mut()
            .query_filtered::<Entity, With<Ball>>()
            .single(app.world())
            .expect("a run has exactly one ball");
        app.world_mut()
            .entity_mut(ball)
            .insert((LinearVelocity(velocity), BallApproach(velocity)));
        hit(app, brick);
    }

    /// The first brick of `class` (every board has at least one of each).
    pub(crate) fn brick_of(app: &mut App, class: BrickClass) -> Entity {
        app.world_mut()
            .query_filtered::<(Entity, &BrickClass), With<Brick>>()
            .iter(app.world())
            .find(|(_, c)| **c == class)
            .map(|(e, _)| e)
            .unwrap_or_else(|| panic!("the board has no {class:?} brick"))
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    fn text<M: Component>(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&TextSpan, With<M>>()
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
        ball.translation.y = -PLAYFIELD_HEIGHT;
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

    fn paddle_state(app: &mut App) -> (f32, Vec2, Vec2) {
        let paddle = paddle(app);
        let entity = app.world().entity(paddle);
        (
            entity.get::<Transform>().unwrap().translation.x,
            entity.get::<LinearVelocity>().unwrap().0,
            entity.get::<ConstantForce>().unwrap().0,
        )
    }

    fn aim_mouse_at(app: &mut App, x: f32) {
        app.world_mut().resource_mut::<PaddleTarget>().x = Some(x);
    }

    fn hold(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
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

    fn score(app: &App) -> i32 {
        app.world().resource::<Score>().0
    }

    fn classes() -> [BrickClass; 9] {
        use bricks::ExplosiveKind::*;
        [
            BrickClass::Ceramic,
            BrickClass::Titanium,
            BrickClass::Tungsten,
            BrickClass::Reactor,
            BrickClass::Regen,
            BrickClass::Shield,
            BrickClass::Explosive(Charge),
            BrickClass::Explosive(Breach),
            BrickClass::Explosive(Demolition),
        ]
    }

    /// A ball moving down onto a brick (the only way shield glass breaks).
    const FROM_ABOVE: Vec2 = Vec2::new(60.0, -BALL_SPEED);

    #[test]
    fn each_class_breaks_after_its_hit_count() {
        for class in classes() {
            let mut app = app();
            let brick = brick_of(&mut app, class);
            let max = class.max_hits();
            for i in 1..max {
                hit_moving(&mut app, brick, FROM_ABOVE);
                assert_eq!(
                    app.world().get::<BrickHealth>(brick).unwrap().0,
                    max - i,
                    "{class:?}"
                );
                assert_eq!(
                    app.world().get::<Sprite>(brick).unwrap().color,
                    theme::brick_color(class),
                    "{class:?} keeps its colour (damage shows as particles)"
                );
                assert_eq!(score(&app), 10 * i as i32);
                assert!(!app.world().resource::<BallCollisionSignals>().broke_brick);
                // One contact is one hit: nothing more happens on later frames.
                for _ in 0..3 {
                    app.update();
                }
                assert_eq!(app.world().get::<BrickHealth>(brick).unwrap().0, max - i);
            }
            hit_moving(&mut app, brick, FROM_ABOVE);
            assert!(app.world().get_entity(brick).is_err(), "{class:?} broke");
            // An explosive also blasts its neighbours (bricks::explosive tests).
            if !matches!(class, BrickClass::Explosive(_)) {
                assert_eq!(score(&app), 10 * max as i32, "{class:?}");
                assert_eq!(bricks(&mut app).len(), BRICK_ROWS * BRICK_COLS - 1);
            }
        }
    }

    #[test]
    fn clearing_every_brick_including_multi_hit_ones_wins() {
        let mut app = app();
        tap(&mut app, KeyCode::Space);
        let a = brick_of(&mut app, BrickClass::Ceramic);
        let b = brick_of(&mut app, BrickClass::Titanium);
        for brick in bricks(&mut app) {
            if brick != a && brick != b {
                app.world_mut().despawn(brick);
            }
        }

        hit(&mut app, a);
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::InGame, "one brick is left");

        hit(&mut app, b);
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::InGame, "a damaged brick is left");

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
    fn every_brick_gets_its_class_look_health_and_cell() {
        let mut app = app();
        assert_eq!(BRICK_ROWS * BRICK_COLS, 70);
        let world = app.world_mut();
        let mut cells = std::collections::HashSet::new();
        for (class, health, sprite, transform, cell) in world
            .query_filtered::<(&BrickClass, &BrickHealth, &Sprite, &Transform, &BrickCell), With<Brick>>()
            .iter(world)
        {
            assert_eq!(health.0, class.max_hits());
            assert_eq!(sprite.color, theme::brick_color(*class));
            assert_eq!(transform.translation, brick_translation(*cell));
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
        // Rows step down from the top margin.
        assert_eq!(
            brick_y(0) + BRICK_HEIGHT / 2.0,
            PLAYFIELD_HEIGHT / 2.0 - BRICK_TOP_MARGIN
        );
        assert!((brick_y(0) - brick_y(1) - (BRICK_HEIGHT + BRICK_GAP)).abs() < 1e-3);
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
            (BALL_SPEED, 300.0),
            (BALL_ANCHOR_GAP, 2.0),
            (PADDLE_STILL_SPEED, 1.0),
            (BRICK_GAP, 5.0),
            (BRICK_TOP_MARGIN, 50.0),
            (BRICK_HEIGHT, 30.0),
        ] {
            assert_eq!(scaled, design * GAME_SCALE);
        }
        // Unit-free tuning stays put.
        assert_eq!(PADDLE_MASS, 3.0);
        assert_eq!(PADDLE_LINEAR_DAMPING, 4.0);
        assert_eq!(BALL_MIN_VERTICAL_FRACTION, 0.3);
    }

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
    fn shield_glass_hit_from_below_or_the_side_only_flashes() {
        let mut app = app();
        let shield = brick_of(&mut app, BrickClass::Shield);
        let color = |app: &App| app.world().get::<Sprite>(shield).unwrap().color;

        hit_moving(&mut app, shield, Vec2::new(0.0, BALL_SPEED));
        assert_eq!(app.world().get::<BrickHealth>(shield).unwrap().0, 1);
        assert_eq!(score(&app), 0);
        assert!(!app.world().resource::<BallCollisionSignals>().broke_brick);
        assert_eq!(color(&app), theme::SHIELD_FLASH);
        assert!(app.world().entity(shield).contains::<bricks::ShieldFlash>());
        app.update(); // 0.1 s
        assert_eq!(color(&app), theme::SHIELD_FLASH);
        app.update(); // 0.2 s
        assert_eq!(color(&app), theme::SHIELD);
        assert!(!app.world().entity(shield).contains::<bricks::ShieldFlash>());

        // A flat side hit: no damage either.
        hit_moving(&mut app, shield, Vec2::new(BALL_SPEED, 0.0));
        assert!(app.world().get_entity(shield).is_ok());
        assert_eq!(score(&app), 0);
        assert_eq!(color(&app), theme::SHIELD_FLASH);
    }

    #[test]
    fn shield_glass_hit_by_a_ball_moving_down_breaks() {
        let mut app = app();
        let shield = brick_of(&mut app, BrickClass::Shield);
        hit_moving(&mut app, shield, FROM_ABOVE);
        assert!(app.world().get_entity(shield).is_err());
        assert_eq!(score(&app), 10);
        assert!(app.world().resource::<BallCollisionSignals>().broke_brick);
    }

    #[test]
    fn a_shield_flash_holds_while_paused() {
        let mut app = app();
        let shield = brick_of(&mut app, BrickClass::Shield);
        hit_moving(&mut app, shield, Vec2::new(0.0, BALL_SPEED));
        tap(&mut app, KeyCode::KeyP);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Sprite>(shield).unwrap().color,
            theme::SHIELD_FLASH
        );
        tap(&mut app, KeyCode::KeyP);
        app.update();
        app.update();
        assert_eq!(
            app.world().get::<Sprite>(shield).unwrap().color,
            theme::SHIELD
        );
    }

    #[test]
    fn the_ball_approach_is_recorded_before_each_physics_step() {
        let mut app = app();
        tap(&mut app, KeyCode::Space);
        let ball = ball(&mut app);
        app.world_mut().get_mut::<LinearVelocity>(ball).unwrap().0 = Vec2::new(100.0, -280.0);
        app.update();
        let approach = app.world().get::<BallApproach>(ball).unwrap().0;
        assert!(
            approach.y < 0.0,
            "recorded the downward velocity, got {approach:?}"
        );
    }

    #[test]
    fn the_run_is_won_only_once_the_last_shield_breaks_from_above() {
        let mut app = app();
        tap(&mut app, KeyCode::Space);
        let ceramic = brick_of(&mut app, BrickClass::Ceramic);
        let shield = brick_of(&mut app, BrickClass::Shield);
        for brick in bricks(&mut app) {
            if brick != ceramic && brick != shield {
                app.world_mut().despawn(brick);
            }
        }
        hit(&mut app, ceramic);
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::InGame);

        hit_moving(&mut app, shield, Vec2::new(0.0, BALL_SPEED));
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::InGame, "the shield survived");

        hit_moving(&mut app, shield, FROM_ABOVE);
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::GameOver);
        assert_eq!(
            app.world().get_resource::<GameOutcome>(),
            Some(&GameOutcome::Won)
        );
    }

    #[test]
    fn the_ball_is_a_round_steel_mesh() {
        let mut app = app();
        let ball = ball(&mut app);
        let entity = app.world().entity(ball);
        assert!(entity.contains::<Mesh2d>());
        assert!(!entity.contains::<Sprite>());
        let material = entity
            .get::<MeshMaterial2d<ColorMaterial>>()
            .unwrap()
            .0
            .clone();
        let color = app
            .world()
            .resource::<Assets<ColorMaterial>>()
            .get(&material)
            .unwrap()
            .color;
        assert_eq!(color, theme::STEEL);
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
                ("LIVES ".to_string(), theme::LABEL),
                ("SCORE ".to_string(), theme::LABEL)
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
    fn in_mouse_mode_a_paddle_edge_hit_still_spins_the_ball() {
        let mut app = app();
        assert_eq!(
            app.world().resource::<ControlSettings>().paddle,
            PaddleControl::Mouse
        );
        tap(&mut app, KeyCode::Space);
        aim_mouse_at(&mut app, 0.0);
        let (ball, paddle) = (ball(&mut app), paddle(&mut app));
        // Coming down onto the paddle's right edge.
        {
            let paddle_at = translation(&app, paddle);
            let mut transform = app.world_mut().get_mut::<Transform>(ball).unwrap();
            transform.translation.x = paddle_at.x + PADDLE_WIDTH * 0.45;
        }
        app.world_mut().get_mut::<LinearVelocity>(ball).unwrap().0 = Vec2::new(0.0, -BALL_SPEED);
        app.world_mut().trigger(CollisionStart {
            collider1: ball,
            collider2: paddle,
            body1: Some(ball),
            body2: Some(paddle),
        });
        app.update();

        let v = ball_velocity(&mut app);
        assert!(v.y > 0.0, "bounced up, got {v:?}");
        // A centre hit goes straight up; near the edge the ball leaves at a
        // clearly angled side trajectory (over 25° off vertical).
        assert!(
            v.x / v.y > 25f32.to_radians().tan(),
            "near the edge the ball leaves at a side angle, got {v:?}"
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

    #[derive(Resource, Default)]
    struct Seen {
        damaged: Vec<(Entity, Vec2, BrickClass)>,
        destroyed: Vec<(Entity, Vec2, BrickClass, bool)>,
    }

    #[test]
    fn brick_damage_and_break_events_carry_class_and_position() {
        let mut app = app();
        app.init_resource::<Seen>()
            .add_observer(|on: On<BrickDamaged>, mut seen: ResMut<Seen>| {
                seen.damaged.push((on.brick, on.position, on.class));
            })
            .add_observer(|on: On<BrickDestroyed>, mut seen: ResMut<Seen>| {
                seen.destroyed
                    .push((on.brick, on.position, on.class, on.by_blast));
            });
        let titanium = brick_of(&mut app, BrickClass::Titanium);
        let centre = translation(&app, titanium).truncate();
        let ball = ball(&mut app);
        let ball_at = translation(&app, ball).truncate();

        hit(&mut app, titanium);
        hit(&mut app, titanium);
        let seen = app.world().resource::<Seen>();
        // The surviving hit: at the contact point (the ball), with its class.
        assert_eq!(seen.damaged, [(titanium, ball_at, BrickClass::Titanium)]);
        // The break: at the brick's centre, by the ball.
        assert_eq!(
            seen.destroyed,
            [(titanium, centre, BrickClass::Titanium, false)]
        );
    }
}
