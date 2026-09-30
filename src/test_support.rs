//! Headless app running the real game logic (no window, renderer or
//! scripting), with a fixed 100 ms step per `update()` and keyboard input
//! driven by hand via [`tap`].

use super::*;
use crate::bricks::grid::Brick;
use crate::controls::PaddleTarget;
use crate::paddle::Paddle;
use crate::world::PLAYFIELD_HEIGHT;
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

pub(crate) fn move_ball_below_screen(app: &mut App) {
    let mut ball = app
        .world_mut()
        .query_filtered::<&mut Transform, With<Ball>>()
        .single_mut(app.world_mut())
        .expect("a run has exactly one ball");
    ball.translation.y = -PLAYFIELD_HEIGHT;
}

pub(crate) fn ball(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<Ball>>()
        .single(app.world())
        .expect("a run has exactly one ball")
}

pub(crate) fn paddle(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<Paddle>>()
        .single(app.world())
        .expect("a run has exactly one paddle")
}

pub(crate) fn translation(app: &App, entity: Entity) -> Vec3 {
    app.world().get::<Transform>(entity).unwrap().translation
}

pub(crate) fn set_paddle_x(app: &mut App, x: f32) {
    let paddle = paddle(app);
    app.world_mut()
        .get_mut::<Transform>(paddle)
        .unwrap()
        .translation
        .x = x;
}

pub(crate) fn aim_mouse_at(app: &mut App, x: f32) {
    app.world_mut().resource_mut::<PaddleTarget>().x = Some(x);
}
