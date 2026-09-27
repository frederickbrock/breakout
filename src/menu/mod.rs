//! Menu screens and the small, reusable widget kit they're built from.
//!
//! The kit (this file) knows nothing about specific screens: a screen spawns
//! a [`MenuList`] whose children are [`menu_button`]s, attaches an observer
//! for [`ButtonActivated`] to each button, and gets mouse hover/press looks,
//! a keyboard focus highlight (Up/Down or W/S, wrapping) and activation
//! (click, Enter or Space) for free. There is no central match over button
//! kinds — each button carries its own behaviour as an observer.
//!
//! Only one menu is expected on screen at a time; each screen's root carries
//! `DespawnOnExit(<its state>)` so it disappears when its state is left.

mod main_menu;
mod settings;

use bevy::prelude::*;
use bevy::state::state::FreelyMutableState;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((main_menu::MainMenuPlugin, settings::SettingsPlugin))
            .add_systems(
                Update,
                (
                    focus_first_button,
                    hover_moves_focus,
                    keyboard_navigation,
                    activate_buttons,
                    update_button_colors,
                )
                    .chain(),
            );
    }
}

/// A vertical column of [`MenuButton`]s. Its children's order is the
/// keyboard navigation order.
#[derive(Component)]
pub struct MenuList;

#[derive(Component)]
#[require(Button)]
pub struct MenuButton;

/// The button keyboard navigation currently points at (at most one).
#[derive(Component)]
pub struct Focused;

/// Fired on a [`MenuButton`] when it's clicked, or when Enter/Space is
/// pressed while it's focused. Attach behaviour with `.observe(...)`.
#[derive(EntityEvent)]
pub struct ButtonActivated {
    pub entity: Entity,
}

const BUTTON_NORMAL: Color = Color::srgb(0.15, 0.15, 0.2);
const BUTTON_HOVERED: Color = Color::srgb(0.25, 0.25, 0.35);
const BUTTON_PRESSED: Color = Color::srgb(0.1, 0.35, 0.15);
const BORDER_NORMAL: Color = Color::srgb(0.3, 0.3, 0.35);
const BORDER_FOCUSED: Color = Color::srgb(1.0, 0.85, 0.2);

/// Full-window, centred column that lives only while in `state`.
pub fn menu_screen<S: States>(state: S) -> impl Bundle {
    (
        DespawnOnExit(state),
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(24),
            ..default()
        },
    )
}

/// Container for a screen's buttons; see [`MenuList`].
pub fn menu_list() -> impl Bundle {
    (
        MenuList,
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(12),
            ..default()
        },
    )
}

pub fn heading(text: &str, size: f32) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(Color::WHITE),
    )
}

/// A button labelled `label`. Spawn it as a child of a [`menu_list`] and
/// `.observe(|_: On<ButtonActivated>, ...| ...)` for what it does.
pub fn menu_button(label: &str) -> impl Bundle {
    (
        MenuButton,
        Node {
            width: px(240),
            height: px(56),
            border: UiRect::all(px(3)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(BUTTON_NORMAL),
        BorderColor::all(BORDER_NORMAL),
        children![heading(label, 28.0)],
    )
}

/// Observer for a button that switches to `state` when activated, e.g.
/// `.observe(go_to(AppState::InGame))`.
pub fn go_to<S: FreelyMutableState + Copy>(
    state: S,
) -> impl Fn(On<ButtonActivated>, ResMut<NextState<S>>) {
    move |_, mut next| next.set(state)
}

/// Menu buttons whose [`Interaction`] changed this frame.
type ChangedButtons = (Changed<Interaction>, With<MenuButton>);

/// The buttons of `list` in navigation order.
fn buttons_of(list: &Children, buttons: &Query<(), With<MenuButton>>) -> Vec<Entity> {
    list.iter().filter(|e| buttons.contains(*e)).collect()
}

/// A freshly spawned menu starts with its first button focused.
fn focus_first_button(
    mut commands: Commands,
    focused: Query<(), With<Focused>>,
    lists: Query<&Children, With<MenuList>>,
    buttons: Query<(), With<MenuButton>>,
) {
    if !focused.is_empty() {
        return;
    }
    let first = lists
        .iter()
        .find_map(|list| buttons_of(list, &buttons).first().copied());
    if let Some(first) = first {
        commands.entity(first).insert(Focused);
    }
}

/// Pointing at a button also focuses it, so there's only ever one highlight
/// and Enter activates what the mouse is on.
fn hover_moves_focus(
    mut commands: Commands,
    hovered: Query<(Entity, &Interaction), ChangedButtons>,
    focused: Query<Entity, With<Focused>>,
) {
    for (entity, interaction) in &hovered {
        if *interaction == Interaction::None {
            continue;
        }
        for old in &focused {
            if old != entity {
                commands.entity(old).remove::<Focused>();
            }
        }
        commands.entity(entity).insert(Focused);
    }
}

fn keyboard_navigation(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    focused: Query<(Entity, &ChildOf), With<Focused>>,
    lists: Query<&Children, With<MenuList>>,
    buttons: Query<(), With<MenuButton>>,
) {
    let step: isize = if keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) {
        -1
    } else if keyboard.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) {
        1
    } else {
        return;
    };
    let Some((current, parent)) = focused.iter().next() else {
        return;
    };
    let Ok(list) = lists.get(parent.parent()) else {
        return;
    };
    let order = buttons_of(list, &buttons);
    let Some(index) = order.iter().position(|e| *e == current) else {
        return;
    };
    let next = order[(index as isize + step).rem_euclid(order.len() as isize) as usize];
    if next != current {
        commands.entity(current).remove::<Focused>();
        commands.entity(next).insert(Focused);
    }
}

fn activate_buttons(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    pressed: Query<(Entity, &Interaction), ChangedButtons>,
    focused: Query<Entity, (With<Focused>, With<MenuButton>)>,
) {
    for (entity, interaction) in &pressed {
        if *interaction == Interaction::Pressed {
            commands.trigger(ButtonActivated { entity });
            return;
        }
    }
    if keyboard.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]) {
        if let Some(entity) = focused.iter().next() {
            commands.trigger(ButtonActivated { entity });
        }
    }
}

fn update_button_colors(
    mut buttons: Query<
        (
            &Interaction,
            Has<Focused>,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<MenuButton>,
    >,
) {
    for (interaction, focused, mut background, mut border) in &mut buttons {
        let bg = match interaction {
            Interaction::Pressed => BUTTON_PRESSED,
            Interaction::Hovered => BUTTON_HOVERED,
            Interaction::None if focused => BUTTON_HOVERED,
            Interaction::None => BUTTON_NORMAL,
        };
        let edge = if focused {
            BORDER_FOCUSED
        } else {
            BORDER_NORMAL
        };
        background.set_if_neq(BackgroundColor(bg));
        border.set_if_neq(BorderColor::all(edge));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::{AppState, PlayState};
    use crate::test_support::*;
    use crate::{Ball, Brick, Lives, Score, STARTING_LIVES};

    /// Labels of the on-screen menu buttons, in navigation order.
    fn button_labels(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        let lists: Vec<Entity> = world
            .query_filtered::<Entity, With<MenuList>>()
            .iter(world)
            .collect();
        let mut labels = Vec::new();
        for list in lists {
            let buttons: Vec<Entity> = world.entity(list).get::<Children>().unwrap().to_vec();
            for button in buttons {
                let label = world.entity(button).get::<Children>().unwrap()[0];
                labels.push(world.entity(label).get::<Text>().unwrap().0.clone());
            }
        }
        labels
    }

    fn button(app: &mut App, label: &str) -> Entity {
        let world = app.world_mut();
        let buttons: Vec<(Entity, Entity)> = world
            .query_filtered::<(Entity, &Children), With<MenuButton>>()
            .iter(world)
            .map(|(e, children)| (e, children[0]))
            .collect();
        buttons
            .into_iter()
            .find(|(_, text)| world.entity(*text).get::<Text>().unwrap().0 == label)
            .map(|(e, _)| e)
            .unwrap_or_else(|| panic!("no {label} button on screen"))
    }

    fn focused_label(app: &mut App) -> String {
        let world = app.world_mut();
        let focused = world
            .query_filtered::<&Children, With<Focused>>()
            .single(world)
            .expect("exactly one focused button")[0];
        world.entity(focused).get::<Text>().unwrap().0.clone()
    }

    fn texts(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        world
            .query::<&Text>()
            .iter(world)
            .map(|t| t.0.clone())
            .collect()
    }

    fn set_interaction(app: &mut App, label: &str, interaction: Interaction) {
        let entity = button(app, label);
        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = interaction;
        app.update();
    }

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
        assert!(shown.contains(&"Coming soon".to_string()));
        assert_eq!(button_labels(&mut app), ["Back"]);

        tap(&mut app, KeyCode::Escape);
        assert_eq!(app_state(&app), AppState::MainMenu);
        assert_eq!(button_labels(&mut app), ["Start", "Settings", "Quit"]);

        tap(&mut app, KeyCode::KeyS);
        tap(&mut app, KeyCode::Space);
        assert_eq!(app_state(&app), AppState::Settings);
        tap(&mut app, KeyCode::Enter);
        assert_eq!(app_state(&app), AppState::MainMenu);
        assert_eq!(count::<With<Ball>>(&mut app), 0);
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
}
