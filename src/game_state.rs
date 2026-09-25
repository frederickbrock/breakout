//! The game's state machine: which screen we're on ([`AppState`]) and, while
//! in a run, whether it's paused ([`PlayState`]).
//!
//! Gameplay systems gate themselves with `run_if(in_state(PlayState::Playing))`
//! rather than checking a status resource by hand, and entities that belong to
//! a run carry `DespawnOnExit(AppState::InGame)` so leaving the run cleans them
//! up. This module also owns Avian's physics clock: it runs only while
//! `InGame/Playing`, so pausing or ending the game freezes every rigid body
//! without anyone zeroing velocities.

use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    /// Title screen. Nothing enters it yet — the main menu (sim-v0i.2) will
    /// make it the initial state.
    MainMenu,
    #[default]
    InGame,
    GameOver,
}

/// Only exists while [`AppState::InGame`]; every new run starts `Playing`.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::InGame)]
pub enum PlayState {
    #[default]
    Playing,
    Paused,
}

/// How the last run ended. Inserted right before switching to
/// [`AppState::GameOver`], so it's always present in that state.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameOutcome {
    Won,
    Lost,
}

pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<PlayState>()
            // Every non-playing state is reached either by leaving `Playing`
            // or as the app's initial state, so these cover all of them
            // regardless of which state the app launches into.
            .add_systems(OnEnter(PlayState::Playing), resume_physics)
            .add_systems(OnExit(PlayState::Playing), pause_physics)
            .add_systems(OnEnter(AppState::MainMenu), pause_physics)
            .add_systems(OnEnter(AppState::GameOver), pause_physics)
            .add_systems(Update, toggle_pause.run_if(in_state(AppState::InGame)));
    }
}

fn pause_physics(mut time: ResMut<Time<Physics>>) {
    time.pause();
}

fn resume_physics(mut time: ResMut<Time<Physics>>) {
    time.unpause();
}

fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    state: Res<State<PlayState>>,
    mut next: ResMut<NextState<PlayState>>,
) {
    if !keyboard.any_just_pressed([KeyCode::KeyP, KeyCode::Escape]) {
        return;
    }
    next.set(match state.get() {
        PlayState::Playing => PlayState::Paused,
        PlayState::Paused => PlayState::Playing,
    });
}
