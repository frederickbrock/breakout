//! What happens when the ball touches something.
//!
//! [`on_ball_collision`] observes Avian's `CollisionStart` (observer-only; a
//! `MessageReader` never sees it). Each brick hit scores 10 and removes one
//! hit point; a hit the brick survives triggers [`BrickDamaged`], and the
//! last one triggers [`BrickDestroyed`] *before* the despawn, so observers
//! (regen, explosive, power-ups, particles) can still read the brick. Shield
//! glass only takes damage from a ball that was moving down (read from
//! [`crate::ball::BallApproach`]); anything else just flashes it.
//!
//! [`BallCollisionSignals`] records what happened this frame (a brick broke,
//! where the paddle was hit) for [`crate::ball::ball_movement`] to consume.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::ball::{Ball, BallApproach};
use crate::bricks::grid::{Brick, BrickHealth};
use crate::bricks::{self, BrickClass};
use crate::paddle::Paddle;
use crate::run::Score;
use crate::theme;

/// Fired by [`on_ball_collision`] when a brick takes its last hit, *before*
/// the brick is despawned, so observers can still read its other components.
/// `powerups` observes it to drop a power-up brick's power-up; this module
/// knows nothing about power-ups.
#[derive(Event)]
pub(crate) struct BrickDestroyed {
    pub(crate) brick: Entity,
    pub(crate) position: Vec2,
    pub(crate) class: BrickClass,
    /// Destroyed by an explosive's blast rather than the ball. The blast's
    /// whole chain is resolved at once, so this doesn't set off another one.
    pub(crate) by_blast: bool,
}

/// Fired when a brick takes damage but survives (it has hits left).
/// Brick behaviours that react to being hurt (regen's heal timer) observe
/// this instead of being special-cased in [`on_ball_collision`]. Explosions
/// (sim-rdl.7.3) fire it too.
#[derive(Event)]
pub(crate) struct BrickDamaged {
    pub(crate) brick: Entity,
    /// Where the hit landed: the ball's position for a ball hit, the brick's
    /// centre for blast damage.
    pub(crate) position: Vec2,
    pub(crate) class: BrickClass,
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
pub(crate) fn on_ball_collision(
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
/// [`on_ball_collision`] and consumed once per frame by [`ball_movement`](crate::ball::ball_movement).
#[derive(Resource, Default)]
pub(crate) struct BallCollisionSignals {
    pub(crate) broke_brick: bool,
    pub(crate) paddle_hit_x: Option<f32>,
}

#[cfg(test)]
mod tests;
