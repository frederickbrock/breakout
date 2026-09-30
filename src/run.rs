//! A run: starting it, ending it, and its score, lives and HUD.
//!
//! [`start_run`] (on entering `AppState::InGame`, i.e. first launch and every
//! restart) resets [`Score`] and [`Lives`] (to [`STARTING_LIVES`]), spawns the
//! run's ball, paddle, bricks and HUD (all scoped to the run), and broadcasts
//! [`RestartGame`]; every other subsystem with state to reset observes that
//! instead of being reset from here. [`end_run`] inserts the `GameOutcome`
//! and switches to `GameOver`; [`restart_from_game_over`] makes R a shortcut
//! for Play again.
//!
//! The HUD is two blocks in the left side panel at [`HUD_X`]: an uppercase
//! `SCORE\n` / `LIVES\n` label line, with the value in a `TextSpan` child
//! (the [`ScoreText`] / [`LivesText`] markers sit on the span), kept current
//! by [`update_hud`].

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::ball::{anchor_position, anchored, Ball, BallApproach, BallLook, BALL_SIZE};
use crate::bricks::grid::spawn_bricks;
use crate::collision::BallCollisionSignals;
use crate::game_state::{AppState, GameOutcome};
use crate::paddle::{
    paddle_field, prong, Paddle, PADDLE_HEIGHT, PADDLE_LINEAR_DAMPING, PADDLE_MARGIN_BOTTOM,
    PADDLE_MASS, PADDLE_WIDTH,
};
use crate::theme;
use crate::world::{GAME_SCALE, PLAYFIELD_HEIGHT, WORLD_WIDTH};

/// Lives at the start of every run, including the first.
pub(crate) const STARTING_LIVES: i32 = 3;
pub(crate) const HUD_FONT_SIZE: f32 = 24.0 * GAME_SCALE;
/// Inset of the HUD from the world's left and top edges.
pub(crate) const HUD_MARGIN: f32 = 30.0;
/// Vertical distance between the tops of the SCORE and LIVES blocks.
pub(crate) const HUD_BLOCK_SPACING: f32 = 120.0;
/// Left edge of the HUD text, in the left side panel.
pub(crate) const HUD_X: f32 = -WORLD_WIDTH / 2.0 + HUD_MARGIN;

#[derive(Component)]
pub(crate) struct ScoreText;

#[derive(Component)]
pub(crate) struct LivesText;

#[derive(Resource, Default)]
pub(crate) struct Score(pub(crate) i32);

#[derive(Resource, Default)]
pub(crate) struct Lives(pub(crate) i32);

/// Broadcast at the start of every run (entering [`AppState::InGame`]). Each
/// subsystem that has its own state to reset (currently just power-ups)
/// registers an observer on this instead of `start_run` reaching into every
/// subsystem by hand — a future obstacles or brick-respawn subsystem resets
/// itself the same way, with no changes needed here.
#[derive(Event)]
pub(crate) struct RestartGame;

/// Starts a fresh run: resets the counters this module owns, spawns the
/// run's entities (all scoped to [`AppState::InGame`], so leaving the run
/// despawns them), and broadcasts [`RestartGame`] for every other subsystem.
pub(crate) fn start_run(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut lives: ResMut<Lives>,
    mut signals: ResMut<BallCollisionSignals>,
    ball_look: Res<BallLook>,
) {
    score.0 = 0;
    lives.0 = STARTING_LIVES;
    *signals = BallCollisionSignals::default();
    spawn_run_entities(&mut commands, &ball_look);
    commands.trigger(RestartGame);
}

pub(crate) fn spawn_run_entities(commands: &mut Commands, ball_look: &BallLook) {
    let paddle_start = Vec3::new(
        0.0,
        -PLAYFIELD_HEIGHT / 2.0 + PADDLE_HEIGHT / 2.0 + PADDLE_MARGIN_BOTTOM,
        0.0,
    );
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        // No sprite of its own: it's drawn by its three children.
        Visibility::default(),
        Transform::from_translation(paddle_start),
        RigidBody::Dynamic,
        Collider::rectangle(PADDLE_WIDTH, PADDLE_HEIGHT),
        Mass(PADDLE_MASS),
        LockedAxes::new().lock_translation_y().lock_rotation(),
        LinearDamping(PADDLE_LINEAR_DAMPING),
        Restitution::ZERO,
        ConstantForce(Vec2::ZERO),
        Paddle {
            width: PADDLE_WIDTH,
        },
        children![prong(-1.0), paddle_field(), prong(1.0)],
    ));

    commands.spawn((
        Mesh2d(ball_look.mesh.clone()),
        MeshMaterial2d(ball_look.material.clone()),
        Transform::from_translation(anchor_position(paddle_start)),
        RigidBody::Dynamic,
        Collider::circle(BALL_SIZE / 2.0),
        LinearVelocity::ZERO,
        anchored(),
        LockedAxes::ROTATION_LOCKED,
        Restitution::new(1.0),
        Friction::ZERO,
        CollisionEventsEnabled,
        Ball,
        BallApproach::default(),
        DespawnOnExit(AppState::InGame),
    ));

    spawn_bricks(commands);

    spawn_hud_line(
        commands,
        "SCORE\n",
        "0",
        PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN,
        ScoreText,
    );
    spawn_hud_line(
        commands,
        "LIVES\n",
        &STARTING_LIVES.to_string(),
        PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN - HUD_BLOCK_SPACING,
        LivesText,
    );
}

/// A HUD block in the left side panel: an uppercase label line in the label
/// colour with the value span in ink on the line below. Top-left anchored at
/// [`HUD_X`], `y`. `marker` goes on the value span, which [`update_hud`] writes.
pub(crate) fn spawn_hud_line(
    commands: &mut Commands,
    label: &str,
    value: &str,
    y: f32,
    marker: impl Component,
) {
    let font = TextFont {
        font_size: FontSize::Px(HUD_FONT_SIZE),
        ..default()
    };
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Text2d::new(label),
        font.clone(),
        TextColor(theme::LABEL),
        Anchor::TOP_LEFT,
        Transform::from_xyz(HUD_X, y, 1.0),
        children![(TextSpan::new(value), font, TextColor(theme::INK), marker)],
    ));
}

pub(crate) fn end_run(
    commands: &mut Commands,
    next_state: &mut NextState<AppState>,
    outcome: GameOutcome,
) {
    commands.insert_resource(outcome);
    next_state.set(AppState::GameOver);
}

/// R on the game-over/win screen starts a new run (a shortcut for its Play
/// again button); entering
/// [`AppState::InGame`] runs [`start_run`], which does the actual resetting.
pub(crate) fn restart_from_game_over(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        next_state.set(AppState::InGame);
    }
}

pub(crate) fn update_hud(
    score: Res<Score>,
    lives: Res<Lives>,
    mut score_text: Query<&mut TextSpan, (With<ScoreText>, Without<LivesText>)>,
    mut lives_text: Query<&mut TextSpan, (With<LivesText>, Without<ScoreText>)>,
) {
    if let Ok(mut text) = score_text.single_mut() {
        text.0 = score.0.to_string();
    }
    if let Ok(mut text) = lives_text.single_mut() {
        text.0 = lives.0.to_string();
    }
}

#[cfg(test)]
mod tests;
