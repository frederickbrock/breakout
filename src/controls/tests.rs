use super::*;
use crate::paddle::PADDLE_WIDTH;
use crate::tuning::PaddleTuning;

#[test]
fn clamp_keeps_the_paddle_between_the_walls() {
    let edge = (PLAYFIELD_WIDTH - PADDLE_WIDTH) / 2.0;
    assert_eq!(clamp_paddle_x(0.0, PADDLE_WIDTH), 0.0);
    assert_eq!(clamp_paddle_x(123.0, PADDLE_WIDTH), 123.0);
    assert_eq!(clamp_paddle_x(10_000.0, PADDLE_WIDTH), edge);
    assert_eq!(clamp_paddle_x(-10_000.0, PADDLE_WIDTH), -edge);
}

#[test]
fn clamp_respects_a_wider_paddle() {
    let wide = PADDLE_WIDTH * 1.5;
    assert_eq!(
        clamp_paddle_x(10_000.0, wide),
        (PLAYFIELD_WIDTH - wide) / 2.0
    );
    // Wider than the play area: stay centred rather than invert the range.
    assert_eq!(clamp_paddle_x(300.0, PLAYFIELD_WIDTH * 2.0), 0.0);
}

#[test]
fn follow_velocity_heads_for_the_target_and_is_capped() {
    let dt = 1.0 / 60.0;
    assert_eq!(follow_velocity(0.0, 0.0, dt, &PaddleTuning::default()), 0.0);
    assert!(follow_velocity(0.0, 10.0, dt, &PaddleTuning::default()) > 0.0);
    assert!(follow_velocity(0.0, -10.0, dt, &PaddleTuning::default()) < 0.0);
    assert_eq!(
        follow_velocity(-400.0, 400.0, dt, &PaddleTuning::default()),
        MAX_FOLLOW_SPEED
    );
    assert_eq!(
        follow_velocity(400.0, -400.0, dt, &PaddleTuning::default()),
        -MAX_FOLLOW_SPEED
    );
}

#[test]
fn a_slow_frame_never_overshoots_the_target() {
    for fps in [240.0, 60.0, 20.0, 10.0, 5.0, 2.0] {
        let dt = 1.0 / fps;
        let gap = 50.0;
        let moved = follow_velocity(0.0, gap, dt, &PaddleTuning::default()) * dt;
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
