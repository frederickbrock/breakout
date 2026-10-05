//! Super-Sizer: temporarily widens the paddle.
//!
//! This is the pattern for adding a new power-up: a new file with its own
//! `Plugin` that
//! 1. registers itself into the shared [`PowerUpSpawner`] registry at
//!    `build()` time, via
//!    `app.world_mut().resource_mut::<PowerUpSpawner>().register(...)`,
//! 2. reacts to [`PowerUpCollected`] with its own observer, and
//! 3. is composed in by `PowerUpsPlugin`'s `.add_plugins(...)`.
//!
//! No shared match statement to edit: only [`PowerUpKind`] needs a new
//! variant centrally.

use super::{ActiveEffects, PowerUpCollected, PowerUpKind, PowerUpSpawner, TickActiveEffects};
use crate::paddle::{Paddle, PADDLE_HEIGHT};
use crate::tuning::Tuning;
use avian2d::prelude::*;
use bevy::prelude::*;

pub(crate) const WEIGHT: f32 = 1.0;
pub(crate) const WIDTH_MULTIPLIER: f32 = 1.25;
pub(crate) const DURATION: f32 = 7.0;

pub struct SuperSizerPlugin;

impl Plugin for SuperSizerPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().resource_mut::<PowerUpSpawner>().register(
            PowerUpKind::SuperSizer,
            WEIGHT,
            crate::theme::POWER_UP,
        );

        app.add_observer(effect)
            .add_systems(Update, apply_weight.run_if(resource_changed::<Tuning>))
            .add_systems(
                Update,
                update_paddle_width
                    .after(TickActiveEffects)
                    .before(crate::paddle::PaddleMovementSet),
            );
    }
}

fn effect(on: On<PowerUpCollected>, mut active: ResMut<ActiveEffects>, tuning: Res<Tuning>) {
    if on.kind == PowerUpKind::SuperSizer {
        active.refresh_or_insert(
            PowerUpKind::SuperSizer,
            tuning.powerups.super_sizer.duration,
        );
    }
}

/// Keeps the spawner's Super-Sizer weight in step with `Tuning` (runs when
/// it changes, including when it's first inserted).
fn apply_weight(tuning: Res<Tuning>, mut spawner: ResMut<PowerUpSpawner>) {
    spawner.set_weight(PowerUpKind::SuperSizer, tuning.powerups.super_sizer.weight);
}

fn update_paddle_width(
    active: Res<ActiveEffects>,
    tuning: Res<Tuning>,
    mut paddle_query: Query<(&mut Paddle, &mut Collider)>,
) {
    let base = tuning.paddle.width;
    // The paddle's visual pieces follow `Paddle.width` (`place_paddle_pieces`).
    let Ok((mut paddle, mut collider)) = paddle_query.single_mut() else {
        return;
    };
    let width = if active.is_active(PowerUpKind::SuperSizer) {
        base * tuning.powerups.super_sizer.width_multiplier
    } else {
        base
    };
    if paddle.width != width {
        paddle.width = width;
        *collider = Collider::rectangle(width, PADDLE_HEIGHT);
    }
}
