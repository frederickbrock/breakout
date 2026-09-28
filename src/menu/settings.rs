//! Settings screen: the paddle-control toggle, and Back (or Esc) to the main
//! menu. Settings are session-only.

use super::{go_to, heading, menu_button, menu_list, menu_screen, ButtonActivated};
use crate::controls::ControlSettings;
use crate::game_state::AppState;
use bevy::prelude::*;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Settings), spawn_settings)
            .add_systems(Update, esc_goes_back.run_if(in_state(AppState::Settings)));
    }
}

fn spawn_settings(mut commands: Commands, settings: Res<ControlSettings>) {
    commands
        .spawn(menu_screen(AppState::Settings))
        .with_children(|screen| {
            screen.spawn(heading("Settings", 56.0));
            screen.spawn(menu_list()).with_children(|list| {
                list.spawn(menu_button(settings.paddle.label()))
                    .observe(toggle_paddle_control);
                list.spawn(menu_button("Back"))
                    .observe(go_to(AppState::MainMenu));
            });
        });
}

/// Flips Mouse/Keyboard and relabels the button that was activated.
fn toggle_paddle_control(
    on: On<ButtonActivated>,
    mut settings: ResMut<ControlSettings>,
    children: Query<&Children>,
    mut texts: Query<&mut Text>,
) {
    settings.paddle = settings.paddle.toggled();
    let label = children
        .get(on.entity)
        .ok()
        .and_then(|children| children.first().copied());
    if let Some(mut text) = label.and_then(|label| texts.get_mut(label).ok()) {
        text.0 = settings.paddle.label().to_string();
    }
}

fn esc_goes_back(keyboard: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next.set(AppState::MainMenu);
    }
}
