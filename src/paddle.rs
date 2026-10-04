//! The paddle: its sizes, movement and visual pieces.
//!
//! The [`Paddle`] parent carries the one full-width collider and has no
//! sprite of its own. It is drawn by three children, a left [`PaddleProng`],
//! a stretched [`PaddleField`] and a right prong, laid out by the pure
//! [`paddle_pieces`] and kept in place by [`place_paddle_pieces`] as
//! Super-Sizer changes `Paddle.width`.
//!
//! [`paddle_movement`] pushes the paddle with a `ConstantForce` for the arrow
//! keys / A/D in both control modes; in Mouse mode, with no key held, it
//! drives `LinearVelocity.x` toward the cursor target instead (see
//! [`crate::controls`]), so Avian still resolves ball bounces.
//! [`PaddleMovementSet`] lets systems in other modules (Super-Sizer's width
//! recompute) order themselves before it.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::controls::{self, ControlSettings, PaddleControl, PaddleTarget};
use crate::theme;
use crate::tuning::Tuning;
use crate::world::GAME_SCALE;

pub(crate) const PADDLE_WIDTH: f32 = 120.0 * GAME_SCALE;
pub(crate) const PADDLE_HEIGHT: f32 = 20.0 * GAME_SCALE;
pub(crate) const PADDLE_MASS: f32 = 3.0;
pub(crate) const PADDLE_FORCE: f32 = 7000.0 * GAME_SCALE;
pub(crate) const PADDLE_LINEAR_DAMPING: f32 = 4.0;
pub(crate) const PADDLE_MARGIN_BOTTOM: f32 = 10.0 * GAME_SCALE;
/// Width of each paddle end prong (the sprite is 54 px at 2x).
pub(crate) const PRONG_WIDTH: f32 = 27.0 * GAME_SCALE;

#[derive(Component)]
pub(crate) struct Paddle {
    pub(crate) width: f32,
}

/// The paddle's end prongs, kept at its ends by [`place_paddle_pieces`] as its
/// width changes (Super-Sizer). `side` is -1 (left) or 1 (right).
#[derive(Component)]
pub(crate) struct PaddleProng {
    pub(crate) side: f32,
}

/// The paddle's glowing field between the prongs, stretched by
/// [`place_paddle_pieces`] to fill the gap.
#[derive(Component)]
pub(crate) struct PaddleField;

/// Where the paddle's visual pieces go for a paddle `width` wide: each prong
/// is centred `prong_offset` either side of the middle, and the field fills
/// the `field_width` between them.
#[derive(Debug, PartialEq)]
pub(crate) struct PaddlePieces {
    pub(crate) prong_offset: f32,
    pub(crate) field_width: f32,
}

pub(crate) fn paddle_pieces(width: f32) -> PaddlePieces {
    PaddlePieces {
        prong_offset: (width - PRONG_WIDTH) / 2.0,
        field_width: (width - 2.0 * PRONG_WIDTH).max(0.0),
    }
}

/// Lets other systems (e.g. a power-up that changes paddle width) declare
/// they must run before paddle movement each frame, without `main.rs` having
/// to manually interleave their systems into its own `Update` chain.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PaddleMovementSet;

/// One of the paddle's end prongs; positioned by [`place_paddle_pieces`].
pub(crate) fn prong(side: f32) -> impl Bundle {
    let pieces = paddle_pieces(PADDLE_WIDTH);
    (
        PaddleProng { side },
        Sprite::from_color(theme::EMITTER_PRONG, Vec2::new(PRONG_WIDTH, PADDLE_HEIGHT)),
        Transform::from_xyz(side * pieces.prong_offset, 0.0, 0.1),
    )
}

/// The paddle's middle field; sized by [`place_paddle_pieces`].
pub(crate) fn paddle_field() -> impl Bundle {
    let pieces = paddle_pieces(PADDLE_WIDTH);
    (
        PaddleField,
        Sprite::from_color(theme::EMITTER, Vec2::new(pieces.field_width, PADDLE_HEIGHT)),
        Transform::default(),
    )
}

/// Re-applies the tuned mass and damping to the paddle whenever [`Tuning`]
/// changes (they're physics components set at spawn; force, follow and width
/// are read every frame).
pub(crate) fn apply_paddle_tuning(
    tuning: Res<Tuning>,
    mut paddles: Query<(&mut Mass, &mut LinearDamping), With<Paddle>>,
) {
    for (mut mass, mut damping) in &mut paddles {
        if mass.0 != tuning.paddle.mass {
            mass.0 = tuning.paddle.mass;
        }
        if damping.0 != tuning.paddle.linear_damping {
            damping.0 = tuning.paddle.linear_damping;
        }
    }
}

/// Keeps the prongs at the paddle's ends when its width changes.
pub(crate) fn place_paddle_pieces(
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

/// Arrow keys / A/D push the paddle with a force in both control modes. In
/// Mouse mode, with no movement key held, the paddle's velocity is driven
/// toward the cursor target instead (see `controls`); a held key takes over
/// and clears that target.
pub(crate) fn paddle_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<ControlSettings>,
    mut target: ResMut<PaddleTarget>,
    tuning: Res<Tuning>,
    mut paddle_query: Query<(&Transform, &Paddle, &mut ConstantForce, &mut LinearVelocity)>,
) {
    let tuning = &tuning.paddle;
    let Ok((transform, paddle, mut force, mut velocity)) = paddle_query.single_mut() else {
        return;
    };

    let mut fx = 0.0;
    if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
        fx -= tuning.force;
    }
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
        fx += tuning.force;
    }
    force.0 = Vec2::new(fx, 0.0);

    if fx != 0.0 {
        target.x = None;
        return;
    }
    if settings.paddle == PaddleControl::Mouse {
        if let Some(target_x) = target.x {
            let target_x = controls::clamp_paddle_x(target_x, paddle.width);
            velocity.0.x = controls::follow_velocity(
                transform.translation.x,
                target_x,
                time.delta_secs(),
                tuning,
            );
        }
    }
}

#[cfg(test)]
mod tests;
