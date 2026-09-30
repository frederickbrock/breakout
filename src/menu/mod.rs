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
//!
//! The kit: [`menu_screen`] (a state-scoped full-window root), [`menu_list`]
//! (a [`MenuList`] column whose children order is the keyboard navigation
//! order), [`menu_button`], the [`Focused`] marker and the [`ButtonActivated`]
//! entity event fired on click or Enter/Space. Each button's behaviour is its
//! own `.observe(...)` (e.g. [`go_to`]`(AppState::InGame)`), so a new screen
//! is a new file with its own plugin, not an edit to a shared match.
//!
//! The screens: `main_menu` (Start / Settings / Quit, Quit native-only),
//! `settings` (the "Paddle control: Mouse/Keyboard" toggle; Back or Esc
//! returns), `pause` (Resume / Main menu over the frozen game while
//! `PlayState::Paused`) and `game_over` ("GAME OVER" or "YOU WIN!", the final
//! score, Play again / Main menu). The pause and game-over roots use
//! `OVERLAY_DIM` as background so the game shows through.

mod game_over;
mod main_menu;
mod pause;
mod settings;

use bevy::prelude::*;
use bevy::state::state::FreelyMutableState;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            main_menu::MainMenuPlugin,
            settings::SettingsPlugin,
            pause::PauseMenuPlugin,
            game_over::GameOverPlugin,
        ))
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

use crate::theme::{BORDER_FOCUSED, BORDER_NORMAL, BUTTON_HOVERED, BUTTON_NORMAL, BUTTON_PRESSED};
const BUTTON_MIN_WIDTH: f32 = 240.0;
/// Background for menus shown over the game (pause, game over).
const OVERLAY_DIM: Color = crate::theme::OVERLAY_DIM;

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

/// Container for a screen's buttons; see [`MenuList`]. Stretches every
/// button to the widest one, so a long label widens the whole column evenly
/// instead of overflowing its button.
pub fn menu_list() -> impl Bundle {
    (
        MenuList,
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
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
        TextColor(crate::theme::INK),
    )
}

/// A button labelled `label`. Spawn it as a child of a [`menu_list`] and
/// `.observe(|_: On<ButtonActivated>, ...| ...)` for what it does.
pub fn menu_button(label: &str) -> impl Bundle {
    (
        MenuButton,
        Node {
            // At least 240 px; wider when the label needs it (the label never
            // wraps, see below), with some room either side of the text.
            min_width: px(BUTTON_MIN_WIDTH),
            height: px(56),
            padding: UiRect::horizontal(px(24)),
            border: UiRect::all(px(3)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(BUTTON_NORMAL),
        BorderColor::all(BORDER_NORMAL),
        children![(heading(label, 28.0), TextLayout::no_wrap())],
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

/// Finding and driving on-screen menu buttons from headless tests.
#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;

    /// Labels of the on-screen menu buttons, in navigation order.
    pub(crate) fn button_labels(app: &mut App) -> Vec<String> {
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

    pub(crate) fn button(app: &mut App, label: &str) -> Entity {
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

    pub(crate) fn focused_label(app: &mut App) -> String {
        let world = app.world_mut();
        let focused = world
            .query_filtered::<&Children, With<Focused>>()
            .single(world)
            .expect("exactly one focused button")[0];
        world.entity(focused).get::<Text>().unwrap().0.clone()
    }

    pub(crate) fn texts(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        world
            .query::<&Text>()
            .iter(world)
            .map(|t| t.0.clone())
            .collect()
    }

    pub(crate) fn set_interaction(app: &mut App, label: &str, interaction: Interaction) {
        let entity = button(app, label);
        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = interaction;
        app.update();
    }

    /// Clicks the button labelled `label` and lets the resulting state
    /// transition happen.
    pub(crate) fn press(app: &mut App, label: &str) {
        set_interaction(app, label, Interaction::Pressed);
        app.update();
    }
}

#[cfg(test)]
mod tests;
