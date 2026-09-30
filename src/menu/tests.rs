use super::test_helpers::*;
use super::*;
use crate::ball::Ball;
use crate::bricks::grid::Brick;
use crate::game_state::{AppState, PlayState};
use crate::run::{Lives, Score, STARTING_LIVES};
use crate::test_support::*;

#[test]
fn launch_shows_the_main_menu_with_no_run_behind_it() {
    let mut app = launch();
    app.update();

    assert_eq!(app_state(&app), AppState::MainMenu);
    assert!(physics_paused(&app));
    assert_eq!(count::<With<Ball>>(&mut app), 0);
    assert_eq!(count::<With<Brick>>(&mut app), 0);
    assert!(texts(&mut app).contains(&"BREAKOUT".to_string()));
    assert_eq!(button_labels(&mut app), ["Start", "Settings", "Quit"]);
    assert_eq!(focused_label(&mut app), "Start");
}

#[test]
fn up_down_and_w_s_move_the_focus_and_wrap() {
    let mut app = launch();

    tap(&mut app, KeyCode::ArrowDown);
    assert_eq!(focused_label(&mut app), "Settings");
    tap(&mut app, KeyCode::KeyS);
    assert_eq!(focused_label(&mut app), "Quit");
    tap(&mut app, KeyCode::ArrowDown);
    assert_eq!(focused_label(&mut app), "Start");
    tap(&mut app, KeyCode::ArrowUp);
    assert_eq!(focused_label(&mut app), "Quit");
    tap(&mut app, KeyCode::KeyW);
    assert_eq!(focused_label(&mut app), "Settings");
}

#[test]
fn focus_is_visible() {
    let mut app = launch();
    let start = button(&mut app, "Start");
    let settings = button(&mut app, "Settings");
    let border = |app: &App, e| app.world().get::<BorderColor>(e).unwrap().top;

    assert_eq!(border(&app, start), BORDER_FOCUSED);
    assert_eq!(border(&app, settings), BORDER_NORMAL);
    tap(&mut app, KeyCode::ArrowDown);
    assert_eq!(border(&app, start), BORDER_NORMAL);
    assert_eq!(border(&app, settings), BORDER_FOCUSED);
}

#[test]
fn enter_on_start_begins_a_fresh_run() {
    let mut app = launch();
    tap(&mut app, KeyCode::Enter);

    assert_eq!(app_state(&app), AppState::InGame);
    assert_eq!(play_state(&app), Some(PlayState::Playing));
    assert!(!physics_paused(&app));
    assert_eq!(app.world().resource::<Score>().0, 0);
    assert_eq!(app.world().resource::<Lives>().0, STARTING_LIVES);
    assert_eq!(count::<With<Ball>>(&mut app), 1);
    assert_eq!(count::<With<MenuButton>>(&mut app), 0);
}

#[test]
fn clicking_start_begins_a_run() {
    let mut app = launch();
    set_interaction(&mut app, "Start", Interaction::Pressed);
    app.update();

    assert_eq!(app_state(&app), AppState::InGame);
    assert_eq!(count::<With<Ball>>(&mut app), 1);
}

#[test]
fn hovering_a_button_changes_its_look_and_focuses_it() {
    let mut app = launch();
    let settings = button(&mut app, "Settings");
    let background = |app: &App| app.world().get::<BackgroundColor>(settings).unwrap().0;
    assert_eq!(background(&app), BUTTON_NORMAL);

    set_interaction(&mut app, "Settings", Interaction::Hovered);
    app.update();
    assert_eq!(background(&app), BUTTON_HOVERED);
    assert_eq!(focused_label(&mut app), "Settings");

    *app.world_mut().get_mut::<Interaction>(settings).unwrap() = Interaction::Pressed;
    app.update();
    assert_eq!(background(&app), BUTTON_PRESSED);
}

#[test]
fn settings_screen_and_back_via_button_or_esc() {
    let mut app = launch();
    set_interaction(&mut app, "Settings", Interaction::Pressed);
    app.update();

    assert_eq!(app_state(&app), AppState::Settings);
    assert!(physics_paused(&app));
    let shown = texts(&mut app);
    assert!(shown.contains(&"Settings".to_string()));
    assert_eq!(button_labels(&mut app), ["Paddle control: Mouse", "Back"]);

    tap(&mut app, KeyCode::Escape);
    assert_eq!(app_state(&app), AppState::MainMenu);
    assert_eq!(button_labels(&mut app), ["Start", "Settings", "Quit"]);

    tap(&mut app, KeyCode::KeyS);
    tap(&mut app, KeyCode::Space);
    assert_eq!(app_state(&app), AppState::Settings);
    tap(&mut app, KeyCode::ArrowDown);
    tap(&mut app, KeyCode::Enter);
    assert_eq!(app_state(&app), AppState::MainMenu);
    assert_eq!(count::<With<Ball>>(&mut app), 0);
}

#[test]
fn the_paddle_control_button_toggles_mouse_and_keyboard() {
    use crate::controls::{ControlSettings, PaddleControl};
    let paddle = |app: &App| app.world().resource::<ControlSettings>().paddle;
    let mut app = launch();
    press(&mut app, "Settings");
    assert_eq!(paddle(&app), PaddleControl::Mouse);
    assert_eq!(focused_label(&mut app), "Paddle control: Mouse");

    tap(&mut app, KeyCode::Enter);
    assert_eq!(paddle(&app), PaddleControl::Keyboard);
    assert_eq!(
        button_labels(&mut app),
        ["Paddle control: Keyboard", "Back"]
    );
    assert_eq!(app_state(&app), AppState::Settings);

    press(&mut app, "Paddle control: Keyboard");
    assert_eq!(paddle(&app), PaddleControl::Mouse);
    assert_eq!(button_labels(&mut app), ["Paddle control: Mouse", "Back"]);

    // The choice survives leaving and re-entering Settings.
    tap(&mut app, KeyCode::Space);
    tap(&mut app, KeyCode::Escape);
    press(&mut app, "Settings");
    assert_eq!(
        button_labels(&mut app),
        ["Paddle control: Keyboard", "Back"]
    );
}

#[test]
fn long_button_labels_widen_the_button_instead_of_wrapping() {
    let mut app = launch();
    press(&mut app, "Settings");
    let toggle = button(&mut app, "Paddle control: Mouse");
    let world = app.world();

    let node = world.get::<Node>(toggle).unwrap();
    assert_eq!(node.width, Val::Auto, "sized by its label, not fixed");
    assert_eq!(node.min_width, px(BUTTON_MIN_WIDTH));
    let label = world.get::<Children>(toggle).unwrap()[0];
    assert_eq!(
        world.get::<TextLayout>(label).unwrap().linebreak,
        bevy::text::LineBreak::NoWrap
    );

    // Every button in the column stretches to the widest one.
    let list = world.get::<ChildOf>(toggle).unwrap().parent();
    assert_eq!(
        world.get::<Node>(list).unwrap().align_items,
        AlignItems::Stretch
    );
}

#[test]
fn quit_exits_the_app() {
    let mut app = launch();
    assert!(app.should_exit().is_none());
    tap(&mut app, KeyCode::ArrowUp);
    assert_eq!(focused_label(&mut app), "Quit");
    tap(&mut app, KeyCode::Enter);
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}
