mod ball;
mod bricks;
mod collision;
mod controls;
mod game_state;
mod menu;
mod paddle;
mod particles;
mod powerups;
mod run;
mod script_manager;
mod spawner;
mod sprites;
#[cfg(test)]
pub(crate) mod test_support;
mod theme;
mod view;
mod world;

use avian2d::prelude::*;
use ball::{ball_movement, follow_paddle, launch_ball, record_ball_approach};
use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use collision::{on_ball_collision, BallCollisionSignals};
use game_state::{AppState, GameStatePlugin, PlayState};
use paddle::{paddle_movement, place_paddle_pieces, PaddleMovementSet};
use run::{restart_from_game_over, start_run, update_hud, Lives, Score, STARTING_LIVES};
use world::{setup_level, GAME_SCALE};

/// The native window's opening size; it is resizable and the world scales.
const WINDOW_START_WIDTH: u32 = 1280;
const WINDOW_START_HEIGHT: u32 = 720;

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

#[cfg(test)]
mod tests {
    use super::*;

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
