//! A run: starting it, ending it, and its score, lives and HUD.
//!
//! [`start_run`] (on entering `AppState::InGame`, i.e. first launch and every
//! restart) resets [`Score`] and [`Lives`] (to [`STARTING_LIVES`]), sets the
//! `BallSpeed` and builds the board from the campaign's first level
//! ([`CampaignLevels`] via [`campaign_level`], else the built-in random
//! board, read fresh each run so an edited level applies at the next Start),
//! resets `CurrentLevel` to 0, spawns the run's ball, paddle, bricks and HUD
//! (all scoped to the run), and broadcasts [`RestartGame`] then
//! `LevelStarted` for level 0; every other subsystem with state to reset
//! observes those instead of being reset from here. [`end_run`] inserts the `GameOutcome`
//! and switches to `GameOver`; [`restart_from_game_over`] makes R a shortcut
//! for Play again.
//!
//! The HUD is two blocks in the left side panel at [`HUD_X`]: an uppercase
//! `SCORE\n` / `LIVES\n` label line, with the value in a `TextSpan` child
//! (the [`ScoreText`] / [`LivesText`] markers sit on the span), and a
//! `SECTOR\n` block (two-digit sector number, [`SectorText`]) at the top of
//! the right panel at [`HUD_RIGHT_X`], with the power-up capsules below it.
//! [`update_hud`] keeps the values current. Each block sits on a dark backing plate
//! ([`crate::plate`], sized by [`hud_plate_rect`] to fit a
//! [`HUD_MAX_DIGITS`]-digit value) that comes and goes with the HUD.

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::ball::{anchor_position, anchored, Ball, BallApproach, BallLook, BallSpeed, BALL_SIZE};
use crate::campaign::{sector_number, spawn_board, CurrentLevel, LevelStarted};
use crate::collision::BallCollisionSignals;
use crate::game_state::{AppState, GameOutcome};
use crate::levels::{campaign_level, CampaignLevels, LevelDef};
use crate::paddle::{
    paddle_field, prong, Paddle, PADDLE_HEIGHT, PADDLE_LINEAR_DAMPING, PADDLE_MARGIN_BOTTOM,
    PADDLE_MASS, PADDLE_WIDTH,
};
use crate::plate::{BackingPlate, PLATE_PADDING, PLATE_Z};
use crate::theme;
use crate::world::{GAME_SCALE, PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH, WORLD_WIDTH};

/// Lives at the start of every run, including the first.
pub(crate) const STARTING_LIVES: i32 = 3;
pub(crate) const HUD_FONT_SIZE: f32 = 24.0 * GAME_SCALE;
/// Inset of the HUD from the world's left and top edges.
pub(crate) const HUD_MARGIN: f32 = 30.0;
/// Vertical distance between the tops of the SCORE and LIVES blocks.
pub(crate) const HUD_BLOCK_SPACING: f32 = 120.0;
/// Left edge of the HUD text, in the left side panel.
pub(crate) const HUD_X: f32 = -WORLD_WIDTH / 2.0 + HUD_MARGIN;
/// Left edge of the right-panel HUD (the SECTOR block), level with the
/// capsules.
pub(crate) const HUD_RIGHT_X: f32 = PLAYFIELD_WIDTH / 2.0 + HUD_MARGIN;
/// The HUD font's advance per character (the default FiraMono: 0.6 em) and
/// its line height (Bevy's default 1.2 em).
const HUD_CHAR_WIDTH: f32 = HUD_FONT_SIZE * 0.6;
const HUD_LINE_HEIGHT: f32 = HUD_FONT_SIZE * 1.2;
/// The widest value a HUD block's plate is sized for (a 6-digit score), so
/// the plate doesn't change as the value grows.
pub(crate) const HUD_MAX_DIGITS: usize = 6;
/// Characters in the longest HUD label (`SECTOR`; `SCORE` and `LIVES` are 5).
const HUD_LABEL_CHARS: usize = 6;

#[derive(Component)]
pub(crate) struct ScoreText;

#[derive(Component)]
pub(crate) struct LivesText;

/// The sector number's span in the right-panel HUD block.
#[derive(Component)]
pub(crate) struct SectorText;

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

/// Starts a fresh run: resets the counters this module owns and
/// [`CurrentLevel`] to the first level, spawns the run's entities and the
/// first level's board (via [`campaign_level`]: [`LevelDef::fallback`] when
/// there is no campaign; all scoped to [`AppState::InGame`], so leaving the
/// run despawns them), then broadcasts [`RestartGame`] and
/// [`LevelStarted`]` { index: 0 }` for every other subsystem.
pub(crate) fn start_run(
    mut commands: Commands,
    (mut score, mut lives): (ResMut<Score>, ResMut<Lives>),
    mut current: ResMut<CurrentLevel>,
    mut signals: ResMut<BallCollisionSignals>,
    ball_look: Res<BallLook>,
    campaign: Option<Res<CampaignLevels>>,
    mut ball_speed: ResMut<BallSpeed>,
) {
    score.0 = 0;
    lives.0 = STARTING_LIVES;
    current.0 = 0;
    *signals = BallCollisionSignals::default();
    let def = campaign_level(campaign.as_deref(), 0).unwrap_or_else(LevelDef::fallback);
    spawn_run_entities(&mut commands, &ball_look);
    spawn_board(&mut commands, &def, 1, &mut ball_speed);
    commands.trigger(RestartGame);
    commands.trigger(LevelStarted { index: 0 });
}

/// The run's paddle, ball and HUD (the board is spawned separately, per
/// level).
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

    spawn_hud_line(
        commands,
        "SCORE\n",
        "0",
        HUD_X,
        PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN,
        ScoreText,
    );
    spawn_hud_line(
        commands,
        "LIVES\n",
        &STARTING_LIVES.to_string(),
        HUD_X,
        PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN - HUD_BLOCK_SPACING,
        LivesText,
    );
    spawn_hud_line(
        commands,
        "SECTOR\n",
        &sector_number(0),
        HUD_RIGHT_X,
        PLAYFIELD_HEIGHT / 2.0 - HUD_MARGIN,
        SectorText,
    );
}

/// The text area of the HUD block whose top-left is at (`x`, `y`): the
/// label line and the value line, as wide as the wider of the label and a
/// [`HUD_MAX_DIGITS`]-digit value.
pub(crate) fn hud_text_rect(x: f32, y: f32) -> Rect {
    let chars = HUD_LABEL_CHARS.max(HUD_MAX_DIGITS) as f32;
    let size = Vec2::new(chars * HUD_CHAR_WIDTH, 2.0 * HUD_LINE_HEIGHT);
    Rect::new(x, y - size.y, x + size.x, y)
}

/// The backing plate of the HUD block at (`x`, `y`): its text area plus
/// padding.
pub(crate) fn hud_plate_rect(x: f32, y: f32) -> Rect {
    hud_text_rect(x, y).inflate(PLATE_PADDING)
}

/// A HUD block in a side panel: an uppercase label line in the label colour
/// with the value span in ink on the line below, on a dark backing plate
/// ([`hud_plate_rect`]). Top-left anchored at (`x`, `y`). `marker` goes on
/// the value span, which [`update_hud`] writes.
pub(crate) fn spawn_hud_line(
    commands: &mut Commands,
    label: &str,
    value: &str,
    x: f32,
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
        Transform::from_xyz(x, y, 1.0),
        children![(TextSpan::new(value), font, TextColor(theme::INK), marker)],
    ));
    let plate = hud_plate_rect(x, y);
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        BackingPlate { size: plate.size() },
        Transform::from_translation(plate.center().extend(PLATE_Z)),
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

/// The HUD value span marked `M` (and not the other two markers, so the
/// three mutable queries are disjoint).
type HudSpan<M, A, B> = (With<M>, Without<A>, Without<B>);

pub(crate) fn update_hud(
    (score, lives, current): (Res<Score>, Res<Lives>, Res<CurrentLevel>),
    mut score_text: Query<&mut TextSpan, HudSpan<ScoreText, LivesText, SectorText>>,
    mut lives_text: Query<&mut TextSpan, HudSpan<LivesText, ScoreText, SectorText>>,
    mut sector_text: Query<&mut TextSpan, HudSpan<SectorText, ScoreText, LivesText>>,
) {
    if let Ok(mut text) = score_text.single_mut() {
        text.0 = score.0.to_string();
    }
    if let Ok(mut text) = lives_text.single_mut() {
        text.0 = lives.0.to_string();
    }
    if let Ok(mut text) = sector_text.single_mut() {
        let sector = sector_number(current.0);
        if text.0 != sector {
            text.0 = sector;
        }
    }
}

#[cfg(test)]
mod tests;
