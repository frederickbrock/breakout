use super::*;
use crate::menu::test_helpers::press;
use crate::test_support::*;

fn enter_level_clear(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<PlayState>>()
        .set(PlayState::LevelClear);
    app.update();
}

#[test]
fn level_clear_freezes_physics_and_p_resumes_into_it() {
    let mut app = app();
    enter_level_clear(&mut app);
    assert_eq!(play_state(&app), Some(PlayState::LevelClear));
    assert!(physics_paused(&app));

    tap(&mut app, KeyCode::KeyP);
    assert_eq!(play_state(&app), Some(PlayState::Paused));
    tap(&mut app, KeyCode::KeyP);
    assert_eq!(play_state(&app), Some(PlayState::LevelClear));
    assert!(physics_paused(&app));
}

#[test]
fn resume_button_returns_to_level_clear() {
    let mut app = app();
    enter_level_clear(&mut app);
    tap(&mut app, KeyCode::Escape);
    assert_eq!(play_state(&app), Some(PlayState::Paused));
    press(&mut app, "Resume");
    assert_eq!(play_state(&app), Some(PlayState::LevelClear));
    assert!(physics_paused(&app));
}

#[test]
fn pausing_from_playing_still_resumes_into_playing() {
    let mut app = app();
    tap(&mut app, KeyCode::Escape);
    press(&mut app, "Resume");
    assert_eq!(play_state(&app), Some(PlayState::Playing));
    assert!(!physics_paused(&app));
}
