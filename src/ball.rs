//! The ball: its movement rules, the serve from the paddle and the approach
//! velocity that direction-dependent brick rules read.
//!
//! The ball is a round `Mesh2d(Circle)` whose handles live in [`BallLook`]
//! (made in [`crate::world::setup_level`]). [`ball_movement`] reapplies the
//! paddle-hit spin, keeps the speed at a constant [`BALL_SPEED`] with a
//! minimum vertical component, ends the run on a win and takes a life when
//! the ball falls out.
//!
//! At the start of a run and after every lost life the ball is served from
//! the paddle: it carries [`Anchored`] plus Avian's `RigidBodyDisabled` and
//! `ColliderDisabled` (always together, via [`anchored`]), [`follow_paddle`]
//! keeps it centred on top of the paddle, and [`launch_ball`] (Space or left
//! click, only while `Playing`) sends it off at 45° toward the side the
//! paddle is moving. [`record_ball_approach`] copies the velocity into
//! [`BallApproach`] just before each physics step.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::bricks::grid::Brick;
use crate::collision::BallCollisionSignals;
use crate::game_state::{AppState, GameOutcome};
use crate::paddle::{Paddle, PADDLE_HEIGHT};
use crate::run::{end_run, Lives};
use crate::world::{GAME_SCALE, PLAYFIELD_HEIGHT};

pub(crate) const BALL_SIZE: f32 = 15.0 * GAME_SCALE;
/// Tunable: the ball's speed factor over its old design value, separate from
/// `GAME_SCALE` so the ball can be faster without resizing anything.
pub(crate) const BALL_SPEED_SCALE: f32 = 1.8;
/// The ball's speed (world units/s): its 300 design value times
/// [`BALL_SPEED_SCALE`] (the one gameplay speed not scaled by `GAME_SCALE`).
pub(crate) const BALL_SPEED: f32 = 300.0 * BALL_SPEED_SCALE;
/// Gap between the anchored ball and the paddle, so the launch doesn't start
/// in contact with the paddle (which would trigger the paddle-hit spin rule
/// and override the 45° serve).
pub(crate) const BALL_ANCHOR_GAP: f32 = 2.0 * GAME_SCALE;
/// Below this horizontal paddle speed the paddle counts as still, and the
/// serve goes right.
pub(crate) const PADDLE_STILL_SPEED: f32 = 1.0 * GAME_SCALE;
// Guards against a real failure mode observed in testing: a wall bounce only
// inverts the velocity component perpendicular to the wall, so a ball that
// ends up moving near-perfectly horizontally between the side walls (below
// the bricks, above the paddle) can get permanently stuck bouncing
// side-to-side forever, since nothing left in that lane can ever touch its Y
// velocity again. Keeping a minimum vertical fraction guarantees the ball
// always keeps drifting toward the bricks or the paddle.
pub(crate) const BALL_MIN_VERTICAL_FRACTION: f32 = 0.3;

#[derive(Component)]
pub(crate) struct Ball;

/// The ball's velocity at the start of the current physics step, recorded
/// by [`record_ball_approach`]. Avian triggers `CollisionStart` after its
/// solver, when `LinearVelocity` has usually already been reflected, so
/// direction-dependent rules (shield glass) read this instead.
#[derive(Component, Default)]
pub(crate) struct BallApproach(pub(crate) Vec2);

/// The ball is resting on the paddle waiting to be served (start of a run and
/// after every lost life). While anchored it's out of the simulation — see
/// [`anchored`] — and [`follow_paddle`] carries it along; Space or a left click
/// launches it ([`launch_ball`]).
#[derive(Component)]
pub(crate) struct Anchored;

pub(crate) type FlyingBall = (With<Ball>, Without<Anchored>);
pub(crate) type AnchoredBall = (With<Ball>, With<Anchored>);

/// Mesh and material for the round ball, made once at startup.
#[derive(Resource)]
pub(crate) struct BallLook {
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<ColorMaterial>,
}

/// Copies the ball's velocity into [`BallApproach`] at the start of every
/// physics step, before the solver bounces it.
pub(crate) fn record_ball_approach(
    mut balls: Query<(&LinearVelocity, &mut BallApproach), With<Ball>>,
) {
    for (velocity, mut approach) in &mut balls {
        approach.0 = velocity.0;
    }
}

/// Avian resolves the actual collision physics (detection + bounce angle);
/// this reacts to what [`on_ball_collision`](crate::collision::on_ball_collision) recorded (score, the paddle-hit
/// "spin" feel) and keeps the ball's speed at a controlled, designed
/// magnitude rather than letting raw momentum transfer drift it. Ends the
/// run (switches to [`AppState::GameOver`]) on a win or on losing the last
/// life; the physics clock stops with it, so nothing needs zeroing here.
pub(crate) fn ball_movement(
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
pub(crate) fn anchored() -> (Anchored, RigidBodyDisabled, ColliderDisabled) {
    (Anchored, RigidBodyDisabled, ColliderDisabled)
}

/// Where an anchored ball sits: centred on top of a paddle at `paddle`.
pub(crate) fn anchor_position(paddle: Vec3) -> Vec3 {
    Vec3::new(
        paddle.x,
        paddle.y + PADDLE_HEIGHT / 2.0 + BALL_ANCHOR_GAP + BALL_SIZE / 2.0,
        0.0,
    )
}

/// Keeps an anchored ball on top of the paddle as it moves. Centred, so a
/// paddle-width change (Super-Sizer) doesn't move it.
pub(crate) fn follow_paddle(
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
pub(crate) fn launch_ball(
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

#[cfg(test)]
mod tests;
