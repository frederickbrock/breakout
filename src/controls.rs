//! Player control settings and the mouse side of paddle control.
//!
//! [`ControlSettings`] is session-only (it resets to Mouse on every launch)
//! and is toggled from the Settings screen. While playing in Mouse mode,
//! [`track_cursor`] turns cursor movement into a [`PaddleTarget`];
//! [`crate::paddle::paddle_movement`] then drives the paddle's velocity
//! toward it with [`follow_velocity`]. The target and clamp maths are pure
//! functions so they're testable without a window.
//!
//! [`clamp_paddle_x`] keeps the target between the walls, inside the
//! playfield well, for the current `Paddle.width`; [`follow_velocity`] is a
//! capped proportional drive, limited to about 80% of the gap per frame so
//! low frame rates don't overshoot. Arrow keys / A/D push the paddle in both
//! modes, and a held key clears the mouse target. Tests set [`PaddleTarget`]
//! directly (no window).

use crate::game_state::PlayState;
use crate::run::RestartGame;
use crate::tuning::PaddleTuning;
use crate::world::{GAME_SCALE, PLAYFIELD_WIDTH};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// How the player steers the paddle. Arrow keys and A/D work in both modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaddleControl {
    /// The paddle also follows the cursor's X position.
    #[default]
    Mouse,
    /// Only the keyboard moves the paddle.
    Keyboard,
}

impl PaddleControl {
    pub fn toggled(self) -> Self {
        match self {
            Self::Mouse => Self::Keyboard,
            Self::Keyboard => Self::Mouse,
        }
    }

    /// The Settings button's label for this value.
    pub fn label(self) -> &'static str {
        match self {
            Self::Mouse => "Paddle control: Mouse",
            Self::Keyboard => "Paddle control: Keyboard",
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct ControlSettings {
    pub paddle: PaddleControl,
}

/// Where the mouse wants the paddle's centre (world X), set when the cursor
/// moves during play. Holding a movement key clears it, so the keyboard
/// isn't fought by a stale cursor position.
#[derive(Resource, Debug, Default)]
pub struct PaddleTarget {
    pub x: Option<f32>,
    /// The cursor's last world X, to tell real movement from a still cursor.
    last_cursor_x: Option<f32>,
}

/// How quickly the paddle closes the gap to its target, per second.
pub(crate) const FOLLOW_GAIN: f32 = 20.0;
/// Top horizontal speed while following the mouse.
pub(crate) const MAX_FOLLOW_SPEED: f32 = 2000.0 * GAME_SCALE;
/// Most of the gap a single frame may close. The velocity is set once per
/// frame but Avian integrates it over every fixed step in that frame, so at
/// a low frame rate an uncapped gain would overshoot and ring.
pub(crate) const MAX_GAP_PER_FRAME: f32 = 0.8;

/// The paddle-centre X for a cursor at `cursor_x`, clamped so a paddle of
/// `paddle_width` stays between the side walls, inside the playfield well.
pub fn clamp_paddle_x(cursor_x: f32, paddle_width: f32) -> f32 {
    let half_room = ((PLAYFIELD_WIDTH - paddle_width) / 2.0).max(0.0);
    cursor_x.clamp(-half_room, half_room)
}

/// Horizontal velocity that moves a paddle at `paddle_x` toward `target_x`
/// (already clamped), proportional to the gap and capped. Driving velocity
/// rather than teleporting keeps Avian resolving the ball bounce.
///
/// `frame_dt` is this frame's length in seconds: the gain is limited so the
/// frame closes at most `max_gap_per_frame` of the gap however many physics
/// steps run in it, so a slow frame rate can't make the paddle overshoot.
/// The gain, cap and per-frame limit come from `Tuning.paddle` (defaults
/// [`FOLLOW_GAIN`], [`MAX_FOLLOW_SPEED`], [`MAX_GAP_PER_FRAME`]).
pub fn follow_velocity(paddle_x: f32, target_x: f32, frame_dt: f32, tuning: &PaddleTuning) -> f32 {
    let gain = if frame_dt > 0.0 {
        tuning.follow_gain.min(tuning.max_gap_per_frame / frame_dt)
    } else {
        tuning.follow_gain
    };
    ((target_x - paddle_x) * gain).clamp(-tuning.max_follow_speed, tuning.max_follow_speed)
}

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ControlSettings>()
            .init_resource::<PaddleTarget>()
            .init_resource::<PointerCaptured>()
            .add_observer(clear_target_on_restart)
            .add_systems(
                Update,
                track_cursor
                    .before(crate::paddle::PaddleMovementSet)
                    .run_if(in_state(PlayState::Playing)),
            );
    }
}

/// The pointer is busy with an on-screen UI (the native dev console), so the
/// paddle doesn't follow it. Always false unless such a UI sets it.
#[derive(Resource, Debug, Default)]
pub struct PointerCaptured(pub bool);

/// Sets the paddle target when the cursor moves over the window (Mouse mode
/// only), unless a UI has the pointer ([`PointerCaptured`]). Does nothing
/// without a window or camera, e.g. in headless tests.
fn track_cursor(
    settings: Res<ControlSettings>,
    captured: Res<PointerCaptured>,
    mut target: ResMut<PaddleTarget>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
) {
    if settings.paddle != PaddleControl::Mouse || captured.0 {
        return;
    }
    let (Ok(window), Ok((camera, camera_transform))) = (window.single(), camera.single()) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(camera_transform, cursor) else {
        return;
    };
    if target.last_cursor_x != Some(world.x) {
        target.last_cursor_x = Some(world.x);
        target.x = Some(world.x);
    }
}

/// A new run doesn't chase where the cursor was in the last one.
fn clear_target_on_restart(_restart: On<RestartGame>, mut target: ResMut<PaddleTarget>) {
    target.x = None;
}

#[cfg(test)]
mod tests;
