//! The power-up framework: the [`PowerUp`] component, the
//! [`PowerUpCollected`] event, and the drop, fall and paddle-pickup systems.
//!
//! A power-up only ever appears when a power-up brick breaks: each level,
//! `attach_reactor_power_ups` (a [`crate::campaign::LevelStarted`] observer)
//! gives every reactor-class brick, and every brick a level flagged
//! `powerup` (`CarriesPowerUp`), a [`PowerUpBrick`] with a kind picked from
//! [`PowerUpSpawner`], and `drop_power_up` observes
//! [`crate::collision::BrickDestroyed`] and spawns it at the brick's position
//! (no timed drops). `reset_on_level_start` clears active effects and falling
//! power-ups at every level start; `reset_on_restart` (on
//! [`crate::run::RestartGame`]) resets the drop count, which is per run.
//!
//! [`ActiveEffects`] is where collected effects land. Consumers recompute
//! their derived values from it every frame, so an effect expiring needs no
//! explicit revert step. [`TickActiveEffects`] is an ordering-only set for
//! those consumers. Each concrete power-up is its own plugin (see
//! [`super_sizer`]). [`capsules`] shows each active effect's time left in
//! the right panel.

pub(crate) mod capsules;
mod super_sizer;

use crate::bricks::grid::{Brick, CarriesPowerUp};
use crate::bricks::BrickClass;
use crate::campaign::LevelStarted;
use crate::collision::BrickDestroyed;
use crate::game_state::{AppState, PlayState};
use crate::paddle::{Paddle, PADDLE_HEIGHT};
use crate::run::RestartGame;
use crate::spawner::Spawner;
use crate::world::{GAME_SCALE, PLAYFIELD_HEIGHT};
use bevy::prelude::*;

const POWER_UP_SIZE: f32 = 24.0 * GAME_SCALE;
const BASE_GRAVITY: f32 = 140.0 * GAME_SCALE;
const GRAVITY_STEP: f32 = 20.0 * GAME_SCALE;
const MAX_GRAVITY: f32 = 420.0 * GAME_SCALE;

pub type PowerUpSpawner = Spawner<PowerUpKind>;

/// Every power-up type. Adding a new power-up: add a variant here, and a new
/// file/module (see [`super_sizer`]) with its own `Plugin` that registers
/// itself with [`PowerUpSpawner`] and reacts to [`PowerUpCollected`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerUpKind {
    SuperSizer,
}

#[derive(Component)]
pub struct PowerUp {
    kind: PowerUpKind,
    velocity: Vec2,
    gravity: f32,
}

/// A brick carrying a power-up: attached to each reactor-class brick at the
/// start of a run by [`attach_reactor_power_ups`], dropped by
/// [`drop_power_up`] when it breaks.
#[derive(Component)]
pub struct PowerUpBrick {
    kind: PowerUpKind,
    color: Color,
}

/// Power-ups dropped so far this run; each successive drop falls a little
/// faster.
#[derive(Resource, Default)]
struct PowerUpDrops(u32);

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
        // The registry must exist before `SuperSizerPlugin` registers into it.
        app.init_resource::<PowerUpSpawner>()
            .init_resource::<PowerUpDrops>()
            .init_resource::<ActiveEffects>()
            .add_observer(reset_on_restart)
            .add_observer(reset_on_level_start)
            .add_observer(attach_reactor_power_ups)
            .add_observer(drop_power_up)
            // Everything that moves power-ups or counts down their timers
            // runs only while playing, so pausing or ending the run freezes
            // falling power-ups and active effects alike.
            .add_systems(
                Update,
                (power_up_physics, power_up_paddle_collision)
                    .chain()
                    .run_if(in_state(PlayState::Playing)),
            )
            .add_systems(
                Update,
                tick_active_effects
                    .in_set(TickActiveEffects)
                    .before(crate::paddle::PaddleMovementSet)
                    .run_if(in_state(PlayState::Playing)),
            )
            .add_plugins((super_sizer::SuperSizerPlugin, capsules::CapsulesPlugin));
    }
}

/// A brick that doesn't carry a power-up yet.
type UnequippedBrick = (With<Brick>, Without<PowerUpBrick>);

/// At the start of every level, gives each reactor-class brick and each
/// brick a level flagged `powerup` ([`CarriesPowerUp`]) a weighted-random
/// power-up from [`PowerUpSpawner`]. Runs on [`LevelStarted`], which is
/// triggered after the board's spawns are queued, so each level's bricks
/// already exist. Bricks already equipped are skipped. Health and colour stay
/// the class's.
fn attach_reactor_power_ups(
    _level: On<LevelStarted>,
    mut commands: Commands,
    spawner: Res<PowerUpSpawner>,
    bricks: Query<(Entity, &BrickClass, Has<CarriesPowerUp>), UnequippedBrick>,
) {
    for (entity, class, flagged) in &bricks {
        if *class != BrickClass::Reactor && !flagged {
            continue;
        }
        let Some(pick) = spawner.pick() else {
            return;
        };
        commands.entity(entity).insert(PowerUpBrick {
            kind: pick.kind,
            color: pick.color,
        });
    }
}

/// A broken power-up brick drops its power-up where it stood. Each
/// successive drop in a run is a little heavier.
fn drop_power_up(
    on: On<BrickDestroyed>,
    mut commands: Commands,
    mut drops: ResMut<PowerUpDrops>,
    bricks: Query<&PowerUpBrick>,
) {
    let Ok(power_up_brick) = bricks.get(on.brick) else {
        return;
    };
    let gravity = (BASE_GRAVITY + drops.0 as f32 * GRAVITY_STEP).min(MAX_GRAVITY);
    drops.0 += 1;
    commands.spawn((
        Sprite::from_color(power_up_brick.color, Vec2::splat(POWER_UP_SIZE)),
        Transform::from_xyz(on.position.x, on.position.y, 0.5),
        PowerUp {
            kind: power_up_brick.kind,
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

        if transform.translation.y < -PLAYFIELD_HEIGHT / 2.0 - POWER_UP_SIZE {
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

/// The drop-gravity ramp is per run: a new run starts it over.
fn reset_on_restart(_restart: On<RestartGame>, mut drops: ResMut<PowerUpDrops>) {
    drops.0 = 0;
}

/// Each level starts clean: no active effects and no falling power-ups.
fn reset_on_level_start(
    _level: On<LevelStarted>,
    mut commands: Commands,
    mut active: ResMut<ActiveEffects>,
    power_ups: Query<Entity, With<PowerUp>>,
) {
    active.clear();
    for entity in &power_ups {
        commands.entity(entity).despawn();
    }
}

/// Spawns a falling power-up the way [`drop_power_up`] does (flat-colour
/// sprite included), for tests in other modules.
#[cfg(test)]
pub(crate) fn test_spawn_power_up(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            Sprite::from_color(crate::theme::POWER_UP, Vec2::splat(POWER_UP_SIZE)),
            Transform::from_xyz(0.0, 200.0, 0.5),
            PowerUp {
                kind: PowerUpKind::SuperSizer,
                velocity: Vec2::ZERO,
                gravity: BASE_GRAVITY,
            },
            DespawnOnExit(AppState::InGame),
        ))
        .id()
}

#[cfg(test)]
mod tests;
