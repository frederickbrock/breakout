//! End-of-run screen: "GAME OVER" or "YOU WIN!", the final score, and
//! Play again / Main menu. R is kept as a Play again shortcut
//! ([`crate::run::restart_from_game_over`]).

use super::{go_to, heading, menu_button, menu_list, menu_screen, OVERLAY_DIM};
use crate::game_state::{AppState, GameOutcome};
use crate::run::Score;
use bevy::prelude::*;

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::GameOver), spawn_game_over);
    }
}

fn spawn_game_over(mut commands: Commands, outcome: Option<Res<GameOutcome>>, score: Res<Score>) {
    let title = match outcome.as_deref() {
        Some(GameOutcome::Won) => "YOU WIN!",
        Some(GameOutcome::Lost) | None => "GAME OVER",
    };
    commands
        .spawn((
            menu_screen(AppState::GameOver),
            BackgroundColor(OVERLAY_DIM),
        ))
        .with_children(|screen| {
            screen.spawn(heading(title, 64.0));
            screen.spawn(heading(&format!("Score: {}", score.0), 32.0));
            screen.spawn(menu_list()).with_children(|list| {
                list.spawn(menu_button("Play again"))
                    .observe(go_to(AppState::InGame));
                list.spawn(menu_button("Main menu"))
                    .observe(go_to(AppState::MainMenu));
            });
        });
}

#[cfg(test)]
mod tests;
