mod ball;
mod bricks;
mod collision;
mod controls;
mod game_state;
mod menu;
mod paddle;
mod particles;
mod powerups;
mod script_manager;
mod spawner;
mod sprites;
#[cfg(test)]
pub(crate) mod test_support;
mod theme;
mod view;
mod world;

use avian2d::prelude::*;
use ball::{
    anchor_position, anchored, ball_movement, follow_paddle, launch_ball, record_ball_approach,
    Ball, BallApproach, BallLook, BALL_SIZE,
};
use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bricks::grid::spawn_bricks;
use collision::{on_ball_collision, BallCollisionSignals};
use game_state::{AppState, GameOutcome, GameStatePlugin, PlayState};
use paddle::{
    paddle_field, paddle_movement, place_paddle_pieces, prong, Paddle, PaddleMovementSet,
    PADDLE_HEIGHT, PADDLE_LINEAR_DAMPING, PADDLE_MARGIN_BOTTOM, PADDLE_MASS, PADDLE_WIDTH,
};
use world::{setup_level, GAME_SCALE, PLAYFIELD_HEIGHT, WORLD_WIDTH};

/// The native window's opening size; it is resizable and the world scales.
const WINDOW_START_WIDTH: u32 = 1280;
const WINDOW_START_HEIGHT: u32 = 720;
/// Lives at the start of every run, including the first.
const STARTING_LIVES: i32 = 3;
const HUD_FONT_SIZE: f32 = 24.0 * GAME_SCALE;
/// Inset of the HUD from the world's left and top edges.
const HUD_MARGIN: f32 = 30.0;
/// Vertical distance between the tops of the SCORE and LIVES blocks.
const HUD_BLOCK_SPACING: f32 = 120.0;
/// Left edge of the HUD text, in the left side panel.
const HUD_X: f32 = -WORLD_WIDTH / 2.0 + HUD_MARGIN;

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
                    resizable: true,
                    // Web only: the canvas follows its parent (the page body,
                    // sized to the viewport by index.html).
                    fit_canvas_to_parent: true,
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
        view::ViewPlugin,
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
        "SCORE\n",
        "0",
        PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN,
        ScoreText,
    );
    spawn_hud_line(
        commands,
        "LIVES\n",
        &STARTING_LIVES.to_string(),
        PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN - HUD_BLOCK_SPACING,
        LivesText,
    );
}

/// A HUD block in the left side panel: an uppercase label line in the label
/// colour with the value span in ink on the line below. Top-left anchored at
/// [`HUD_X`], `y`. `marker` goes on the value span, which [`update_hud`] writes.
fn spawn_hud_line(
    commands: &mut Commands,
    label: &str,
    value: &str,
    y: f32,
    marker: impl Component,
) {
    let font = TextFont {
        font_size: FontSize::Px(HUD_FONT_SIZE),
        ..default()
    };
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Text2d::new(label),
        font.clone(),
        TextColor(theme::LABEL),
        Anchor::TOP_LEFT,
        Transform::from_xyz(HUD_X, y, 1.0),
        children![(TextSpan::new(value), font, TextColor(theme::INK), marker)],
    ));
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

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::ball::*;
    use crate::bricks::grid::*;
    use crate::paddle::*;
    use crate::world::*;

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
