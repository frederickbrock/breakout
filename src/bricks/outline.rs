//! Behaviour outlines: a border drawn over each special brick so the player
//! can tell what it does at a glance. Coded placeholders; the final art
//! (sim-rdl.7.5) replaces only the strip children.
//!
//! Every special brick gets one [`BrickOutline`] child holding thin
//! [`OutlineStrip`] sprites laid out by the pure [`strips`]. Plain classes
//! (ceramic, titanium, tungsten) get none. The outline is a child of the
//! brick, so it goes when the brick does. [`animate_outlines`] advances each
//! outline's pulse phase and sets its strips' brightness; it only runs while
//! playing, so the animation freezes while paused.

use super::grid::{BRICK_HEIGHT, BRICK_WIDTH};
use super::regen::RegenTimer;
use super::{BrickClass, ExplosiveKind};
use crate::game_state::PlayState;
use crate::theme;
use bevy::prelude::*;
use std::f32::consts::TAU;

/// Draws above the brick's face (and its sprite, once skinned), below falling
/// power-up drops (z 0.5) and particles (z 0.6).
const OUTLINE_Z: f32 = 0.25;
/// Border thickness.
const THICK: f32 = 2.0;
/// Breach's bright side-midpoint marks: length along the edge, thickness.
const BREACH_MARK: f32 = 14.0;
const BREACH_MARK_THICK: f32 = 3.0;
/// Demolition's inner border sits this far inside the outer one.
const DEMOLITION_INSET: f32 = 5.0;
/// Shield glass's top edge (the side that breaks it).
const SHIELD_TOP_THICK: f32 = 3.0;
/// Strip weight of the parts of an outline that are only faintly lit.
const DIM: f32 = 0.3;
/// Brightness range of a pulsing outline.
const PULSE_LOW: f32 = 0.25;
const PULSE_HIGH: f32 = 1.0;

/// Which outline a special brick wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineStyle {
    Charge,
    Breach,
    Demolition,
    Regen,
    Shield,
    Reactor,
}

impl OutlineStyle {
    /// The outline for `class`, or `None` for the plain classes.
    pub fn of(class: BrickClass) -> Option<Self> {
        match class {
            BrickClass::Ceramic | BrickClass::Titanium | BrickClass::Tungsten => None,
            BrickClass::Explosive(ExplosiveKind::Charge) => Some(Self::Charge),
            BrickClass::Explosive(ExplosiveKind::Breach) => Some(Self::Breach),
            BrickClass::Explosive(ExplosiveKind::Demolition) => Some(Self::Demolition),
            BrickClass::Regen => Some(Self::Regen),
            BrickClass::Shield => Some(Self::Shield),
            BrickClass::Reactor => Some(Self::Reactor),
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Charge | Self::Breach | Self::Demolition => theme::OUTLINE_EXPLOSIVE,
            Self::Regen => theme::OUTLINE_REGEN,
            Self::Shield => theme::OUTLINE_SHIELD,
            Self::Reactor => theme::OUTLINE_REACTOR,
        }
    }
}

/// A special brick's outline (a child of the brick); its children are the
/// [`OutlineStrip`]s.
#[derive(Component, Debug)]
pub struct BrickOutline {
    pub style: OutlineStyle,
    /// Pulse phase in cycles; advanced only while playing.
    phase: f32,
}

/// One quad of an outline. `weight` is its brightness relative to the
/// outline's current pulse (1 = the brightest part).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct OutlineStrip {
    pub weight: f32,
}

/// Where one strip sits, relative to the brick's centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strip {
    pub center: Vec2,
    pub size: Vec2,
    pub weight: f32,
}

pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(add_outline).add_systems(
            Update,
            animate_outlines.run_if(in_state(PlayState::Playing)),
        );
    }
}

/// A plain border of `thickness` just inside a `size` rectangle, at `weight`.
fn border(size: Vec2, thickness: f32, weight: f32) -> [Strip; 4] {
    let half = size / 2.0;
    let horizontal = Vec2::new(size.x, thickness);
    let vertical = Vec2::new(thickness, size.y - 2.0 * thickness);
    [
        Strip {
            center: Vec2::new(0.0, half.y - thickness / 2.0),
            size: horizontal,
            weight,
        },
        Strip {
            center: Vec2::new(0.0, -half.y + thickness / 2.0),
            size: horizontal,
            weight,
        },
        Strip {
            center: Vec2::new(-half.x + thickness / 2.0, 0.0),
            size: vertical,
            weight,
        },
        Strip {
            center: Vec2::new(half.x - thickness / 2.0, 0.0),
            size: vertical,
            weight,
        },
    ]
}

/// The strips making up `style`'s outline on a brick of `size`.
pub fn strips(style: OutlineStyle, size: Vec2) -> Vec<Strip> {
    let half = size / 2.0;
    match style {
        OutlineStyle::Charge | OutlineStyle::Regen | OutlineStyle::Reactor => {
            border(size, THICK, 1.0).to_vec()
        }
        OutlineStyle::Breach => {
            // A faint border, bright only at the four side midpoints: a "+".
            let t = BREACH_MARK_THICK;
            let mut strips = border(size, THICK, DIM).to_vec();
            strips.extend([
                Strip {
                    center: Vec2::new(0.0, half.y - t / 2.0),
                    size: Vec2::new(BREACH_MARK, t),
                    weight: 1.0,
                },
                Strip {
                    center: Vec2::new(0.0, -half.y + t / 2.0),
                    size: Vec2::new(BREACH_MARK, t),
                    weight: 1.0,
                },
                Strip {
                    center: Vec2::new(-half.x + t / 2.0, 0.0),
                    size: Vec2::new(t, BREACH_MARK),
                    weight: 1.0,
                },
                Strip {
                    center: Vec2::new(half.x - t / 2.0, 0.0),
                    size: Vec2::new(t, BREACH_MARK),
                    weight: 1.0,
                },
            ]);
            strips
        }
        OutlineStyle::Demolition => {
            // Thick double border.
            let mut strips = border(size, THICK + 1.0, 1.0).to_vec();
            strips.extend(border(
                size - Vec2::splat(2.0 * DEMOLITION_INSET),
                THICK,
                1.0,
            ));
            strips
        }
        OutlineStyle::Shield => {
            // Brightest along the top edge, the side that breaks it.
            let mut strips = border(size, THICK, DIM).to_vec();
            strips[0] = Strip {
                center: Vec2::new(0.0, half.y - SHIELD_TOP_THICK / 2.0),
                size: Vec2::new(size.x, SHIELD_TOP_THICK),
                weight: 1.0,
            };
            strips
        }
    }
}

/// How fast `style`'s outline pulses, in Hz; `None` for a steady glow.
/// `heal_elapsed` is a damaged regen brick's heal countdown progress (0 just
/// hit, 1 about to heal): the closer to healing, the faster it blinks.
pub fn pulse_hz(style: OutlineStyle, heal_elapsed: Option<f32>) -> Option<f32> {
    match style {
        OutlineStyle::Charge => Some(theme::CHARGE_PULSE_HZ),
        OutlineStyle::Breach => Some(theme::BREACH_PULSE_HZ),
        OutlineStyle::Demolition => Some(theme::DEMOLITION_PULSE_HZ),
        OutlineStyle::Regen => Some(match heal_elapsed {
            None => theme::REGEN_BREATHE_HZ,
            Some(t) => {
                let t = t.clamp(0.0, 1.0);
                theme::REGEN_BLINK_START_HZ
                    + (theme::REGEN_BLINK_END_HZ - theme::REGEN_BLINK_START_HZ) * t
            }
        }),
        OutlineStyle::Shield | OutlineStyle::Reactor => None,
    }
}

/// An outline's brightness (0..=1) at `phase` cycles into its pulse; a
/// steady outline is always fully lit.
pub fn brightness(pulsing: bool, phase: f32) -> f32 {
    if !pulsing {
        return PULSE_HIGH;
    }
    let wave = 0.5 + 0.5 * (TAU * phase).cos();
    PULSE_LOW + (PULSE_HIGH - PULSE_LOW) * wave
}

fn strip_color(style: OutlineStyle, weight: f32, brightness: f32) -> Color {
    style.color().with_alpha(weight * brightness)
}

/// Gives every special brick its outline as soon as it's spawned.
fn add_outline(on: On<Add, BrickClass>, mut commands: Commands, classes: Query<&BrickClass>) {
    let Ok(&class) = classes.get(on.entity) else {
        return;
    };
    let Some(style) = OutlineStyle::of(class) else {
        return;
    };
    let size = Vec2::new(BRICK_WIDTH, BRICK_HEIGHT);
    let outline = commands
        .spawn((
            BrickOutline { style, phase: 0.0 },
            Transform::from_xyz(0.0, 0.0, OUTLINE_Z),
            Visibility::default(),
            ChildOf(on.entity),
        ))
        .id();
    for strip in strips(style, size) {
        commands.spawn((
            Sprite::from_color(strip_color(style, strip.weight, PULSE_HIGH), strip.size),
            Transform::from_translation(strip.center.extend(0.0)),
            OutlineStrip {
                weight: strip.weight,
            },
            ChildOf(outline),
        ));
    }
}

/// Advances each outline's pulse and relights its strips.
fn animate_outlines(
    time: Res<Time>,
    mut outlines: Query<(&mut BrickOutline, &ChildOf, &Children)>,
    timers: Query<&RegenTimer>,
    mut strips: Query<(&OutlineStrip, &mut Sprite)>,
) {
    for (mut outline, parent, children) in &mut outlines {
        let heal = timers
            .get(parent.parent())
            .ok()
            .map(RegenTimer::fraction_elapsed);
        let hz = pulse_hz(outline.style, heal);
        if let Some(hz) = hz {
            outline.phase = (outline.phase + hz * time.delta_secs()).fract();
        }
        let lit = brightness(hz.is_some(), outline.phase);
        for &child in children {
            if let Ok((strip, mut sprite)) = strips.get_mut(child) {
                sprite.color = strip_color(outline.style, strip.weight, lit);
            }
        }
    }
}

#[cfg(test)]
mod tests;
