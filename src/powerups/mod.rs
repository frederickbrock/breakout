mod super_sizer;

use crate::game_state::{AppState, PlayState};
use crate::spawner::Spawner;
use crate::{Brick, Paddle, RestartGame, PADDLE_HEIGHT, WINDOW_HEIGHT};
use bevy::prelude::*;
use rand::seq::IndexedRandom;

const POWER_UP_SIZE: f32 = 24.0;
const SPAWN_INTERVAL_MIN: f32 = 7.0;
const SPAWN_INTERVAL_MAX: f32 = 10.0;
const BASE_GRAVITY: f32 = 140.0;
const GRAVITY_STEP: f32 = 20.0;
const MAX_GRAVITY: f32 = 420.0;

pub type PowerUpSpawner = Spawner<PowerUpKind>;

/// Every power-up type. Adding a new power-up: add a variant here, and a new
/// file/module (see [`super_sizer`]) with its own `Plugin` that registers
/// itself with [`PowerUpSpawner`] and reacts to [`PowerUpCollected`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PowerUpKind {
    SuperSizer,
}

#[derive(Component)]
pub struct PowerUp {
    kind: PowerUpKind,
    velocity: Vec2,
    gravity: f32,
}

/// Fires when a falling [`PowerUp`] is caught by the paddle. Each power-up
/// kind reacts to this via its own observer (see [`super_sizer`]).
#[derive(Event)]
pub struct PowerUpCollected {
    pub kind: PowerUpKind,
}

struct ActiveEffect {
    kind: PowerUpKind,
    timer: Timer,
}

/// The "game state" power-up effects land in. Systems that care about a given
/// effect (e.g. paddle width) recompute their derived value from this list
/// every frame, so an effect expiring is just "timer runs out, no longer
/// counted" — there's no separate revert step to maintain.
#[derive(Resource, Default)]
pub struct ActiveEffects(Vec<ActiveEffect>);

impl ActiveEffects {
    fn clear(&mut self) {
        self.0.clear();
    }

    fn is_active(&self, kind: PowerUpKind) -> bool {
        self.0.iter().any(|effect| effect.kind == kind)
    }

    fn refresh_or_insert(&mut self, kind: PowerUpKind, duration: f32) {
        if let Some(effect) = self.0.iter_mut().find(|effect| effect.kind == kind) {
            effect.timer = Timer::from_seconds(duration, TimerMode::Once);
        } else {
            self.0.push(ActiveEffect {
                kind,
                timer: Timer::from_seconds(duration, TimerMode::Once),
            });
        }
    }
}

/// Systems that tick down every active power-up effect's timer are ordered
/// relative to this set, so any power-up's own "recompute derived state from
/// `ActiveEffects`" system can declare `.after(TickActiveEffects)` without
/// needing to know `tick_active_effects`'s function name.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TickActiveEffects;

pub struct PowerUpsPlugin;

impl Plugin for PowerUpsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PowerUpSpawner::new(SPAWN_INTERVAL_MIN..=SPAWN_INTERVAL_MAX))
            .init_resource::<ActiveEffects>()
            .add_observer(reset_on_restart)
            // Everything that moves power-ups or counts down their timers
            // runs only while playing, so pausing or ending the run freezes
            // falling power-ups and active effects alike.
            .add_systems(
                Update,
                (spawn_power_ups, power_up_physics, power_up_paddle_collision)
                    .chain()
                    .run_if(in_state(PlayState::Playing)),
            )
            .add_systems(
                Update,
                tick_active_effects
                    .in_set(TickActiveEffects)
                    .before(crate::PaddleMovementSet)
                    .run_if(in_state(PlayState::Playing)),
            )
            .add_plugins(super_sizer::SuperSizerPlugin);
    }
}

fn spawn_power_ups(
    mut commands: Commands,
    time: Res<Time>,
    mut spawner: ResMut<PowerUpSpawner>,
    brick_query: Query<&Transform, With<Brick>>,
) {
    // `dispensed()` before `tick()` is how many power-ups came before this
    // one (0 for the first), matching the "each successive power-up is a
    // little heavier" ramp; `tick()` itself increments it past this point.
    let dispensed_before_this_one = spawner.dispensed();
    let Some(result) = spawner.tick(time.delta()) else {
        return;
    };

    let mut rng = rand::rng();
    let bricks: Vec<Vec3> = brick_query
        .iter()
        .map(|transform| transform.translation)
        .collect();
    let Some(brick_pos) = bricks.choose(&mut rng) else {
        return;
    };

    let gravity = (BASE_GRAVITY + dispensed_before_this_one as f32 * GRAVITY_STEP).min(MAX_GRAVITY);

    commands.spawn((
        Sprite::from_color(result.color, Vec2::splat(POWER_UP_SIZE)),
        Transform::from_xyz(brick_pos.x, brick_pos.y, 0.5),
        PowerUp {
            kind: result.kind,
            velocity: Vec2::ZERO,
            gravity,
        },
        DespawnOnExit(AppState::InGame),
    ));
}

fn power_up_physics(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut PowerUp)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut power_up) in &mut query {
        power_up.velocity.y -= power_up.gravity * dt;
        transform.translation.y += power_up.velocity.y * dt;

        if transform.translation.y < -WINDOW_HEIGHT / 2.0 - POWER_UP_SIZE {
            commands.entity(entity).despawn();
        }
    }
}

fn power_up_paddle_collision(
    mut commands: Commands,
    paddle_query: Query<(&Transform, &Paddle)>,
    power_up_query: Query<(Entity, &Transform, &PowerUp)>,
) {
    let Ok((paddle_transform, paddle)) = paddle_query.single() else {
        return;
    };
    let paddle_left = paddle_transform.translation.x - paddle.width / 2.0;
    let paddle_right = paddle_transform.translation.x + paddle.width / 2.0;
    let paddle_top = paddle_transform.translation.y + PADDLE_HEIGHT / 2.0;
    let paddle_bottom = paddle_transform.translation.y - PADDLE_HEIGHT / 2.0;
    let half = POWER_UP_SIZE / 2.0;

    for (entity, transform, power_up) in &power_up_query {
        let overlaps_x = transform.translation.x + half >= paddle_left
            && transform.translation.x - half <= paddle_right;
        let overlaps_y = transform.translation.y - half <= paddle_top
            && transform.translation.y + half >= paddle_bottom;

        if overlaps_x && overlaps_y {
            commands.entity(entity).despawn();
            commands.trigger(PowerUpCollected {
                kind: power_up.kind,
            });
        }
    }
}

fn tick_active_effects(time: Res<Time>, mut active: ResMut<ActiveEffects>) {
    let dt = time.delta();
    active
        .0
        .retain_mut(|effect| !effect.timer.tick(dt).is_finished());
}

fn reset_on_restart(
    _restart: On<RestartGame>,
    mut commands: Commands,
    mut spawner: ResMut<PowerUpSpawner>,
    mut active: ResMut<ActiveEffects>,
    power_up_query: Query<Entity, With<PowerUp>>,
) {
    spawner.reset();
    active.clear();
    for entity in &power_up_query {
        commands.entity(entity).despawn();
    }
}
