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
//! A second observer, [`on_ball_bounce`], turns every contact with a wall,
//! brick or the paddle into [`BallBounced`] at the contact point, and a
//! paddle contact into [`PaddleHit`] on the paddle's top edge. These exist
//! for the VFX layer (`particles`), which observes them; gameplay doesn't
//! react to them.
//!
//! [`BallCollisionSignals`] records what happened this frame (a brick broke,
//! where the paddle was hit) for [`crate::ball::ball_movement`] to consume.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::ball::{Ball, BallApproach};
use crate::bricks::grid::{Brick, BrickHealth};
use crate::bricks::{self, BrickClass};
use crate::paddle::{Paddle, PADDLE_HEIGHT};
use crate::run::Score;
use crate::theme;
use crate::world::Wall;

/// What the ball bounced off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BounceSurface {
    Wall,
    Brick,
    Paddle,
}

/// Fired by [`on_ball_collision`] for every ball contact with a wall, a brick
/// (shield deflects and breaks included) or the paddle, at the contact point.
#[derive(Event, Debug)]
pub(crate) struct BallBounced {
    pub(crate) position: Vec2,
    pub(crate) surface: BounceSurface,
}

/// Fired by [`on_ball_collision`] when the ball hits the paddle, at the point
/// on the paddle's top edge under the ball (clamped to the paddle's ends).
#[derive(Event, Debug)]
pub(crate) struct PaddleHit {
    pub(crate) paddle: Entity,
    pub(crate) position: Vec2,
}

/// Where the paddle's top edge is under a ball at `ball_x`: clamped to a
/// paddle `width` wide centred at `paddle`.
pub(crate) fn paddle_hit_point(ball_x: f32, paddle: Vec2, width: f32) -> Vec2 {
    let half = width / 2.0;
    Vec2::new(
        ball_x.clamp(paddle.x - half, paddle.x + half),
        paddle.y + PADDLE_HEIGHT / 2.0,
    )
}

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

/// What [`on_ball_bounce`] reads off whatever the ball touched.
type BounceSurfaceData = (
    &'static Collider,
    &'static Transform,
    Has<Wall>,
    Has<Brick>,
    Option<&'static Paddle>,
);

/// Observes the same `CollisionStart` as [`on_ball_collision`] (see there for
/// why an observer) and reports the bounce for the VFX layer: a
/// [`BallBounced`] at the point on a wall's, brick's or the paddle's collider
/// nearest the ball, plus a [`PaddleHit`] on the paddle's top edge for a
/// paddle contact. Anything else the ball touches isn't a bounce. Gameplay
/// doesn't depend on these, so they stay out of [`on_ball_collision`].
pub(crate) fn on_ball_bounce(
    on: On<CollisionStart>,
    mut commands: Commands,
    balls: Query<&Transform, With<Ball>>,
    surfaces: Query<BounceSurfaceData>,
) {
    let Ok(ball) = balls.get(on.collider1) else {
        return;
    };
    let ball = ball.translation.truncate();
    let other = on.collider2;
    let Ok((collider, transform, is_wall, is_brick, paddle)) = surfaces.get(other) else {
        return;
    };
    let surface = match (is_wall, is_brick, paddle) {
        (true, _, _) => BounceSurface::Wall,
        (_, true, _) => BounceSurface::Brick,
        (_, _, Some(_)) => BounceSurface::Paddle,
        _ => return,
    };
    let at = transform.translation.truncate();
    let (position, _) = collider.project_point(at, transform.rotation, ball, true);
    commands.trigger(BallBounced { position, surface });
    if let Some(paddle) = paddle {
        commands.trigger(PaddleHit {
            paddle: other,
            position: paddle_hit_point(ball.x, at, paddle.width),
        });
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
