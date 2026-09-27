//! Title screen: Start, Settings and (native only) Quit.

#[cfg(not(target_arch = "wasm32"))]
use super::ButtonActivated;
use super::{go_to, heading, menu_button, menu_list, menu_screen};
use crate::game_state::AppState;
use bevy::prelude::*;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), spawn_main_menu);
    }
}

fn spawn_main_menu(mut commands: Commands) {
    commands
        .spawn(menu_screen(AppState::MainMenu))
        .with_children(|screen| {
            screen.spawn(heading("BREAKOUT", 72.0));
            screen.spawn(menu_list()).with_children(|list| {
                list.spawn(menu_button("Start"))
                    .observe(go_to(AppState::InGame));
                list.spawn(menu_button("Settings"))
                    .observe(go_to(AppState::Settings));
                // A browser tab can't be closed by the page, so no Quit there.
                #[cfg(not(target_arch = "wasm32"))]
                list.spawn(menu_button("Quit")).observe(quit);
            });
        });
}

#[cfg(not(target_arch = "wasm32"))]
fn quit(_: On<ButtonActivated>, mut exit: MessageWriter<AppExit>) {
    exit.write(AppExit::Success);
}
