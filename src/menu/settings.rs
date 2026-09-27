//! Settings screen. Placeholder for now: a heading, "Coming soon" and Back
//! (or Esc) to the main menu.

use super::{go_to, heading, menu_button, menu_list, menu_screen};
use crate::game_state::AppState;
use bevy::prelude::*;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Settings), spawn_settings)
            .add_systems(Update, esc_goes_back.run_if(in_state(AppState::Settings)));
    }
}

fn spawn_settings(mut commands: Commands) {
    commands
        .spawn(menu_screen(AppState::Settings))
        .with_children(|screen| {
            screen.spawn(heading("Settings", 56.0));
            screen.spawn(heading("Coming soon", 28.0));
            screen.spawn(menu_list()).with_children(|list| {
                list.spawn(menu_button("Back"))
                    .observe(go_to(AppState::MainMenu));
            });
        });
}

fn esc_goes_back(keyboard: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next.set(AppState::MainMenu);
    }
}
