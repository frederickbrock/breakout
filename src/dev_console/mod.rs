//! The dev console: an egui panel that edits `Tuning` live while the game
//! runs (`crate::tuning::Tuning`). **Native debug builds with the `dev` feature only** (in `default`):
//! release and wasm builds compile [`DevConsolePlugin`] to nothing, and
//! bevy_egui isn't even a dependency on wasm.
//!
//! - ` (backquote) opens and closes the panel; the game keeps running.
//! - While egui wants the keyboard (a focused number field) or the pointer
//!   (over the panel), bevy_egui's input absorption clears the game's key
//!   and mouse-button input (so typing `p`, `r` or space doesn't pause,
//!   restart or serve), and `controls::PointerCaptured` stops the mouse-follow
//!   paddle.
//! - **Ball:** start speed factor, ramp step and max, min vertical share, and
//!   the current `BallSpeed` (written directly, for instant feel).
//! - **Paddle:** every `Tuning.paddle` field, and starting lives.
//! - **Save** writes the whole `Tuning` to `assets/game.tuning.ron`
//!   (`tuning::tuning_file_text`); the hot reload that follows changes
//!   nothing.
//!   **Reset** puts every value back to its compiled default.
//!
//! Sliders write `Tuning` only when a value actually changes, so its
//! change detection (which re-applies paddle mass and damping) stays quiet.

use bevy::prelude::*;

pub struct DevConsolePlugin;

impl Plugin for DevConsolePlugin {
    fn build(&self, _app: &mut App) {
        #[cfg(all(feature = "dev", debug_assertions, not(target_arch = "wasm32")))]
        imp::build(_app);
    }
}

#[cfg(all(feature = "dev", debug_assertions, not(target_arch = "wasm32")))]
mod imp {
    use super::*;
    use crate::ball::BallSpeed;
    use crate::controls::PointerCaptured;
    use crate::tuning::TUNING_PATH;
    use crate::tuning::{tuning_file_text, Tuning};
    use bevy::asset::io::file::FileAssetReader;
    use bevy_egui::input::{egui_wants_any_pointer_input, EguiWantsInput};
    use bevy_egui::{egui, EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass};

    /// Whether the panel is showing, and the last Save/Reset message.
    #[derive(Resource, Default)]
    struct Console {
        open: bool,
        status: String,
    }

    pub(super) fn build(app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .init_resource::<Console>()
            .add_systems(Startup, absorb_game_input)
            .add_systems(PreUpdate, (toggle, track_pointer).chain())
            .add_systems(EguiPrimaryContextPass, panel);
    }

    /// Let bevy_egui clear the game's keyboard and mouse-button input while
    /// egui is using it.
    fn absorb_game_input(mut settings: ResMut<EguiGlobalSettings>) {
        settings.enable_absorb_bevy_input_system = true;
    }

    fn toggle(keyboard: Res<ButtonInput<KeyCode>>, mut console: ResMut<Console>) {
        if keyboard.just_pressed(KeyCode::Backquote) {
            console.open = !console.open;
        }
    }

    fn track_pointer(wants: Option<Res<EguiWantsInput>>, mut captured: ResMut<PointerCaptured>) {
        let busy = wants.is_some_and(|w| egui_wants_any_pointer_input(w));
        if captured.0 != busy {
            captured.0 = busy;
        }
    }

    /// A slider that edits `value` in place; true if it changed.
    fn slider(
        ui: &mut egui::Ui,
        label: &str,
        value: &mut f32,
        range: std::ops::RangeInclusive<f32>,
    ) -> bool {
        ui.add(egui::Slider::new(value, range).text(label))
            .changed()
    }

    fn panel(
        mut contexts: EguiContexts,
        mut console: ResMut<Console>,
        mut tuning: ResMut<Tuning>,
        mut ball_speed: ResMut<BallSpeed>,
    ) -> Result {
        if !console.open {
            return Ok(());
        }
        // Edit a copy and write back only on a real change.
        let mut t = tuning.clone();
        let mut speed = ball_speed.0;
        let mut save = false;
        let mut reset = false;
        egui::Window::new("Dev console")
            .default_width(320.0)
            .show(contexts.ctx_mut()?, |ui| {
                egui::CollapsingHeader::new("Ball")
                    .default_open(true)
                    .show(ui, |ui| {
                        slider(
                            ui,
                            "start speed factor",
                            &mut t.ball.speed_factor,
                            0.5..=4.0,
                        );
                        slider(ui, "ramp step", &mut t.ball.ramp_step, 0.0..=0.5);
                        slider(ui, "ramp max", &mut t.ball.ramp_max, 0.5..=5.0);
                        slider(
                            ui,
                            "min vertical share",
                            &mut t.ball.min_vertical_fraction,
                            0.05..=0.9,
                        );
                        slider(
                            ui,
                            "current ball speed (units/s)",
                            &mut speed,
                            100.0..=1500.0,
                        );
                    });
                egui::CollapsingHeader::new("Paddle")
                    .default_open(true)
                    .show(ui, |ui| {
                        let p = &mut t.paddle;
                        slider(ui, "width", &mut p.width, 60.0..=480.0);
                        slider(ui, "mass", &mut p.mass, 0.5..=20.0);
                        slider(ui, "keyboard force", &mut p.force, 1000.0..=30000.0);
                        slider(ui, "damping", &mut p.linear_damping, 0.0..=20.0);
                        slider(ui, "mouse follow gain", &mut p.follow_gain, 1.0..=60.0);
                        slider(
                            ui,
                            "mouse follow max speed",
                            &mut p.max_follow_speed,
                            500.0..=8000.0,
                        );
                        slider(
                            ui,
                            "mouse follow max gap/frame",
                            &mut p.max_gap_per_frame,
                            0.1..=1.0,
                        );
                        ui.add(egui::Slider::new(&mut t.ball.lives, 1..=9).text("starting lives"));
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    save = ui.button("Save").clicked();
                    reset = ui.button("Reset to defaults").clicked();
                });
                if !console.status.is_empty() {
                    ui.label(&console.status);
                }
            });
        if reset {
            t = Tuning::default();
            console.status = "Reset to the compiled defaults (not saved).".into();
        }
        if *tuning != t {
            *tuning = t;
        }
        if speed != ball_speed.0 {
            ball_speed.0 = speed;
        }
        if save {
            let path = FileAssetReader::get_base_path()
                .join("assets")
                .join(TUNING_PATH);
            console.status = match std::fs::write(&path, tuning_file_text(&tuning)) {
                Ok(()) => format!("Saved {}", path.display()),
                Err(e) => format!("Save failed: {e}"),
            };
            info!("dev console: {}", console.status);
        }
        Ok(())
    }
}
