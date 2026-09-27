//! Pause menu, shown over the frozen game while [`PlayState::Paused`]:
//! Resume, or abandon the run for the main menu. P/Esc still toggle pause
//! (see `game_state::toggle_pause`).

use super::{go_to, heading, menu_button, menu_list, menu_screen, OVERLAY_DIM};
use crate::game_state::{AppState, PlayState};
use bevy::prelude::*;

pub struct PauseMenuPlugin;

impl Plugin for PauseMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(PlayState::Paused), spawn_pause_menu);
    }
}

fn spawn_pause_menu(mut commands: Commands) {
    commands
        .spawn((menu_screen(PlayState::Paused), BackgroundColor(OVERLAY_DIM)))
        .with_children(|screen| {
            screen.spawn(heading("Paused", 56.0));
            screen.spawn(menu_list()).with_children(|list| {
                list.spawn(menu_button("Resume"))
                    .observe(go_to(PlayState::Playing));
                // Leaving `InGame` despawns the run's entities; Start from the
                // main menu then begins a fresh run via `start_run`.
                list.spawn(menu_button("Main menu"))
                    .observe(go_to(AppState::MainMenu));
            });
        });
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use crate::game_state::{AppState, PlayState};
    use crate::test_support::*;
    use crate::{Ball, Brick, Lives, Score, STARTING_LIVES};
    use bevy::prelude::*;

    fn ball_position(app: &mut App) -> Vec3 {
        let world = app.world_mut();
        world
            .query_filtered::<&Transform, With<Ball>>()
            .single(world)
            .expect("a run has exactly one ball")
            .translation
    }

    #[test]
    fn pausing_shows_the_pause_menu_and_p_or_esc_resumes() {
        let mut app = app();
        assert!(button_labels(&mut app).is_empty());

        tap(&mut app, KeyCode::Escape);
        assert_eq!(play_state(&app), Some(PlayState::Paused));
        assert!(physics_paused(&app));
        assert!(texts(&mut app).contains(&"Paused".to_string()));
        assert_eq!(button_labels(&mut app), ["Resume", "Main menu"]);
        assert_eq!(focused_label(&mut app), "Resume");

        tap(&mut app, KeyCode::KeyP);
        assert_eq!(play_state(&app), Some(PlayState::Playing));
        assert!(!physics_paused(&app));
        assert!(button_labels(&mut app).is_empty());
    }

    #[test]
    fn resume_continues_the_run_where_it_was() {
        let mut app = app();
        app.world_mut().resource_mut::<Score>().0 = 40;
        app.world_mut().resource_mut::<Lives>().0 = 2;
        tap(&mut app, KeyCode::KeyP);
        let frozen = ball_position(&mut app);
        app.update();
        app.update();
        assert_eq!(ball_position(&mut app), frozen);

        tap(&mut app, KeyCode::Enter);
        assert_eq!(play_state(&app), Some(PlayState::Playing));
        assert_eq!(ball_position(&mut app), frozen);
        assert_eq!(app.world().resource::<Score>().0, 40);
        assert_eq!(app.world().resource::<Lives>().0, 2);

        tap(&mut app, KeyCode::KeyP);
        press(&mut app, "Resume");
        assert_eq!(play_state(&app), Some(PlayState::Playing));
    }

    #[test]
    fn main_menu_abandons_the_run_and_start_begins_fresh() {
        let mut app = app();
        app.world_mut().resource_mut::<Score>().0 = 40;
        app.world_mut().resource_mut::<Lives>().0 = 1;
        let first_brick = app
            .world_mut()
            .query_filtered::<Entity, With<Brick>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut().despawn(first_brick);

        tap(&mut app, KeyCode::Escape);
        press(&mut app, "Main menu");
        assert_eq!(app_state(&app), AppState::MainMenu);
        assert_eq!(play_state(&app), None);
        assert!(physics_paused(&app));
        assert_eq!(count::<With<Ball>>(&mut app), 0);
        assert_eq!(count::<With<Brick>>(&mut app), 0);
        assert_eq!(button_labels(&mut app), ["Start", "Settings", "Quit"]);

        press(&mut app, "Start");
        assert_eq!(app_state(&app), AppState::InGame);
        assert_eq!(play_state(&app), Some(PlayState::Playing));
        assert_eq!(app.world().resource::<Score>().0, 0);
        assert_eq!(app.world().resource::<Lives>().0, STARTING_LIVES);
        assert_eq!(
            count::<With<Brick>>(&mut app),
            crate::BRICK_ROWS * crate::BRICK_COLS
        );
        assert!(button_labels(&mut app).is_empty());
    }
}
