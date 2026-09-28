//! Player control settings and the mouse side of paddle control.
//!
//! [`ControlSettings`] is session-only (it resets to Mouse on every launch)
//! and is toggled from the Settings screen. While playing in Mouse mode,
//! [`track_cursor`] turns cursor movement into a [`PaddleTarget`];
//! `paddle_movement` in `main.rs` then drives the paddle's velocity toward it
//! with [`follow_velocity`]. The target and clamp maths are pure functions
//! so they're testable without a window.

use crate::game_state::PlayState;
use crate::{RestartGame, WINDOW_WIDTH};
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
const FOLLOW_GAIN: f32 = 20.0;
/// Top horizontal speed while following the mouse.
const MAX_FOLLOW_SPEED: f32 = 2000.0;
/// Most of the gap a single frame may close. The velocity is set once per
/// frame but Avian integrates it over every fixed step in that frame, so at
/// a low frame rate an uncapped gain would overshoot and ring.
const MAX_GAP_PER_FRAME: f32 = 0.8;

/// The paddle-centre X for a cursor at `cursor_x`, clamped so a paddle of
/// `paddle_width` stays between the side walls.
pub fn clamp_paddle_x(cursor_x: f32, paddle_width: f32) -> f32 {
    let half_room = ((WINDOW_WIDTH - paddle_width) / 2.0).max(0.0);
    cursor_x.clamp(-half_room, half_room)
}

/// Horizontal velocity that moves a paddle at `paddle_x` toward `target_x`
/// (already clamped), proportional to the gap and capped. Driving velocity
/// rather than teleporting keeps Avian resolving the ball bounce.
///
/// `frame_dt` is this frame's length in seconds: the gain is limited so the
/// frame closes at most [`MAX_GAP_PER_FRAME`] of the gap however many physics
/// steps run in it, so a slow frame rate can't make the paddle overshoot.
pub fn follow_velocity(paddle_x: f32, target_x: f32, frame_dt: f32) -> f32 {
    let gain = if frame_dt > 0.0 {
        FOLLOW_GAIN.min(MAX_GAP_PER_FRAME / frame_dt)
    } else {
        FOLLOW_GAIN
    };
    ((target_x - paddle_x) * gain).clamp(-MAX_FOLLOW_SPEED, MAX_FOLLOW_SPEED)
}

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ControlSettings>()
            .init_resource::<PaddleTarget>()
            .add_observer(clear_target_on_restart)
            .add_systems(
                Update,
                track_cursor
                    .before(crate::PaddleMovementSet)
                    .run_if(in_state(PlayState::Playing)),
            );
    }
}

/// Sets the paddle target when the cursor moves over the window (Mouse mode
/// only). Does nothing without a window or camera, e.g. in headless tests.
fn track_cursor(
    settings: Res<ControlSettings>,
    mut target: ResMut<PaddleTarget>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
) {
    if settings.paddle != PaddleControl::Mouse {
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
mod tests {
    use super::*;
    use crate::PADDLE_WIDTH;

    #[test]
    fn clamp_keeps_the_paddle_between_the_walls() {
        let edge = (WINDOW_WIDTH - PADDLE_WIDTH) / 2.0;
        assert_eq!(clamp_paddle_x(0.0, PADDLE_WIDTH), 0.0);
        assert_eq!(clamp_paddle_x(123.0, PADDLE_WIDTH), 123.0);
        assert_eq!(clamp_paddle_x(10_000.0, PADDLE_WIDTH), edge);
        assert_eq!(clamp_paddle_x(-10_000.0, PADDLE_WIDTH), -edge);
    }

    #[test]
    fn clamp_respects_a_wider_paddle() {
        let wide = PADDLE_WIDTH * 1.5;
        assert_eq!(clamp_paddle_x(10_000.0, wide), (WINDOW_WIDTH - wide) / 2.0);
        // Wider than the play area: stay centred rather than invert the range.
        assert_eq!(clamp_paddle_x(300.0, WINDOW_WIDTH * 2.0), 0.0);
    }

    #[test]
    fn follow_velocity_heads_for_the_target_and_is_capped() {
        let dt = 1.0 / 60.0;
        assert_eq!(follow_velocity(0.0, 0.0, dt), 0.0);
        assert!(follow_velocity(0.0, 10.0, dt) > 0.0);
        assert!(follow_velocity(0.0, -10.0, dt) < 0.0);
        assert_eq!(follow_velocity(-400.0, 400.0, dt), MAX_FOLLOW_SPEED);
        assert_eq!(follow_velocity(400.0, -400.0, dt), -MAX_FOLLOW_SPEED);
    }

    #[test]
    fn a_slow_frame_never_overshoots_the_target() {
        for fps in [240.0, 60.0, 20.0, 10.0, 5.0, 2.0] {
            let dt = 1.0 / fps;
            let gap = 50.0;
            let moved = follow_velocity(0.0, gap, dt) * dt;
            assert!(
                moved > 0.0 && moved <= gap * MAX_GAP_PER_FRAME + 1e-3,
                "{fps} fps moved {moved}"
            );
        }
    }

    #[test]
    fn paddle_control_toggles_and_labels() {
        assert_eq!(ControlSettings::default().paddle, PaddleControl::Mouse);
        assert_eq!(PaddleControl::Mouse.toggled(), PaddleControl::Keyboard);
        assert_eq!(PaddleControl::Keyboard.toggled(), PaddleControl::Mouse);
        assert_eq!(PaddleControl::Mouse.label(), "Paddle control: Mouse");
        assert_eq!(PaddleControl::Keyboard.label(), "Paddle control: Keyboard");
    }
}
