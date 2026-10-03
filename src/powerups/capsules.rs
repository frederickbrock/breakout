//! Time capsules: one gauge per active timed power-up in the right side
//! panel, below the SECTOR block, showing how long it has left.
//!
//! Everything is recomputed from [`ActiveEffects`] each frame
//! ([`capsule_views`]), so there's no bookkeeping. Capsule `slot` *i* shows
//! the *i*-th effect in pickup order, newest at the bottom. A refreshed effect
//! keeps its place, so its capsule refills in place. When an effect expires,
//! the later ones move up a slot (the gap closes), and the last slot is
//! despawned. Capsules are run-scoped (`DespawnOnExit(AppState::InGame)`), and
//! a new run's empty `ActiveEffects` leaves none.
//!
//! A capsule is drawn in world space, like the HUD in the left panel. From
//! left to right it has:
//! - the power-up's icon ([`CapsuleIcon`], skinned with `powerup.png` by
//!   `sprites`)
//! - a pill-shaped gauge (light outline, dark pill, `Capsule2d` meshes) whose
//!   cyan fill drains from the left as time runs out
//! - the seconds left, e.g. `6.2s`
//!
//! Each row sits on a dark backing plate ([`crate::plate`], sized by
//! [`capsule_plate_rect`]), a child of the capsule, so it comes and goes
//! with it. It stays solid while the fill blinks.
//!
//! In the last [`WARN_SECS`] the fill turns amber and blinks. The blink phase
//! comes from the time left, not the clock, so a paused game (whose effect
//! timers don't tick) freezes the capsules too.

use super::{ActiveEffects, PowerUpKind, TickActiveEffects};
use crate::game_state::AppState;
use crate::plate::{BackingPlate, PLATE_Z};
use crate::run::{HUD_BLOCK_SPACING, HUD_MARGIN, HUD_RIGHT_X};
use crate::theme;
use crate::world::WORLD_HEIGHT;
use bevy::prelude::*;
use bevy::sprite::Anchor;

/// The last seconds in which the gauge turns amber and blinks.
pub(crate) const WARN_SECS: f32 = 2.0;
/// Blinks per second in the warning window.
const BLINK_HZ: f32 = 4.0;
/// Fill alpha on the "off" half of a blink.
const BLINK_DIM: f32 = 0.3;

/// The right panel's inner left edge, inset by the HUD margin.
const PANEL_LEFT: f32 = HUD_RIGHT_X;
/// Centre line of the first capsule, and the distance between slots. The
/// SECTOR block sits above slot 0, one HUD block higher.
const FIRST_SLOT_Y: f32 = WORLD_HEIGHT / 2.0 - HUD_MARGIN - HUD_BLOCK_SPACING - ICON_SIZE / 2.0;
pub(crate) const SLOT_SPACING: f32 = 56.0;

const ICON_SIZE: f32 = 32.0;
const GAP: f32 = 8.0;
const PILL_HEIGHT: f32 = 22.0;
const PILL_WIDTH: f32 = 96.0;
const OUTLINE: f32 = 2.0;
/// The fill sits inside the pill's straight middle, inset from its edges.
const FILL_INSET: f32 = 4.0;
pub(crate) const FILL_WIDTH: f32 = PILL_WIDTH - PILL_HEIGHT;
const FILL_HEIGHT: f32 = PILL_HEIGHT - 2.0 * FILL_INSET;
const TEXT_SIZE: f32 = 20.0;

const ICON_X: f32 = PANEL_LEFT + ICON_SIZE / 2.0;
const PILL_X: f32 = PANEL_LEFT + ICON_SIZE + GAP + PILL_WIDTH / 2.0;
const TEXT_X: f32 = PANEL_LEFT + ICON_SIZE + GAP + PILL_WIDTH + GAP;
/// A capsule's z; its contents draw at it, its backing plate below.
const CAPSULE_Z: f32 = 1.0;
/// Characters in the widest seconds label a plate fits: `9.9s`, as every
/// power-up today lasts under 10 s.
const LABEL_CHARS: usize = 4;
/// The label font's advance per character (the default FiraMono: 0.6 em).
const TEXT_CHAR_WIDTH: f32 = TEXT_SIZE * 0.6;
/// Padding between a capsule row and the edge of its plate (tighter than the
/// HUD's, so the row's plate stays inside the panel).
const PLATE_PADDING: f32 = 6.0;

/// What one capsule shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CapsuleView {
    pub(crate) kind: PowerUpKind,
    /// Time left as a fraction of the effect's duration, 1 → 0.
    pub(crate) fraction: f32,
    pub(crate) secs_left: f32,
}

impl CapsuleView {
    pub(crate) fn warning(&self) -> bool {
        self.secs_left <= WARN_SECS
    }

    /// The gauge fill's colour: cyan, or amber blinking in the last
    /// [`WARN_SECS`].
    pub(crate) fn fill_color(&self) -> Color {
        if !self.warning() {
            return theme::CAPSULE_FILL;
        }
        let lit = (self.secs_left * BLINK_HZ).fract() >= 0.5;
        theme::CAPSULE_WARN.with_alpha(if lit { 1.0 } else { BLINK_DIM })
    }

    /// `6.2s`.
    pub(crate) fn label(&self) -> String {
        format!("{:.1}s", self.secs_left)
    }
}

/// One capsule per active timed effect, in pickup order. Every effect today
/// is timed. An untimed one would need a flag on `ActiveEffect` and a filter
/// here.
pub(crate) fn capsule_views(active: &ActiveEffects) -> Vec<CapsuleView> {
    active
        .0
        .iter()
        .map(|effect| {
            let duration = effect.timer.duration().as_secs_f32();
            let secs_left = effect.timer.remaining_secs();
            CapsuleView {
                kind: effect.kind,
                fraction: if duration > 0.0 {
                    (secs_left / duration).clamp(0.0, 1.0)
                } else {
                    0.0
                },
                secs_left,
            }
        })
        .collect()
}

/// Centre height of capsule `slot` (0 at the top).
pub(crate) fn slot_y(slot: usize) -> f32 {
    FIRST_SLOT_Y - slot as f32 * SLOT_SPACING
}

/// A capsule row's backing plate, in the capsule's local space (centred on
/// its slot line): from the icon's left edge to the end of a
/// [`LABEL_CHARS`]-wide label, as tall as the icon, plus padding.
pub(crate) fn capsule_plate_rect() -> Rect {
    let content_height = ICON_SIZE.max(PILL_HEIGHT + 2.0 * OUTLINE);
    Rect::new(
        PANEL_LEFT,
        -content_height / 2.0,
        TEXT_X + LABEL_CHARS as f32 * TEXT_CHAR_WIDTH,
        content_height / 2.0,
    )
    .inflate(PLATE_PADDING)
}

/// A capsule; slot 0 is the top one.
#[derive(Component, Debug)]
pub(crate) struct Capsule {
    pub(crate) slot: usize,
}

/// A capsule's power-up icon (flat colour until `sprites` skins it).
#[derive(Component)]
pub(crate) struct CapsuleIcon;

/// A capsule's gauge fill; its width is the fraction left.
#[derive(Component)]
pub(crate) struct CapsuleFill;

/// A capsule's seconds-left text.
#[derive(Component)]
pub(crate) struct CapsuleLabel;

/// The pill's meshes and materials, made once.
#[derive(Resource)]
struct CapsuleLook {
    outline: Handle<Mesh>,
    pill: Handle<Mesh>,
    outline_color: Handle<ColorMaterial>,
    pill_color: Handle<ColorMaterial>,
}

pub(crate) struct CapsulesPlugin;

impl Plugin for CapsulesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, make_capsule_look).add_systems(
            Update,
            sync_capsules
                .after(TickActiveEffects)
                .run_if(in_state(AppState::InGame))
                .run_if(resource_exists::<CapsuleLook>),
        );
    }
}

fn make_capsule_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let pill = |height: f32| Capsule2d::new(height / 2.0, PILL_WIDTH - PILL_HEIGHT);
    commands.insert_resource(CapsuleLook {
        outline: meshes.add(pill(PILL_HEIGHT + 2.0 * OUTLINE)),
        pill: meshes.add(pill(PILL_HEIGHT)),
        outline_color: materials.add(theme::CAPSULE_OUTLINE),
        pill_color: materials.add(theme::CAPSULE_PILL),
    });
}

/// Keeps one capsule per active effect, in order, each showing its effect's
/// time left.
fn sync_capsules(
    mut commands: Commands,
    active: Res<ActiveEffects>,
    look: Res<CapsuleLook>,
    capsules: Query<(Entity, &Capsule)>,
    children: Query<&Children>,
    mut fills: Query<(&mut Sprite, &mut Transform), With<CapsuleFill>>,
    mut labels: Query<&mut Text2d, With<CapsuleLabel>>,
) {
    let views = capsule_views(&active);
    let mut shown = vec![false; views.len()];
    for (entity, capsule) in &capsules {
        let Some(view) = views.get(capsule.slot) else {
            commands.entity(entity).despawn();
            continue;
        };
        shown[capsule.slot] = true;
        for child in children.iter_descendants(entity) {
            if let Ok((mut sprite, mut transform)) = fills.get_mut(child) {
                set_fill(&mut sprite, &mut transform, view);
            }
            if let Ok(mut text) = labels.get_mut(child) {
                let label = view.label();
                if text.0 != label {
                    text.0 = label;
                }
            }
        }
    }
    for (slot, view) in views.iter().enumerate() {
        if !shown[slot] {
            spawn_capsule(&mut commands, &look, slot, view);
        }
    }
}

/// The fill is anchored at the gauge's right end and shrinks toward it, so
/// it drains from the left.
fn set_fill(sprite: &mut Sprite, transform: &mut Transform, view: &CapsuleView) {
    let size = Vec2::new(FILL_WIDTH * view.fraction, FILL_HEIGHT);
    if sprite.custom_size != Some(size) {
        sprite.custom_size = Some(size);
    }
    let color = view.fill_color();
    if sprite.color != color {
        sprite.color = color;
    }
    let x = FILL_WIDTH / 2.0;
    if transform.translation.x != x {
        transform.translation.x = x;
    }
}

fn spawn_capsule(commands: &mut Commands, look: &CapsuleLook, slot: usize, view: &CapsuleView) {
    let mut fill = Sprite::from_color(theme::CAPSULE_FILL, Vec2::ZERO);
    let mut fill_at = Transform::from_xyz(0.0, 0.0, 0.2);
    set_fill(&mut fill, &mut fill_at, view);
    let plate = capsule_plate_rect();
    commands.spawn((
        Capsule { slot },
        Transform::from_xyz(0.0, slot_y(slot), CAPSULE_Z),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
        children![
            (
                BackingPlate { size: plate.size() },
                Transform::from_translation(plate.center().extend(PLATE_Z - CAPSULE_Z)),
            ),
            (
                CapsuleIcon,
                Sprite::from_color(theme::POWER_UP, Vec2::splat(ICON_SIZE)),
                Transform::from_xyz(ICON_X, 0.0, 0.0),
            ),
            (
                Mesh2d(look.outline.clone()),
                MeshMaterial2d(look.outline_color.clone()),
                Transform::from_xyz(PILL_X, 0.0, 0.0),
            ),
            (
                Mesh2d(look.pill.clone()),
                MeshMaterial2d(look.pill_color.clone()),
                Transform::from_xyz(PILL_X, 0.0, 0.1),
                children![(CapsuleFill, fill, Anchor::CENTER_RIGHT, fill_at)],
            ),
            (
                CapsuleLabel,
                Text2d::new(view.label()),
                TextFont {
                    font_size: FontSize::Px(TEXT_SIZE),
                    ..default()
                },
                TextColor(theme::INK),
                Anchor::CENTER_LEFT,
                Transform::from_xyz(TEXT_X, 0.0, 0.0),
            ),
        ],
    ));
}

#[cfg(test)]
mod tests;
