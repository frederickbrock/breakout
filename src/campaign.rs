//! Campaign progression: a run plays the campaign's levels in order.
//!
//! [`CurrentLevel`] is the 0-based index of the level in play (sector number
//! = index + 1); `start_run` resets it to 0. When `ball_movement` sees the
//! last brick go it triggers [`LevelCleared`], and [`on_level_cleared`]
//! decides: after the last level (see `levels::campaign_level`) it ends the
//! run as won; otherwise it puts up the `SECTOR NN // name` card, stores the
//! next level in [`LevelTransition`] and enters [`PlayState::LevelClear`],
//! where the game is frozen like `Paused` (physics stops on leaving
//! `Playing`, and every gameplay system gates on `Playing`). After
//! [`SECTOR_CARD_SECS`], [`advance_after_card`] spawns the next board, drops
//! the card and broadcasts [`LevelStarted`], which per-level subsystems
//! (power-ups, the ball's re-anchor) observe; score and lives carry over.
//!
//! The card's timer only ticks in `LevelClear`, so pausing during the card
//! freezes it, and resuming returns to the card (`game_state::PausedFrom`).
//! The card is not state-scoped to `LevelClear` (pausing would delete it):
//! it carries [`SectorCard`] and `DespawnOnExit(AppState::InGame)` and is
//! despawned when the next level starts.

use bevy::prelude::*;

use crate::ball::BallSpeed;
use crate::bricks::grid::spawn_bricks;
use crate::collision::BallCollisionSignals;
use crate::game_state::{AppState, GameOutcome, PlayState};
use crate::levels::{build_board, campaign_level, CampaignLevels, LevelDef};
use crate::menu::heading;
use crate::run::end_run;
use crate::theme;
use crate::tuning::Tuning;

/// How long the sector card stays up between levels.
pub(crate) const SECTOR_CARD_SECS: f32 = 2.0;

/// The 0-based index of the campaign level in play.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CurrentLevel(pub(crate) usize);

/// The level in play has no bricks left. Triggered by `ball_movement`;
/// [`on_level_cleared`] decides between the next level and a win.
#[derive(Event)]
pub(crate) struct LevelCleared;

/// Per-level reset broadcast: fired by `start_run` for level 0 (after
/// `RestartGame`) and by [`advance_after_card`] for each later level, after
/// the new board's spawns are queued. Unlike `RestartGame` it keeps score and
/// lives.
#[derive(Event, Clone, Copy, Debug)]
pub(crate) struct LevelStarted {
    pub(crate) index: usize,
}

/// The pending move to the next level while the sector card is up.
#[derive(Resource)]
pub(crate) struct LevelTransition {
    pub(crate) timer: Timer,
    /// The index of the level to start.
    pub(crate) next: usize,
    /// That level, copied when the previous one was cleared, so the card's
    /// name and the spawned board always agree.
    pub(crate) def: LevelDef,
}

/// The root of the between-levels sector card.
#[derive(Component)]
pub(crate) struct SectorCard;

pub struct CampaignPlugin;

impl Plugin for CampaignPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentLevel>()
            .add_observer(on_level_cleared)
            .add_observer(log_level_start)
            .add_systems(
                Update,
                advance_after_card.run_if(
                    in_state(PlayState::LevelClear).and_then(resource_exists::<LevelTransition>),
                ),
            )
            .add_systems(OnEnter(PlayState::Playing), resume_card_if_pending)
            .add_systems(OnExit(AppState::InGame), drop_transition);
    }
}

/// The two-digit sector number of the level at `index` ("01" for 0).
pub(crate) fn sector_number(index: usize) -> String {
    format!("{:02}", index + 1)
}

/// The card's text for the level at `index`: `SECTOR 02 // Second`.
pub(crate) fn sector_card_text(index: usize, name: &str) -> String {
    format!("SECTOR {} // {}", sector_number(index), name)
}

/// Sets the ball speed for `def` played as round `round` (from 1: the
/// level index + 1) and spawns its board.
pub(crate) fn spawn_board(
    commands: &mut Commands,
    def: &LevelDef,
    round: usize,
    ball_speed: &mut BallSpeed,
    tuning: &Tuning,
) {
    *ball_speed = BallSpeed::for_level(def, round, &tuning.ball);
    let board = build_board(def, &mut rand::rng(), &tuning.bricks);
    spawn_bricks(commands, &board, def.cols());
}

/// A centred panel with `text`, over the frozen game.
fn spawn_sector_card(commands: &mut Commands, text: &str) {
    commands.spawn((
        SectorCard,
        DespawnOnExit(AppState::InGame),
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Node {
                padding: UiRect::axes(px(48), px(24)),
                border: UiRect::all(px(3)),
                ..default()
            },
            BackgroundColor(theme::HUD_PLATE),
            BorderColor::all(theme::EMITTER),
            children![heading(text, 48.0)],
        )],
    ));
}

/// Ends the run as won after the last level; otherwise shows the next
/// level's sector card and enters [`PlayState::LevelClear`].
fn on_level_cleared(
    _on: On<LevelCleared>,
    mut commands: Commands,
    current: Res<CurrentLevel>,
    campaign: Option<Res<CampaignLevels>>,
    mut next_play: ResMut<NextState<PlayState>>,
    mut next_app: ResMut<NextState<AppState>>,
) {
    let next = current.0 + 1;
    match campaign_level(campaign.as_deref(), next) {
        None => end_run(&mut commands, &mut next_app, GameOutcome::Won),
        Some(def) => {
            spawn_sector_card(&mut commands, &sector_card_text(next, &def.name));
            commands.insert_resource(LevelTransition {
                timer: Timer::from_seconds(SECTOR_CARD_SECS, TimerMode::Once),
                next,
                def,
            });
            next_play.set(PlayState::LevelClear);
        }
    }
}

/// Once the card's time is up: starts the next level (board, ball speed),
/// drops the card, broadcasts [`LevelStarted`] and resumes play. Runs only
/// in [`PlayState::LevelClear`], so the timer stops while paused.
fn advance_after_card(
    mut commands: Commands,
    time: Res<Time>,
    mut transition: ResMut<LevelTransition>,
    mut current: ResMut<CurrentLevel>,
    (mut ball_speed, mut signals): (ResMut<BallSpeed>, ResMut<BallCollisionSignals>),
    cards: Query<Entity, With<SectorCard>>,
    (mut next_play, tuning): (ResMut<NextState<PlayState>>, Res<Tuning>),
) {
    if !transition.timer.tick(time.delta()).is_finished() {
        return;
    }
    current.0 = transition.next;
    for card in &cards {
        commands.entity(card).despawn();
    }
    *signals = BallCollisionSignals::default();
    // The bricks are queued before the trigger, so `LevelStarted` observers
    // see the new board.
    spawn_board(
        &mut commands,
        &transition.def,
        current.0 + 1,
        &mut ball_speed,
        &tuning,
    );
    commands.remove_resource::<LevelTransition>();
    commands.trigger(LevelStarted { index: current.0 });
    next_play.set(PlayState::Playing);
}

/// Backstop: if play resumed while a card is still pending (e.g. a pause
/// toggled in the frame the level cleared), go back to the card.
fn resume_card_if_pending(
    transition: Option<Res<LevelTransition>>,
    mut next: ResMut<NextState<PlayState>>,
) {
    if transition.is_some() {
        next.set(PlayState::LevelClear);
    }
}

fn log_level_start(on: On<LevelStarted>) {
    info!("sector {} started", sector_number(on.index));
}

/// Abandoning a run mid-card leaves nothing pending for the next one.
fn drop_transition(mut commands: Commands) {
    commands.remove_resource::<LevelTransition>();
}

#[cfg(test)]
mod tests;
