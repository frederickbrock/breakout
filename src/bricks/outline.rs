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
mod tests {
    use super::*;
    use crate::collision::BrickDamaged;
    use crate::game_state::AppState;
    use crate::test_support::*;

    const SIZE: Vec2 = Vec2::new(BRICK_WIDTH, BRICK_HEIGHT);

    fn outline_of(app: &mut App, brick: Entity) -> Option<Entity> {
        app.world_mut()
            .query::<(Entity, &ChildOf, &BrickOutline)>()
            .iter(app.world())
            .find(|(_, parent, _)| parent.parent() == brick)
            .map(|(e, _, _)| e)
    }

    fn phase(app: &App, outline: Entity) -> f32 {
        app.world().get::<BrickOutline>(outline).unwrap().phase
    }

    fn strip_colors(app: &mut App, outline: Entity) -> Vec<Color> {
        let children: Vec<Entity> = app.world().get::<Children>(outline).unwrap().to_vec();
        children
            .into_iter()
            .map(|c| app.world().get::<Sprite>(c).unwrap().color)
            .collect()
    }

    #[test]
    fn only_special_classes_get_an_outline() {
        use ExplosiveKind::*;
        for class in [
            BrickClass::Ceramic,
            BrickClass::Titanium,
            BrickClass::Tungsten,
        ] {
            assert_eq!(OutlineStyle::of(class), None);
        }
        assert_eq!(
            OutlineStyle::of(BrickClass::Explosive(Charge)),
            Some(OutlineStyle::Charge)
        );
        assert_eq!(
            OutlineStyle::of(BrickClass::Explosive(Breach)),
            Some(OutlineStyle::Breach)
        );
        assert_eq!(
            OutlineStyle::of(BrickClass::Explosive(Demolition)),
            Some(OutlineStyle::Demolition)
        );
        assert_eq!(
            OutlineStyle::of(BrickClass::Regen),
            Some(OutlineStyle::Regen)
        );
        assert_eq!(
            OutlineStyle::of(BrickClass::Shield),
            Some(OutlineStyle::Shield)
        );
        assert_eq!(
            OutlineStyle::of(BrickClass::Reactor),
            Some(OutlineStyle::Reactor)
        );
    }

    #[test]
    fn outline_colours_follow_the_spec() {
        assert_eq!(
            OutlineStyle::Charge.color(),
            Color::srgb_u8(0xff, 0x3b, 0x3b)
        );
        assert_eq!(OutlineStyle::Breach.color(), theme::OUTLINE_EXPLOSIVE);
        assert_eq!(OutlineStyle::Demolition.color(), theme::OUTLINE_EXPLOSIVE);
        assert_eq!(
            OutlineStyle::Regen.color(),
            Color::srgb_u8(0x3d, 0xff, 0x7a)
        );
        assert_eq!(
            OutlineStyle::Shield.color(),
            Color::srgb_u8(0x4f, 0xd8, 0xff)
        );
        assert_eq!(
            OutlineStyle::Reactor.color(),
            Color::srgb_u8(0xb5, 0x8c, 0xff)
        );
    }

    #[test]
    fn every_strip_stays_on_the_brick() {
        use OutlineStyle::*;
        for style in [Charge, Breach, Demolition, Regen, Shield, Reactor] {
            for s in strips(style, SIZE) {
                let min = s.center - s.size / 2.0;
                let max = s.center + s.size / 2.0;
                assert!(min.cmpge(-SIZE / 2.0 - 0.01).all(), "{style:?} {s:?}");
                assert!(max.cmple(SIZE / 2.0 + 0.01).all(), "{style:?} {s:?}");
            }
        }
    }

    #[test]
    fn the_explosive_variants_have_different_shapes_and_rates() {
        let charge = strips(OutlineStyle::Charge, SIZE);
        let breach = strips(OutlineStyle::Breach, SIZE);
        let demolition = strips(OutlineStyle::Demolition, SIZE);
        // Charge: one solid, evenly lit border.
        assert_eq!(charge.len(), 4);
        assert!(charge.iter().all(|s| s.weight == 1.0));
        // Breach: a dim border, bright only at the four side midpoints.
        let bright: Vec<_> = breach.iter().filter(|s| s.weight == 1.0).collect();
        assert_eq!(bright.len(), 4);
        assert!(bright
            .iter()
            .all(|s| s.center.x == 0.0 || s.center.y == 0.0));
        assert!(bright.iter().all(|s| s.size.max_element() < SIZE.y));
        // Demolition: two nested borders, the outer thicker than charge's.
        assert_eq!(demolition.len(), 8);
        assert!(demolition[0].size.y > charge[0].size.y);
        // Demolition pulses twice as fast.
        let hz = |s| pulse_hz(s, None).unwrap();
        assert_eq!(hz(OutlineStyle::Charge), 1.0);
        assert_eq!(hz(OutlineStyle::Breach), 1.0);
        assert_eq!(hz(OutlineStyle::Demolition), 2.0);
    }

    #[test]
    fn shield_glass_is_brightest_along_its_top_edge() {
        let shield = strips(OutlineStyle::Shield, SIZE);
        let top = shield
            .iter()
            .max_by(|a, b| a.center.y.total_cmp(&b.center.y))
            .unwrap();
        assert_eq!(top.weight, 1.0);
        assert!(shield.iter().filter(|s| s.weight == 1.0).count() == 1);
        assert_eq!(pulse_hz(OutlineStyle::Shield, None), None, "steady");
        assert_eq!(pulse_hz(OutlineStyle::Reactor, None), None, "steady");
    }

    #[test]
    fn a_damaged_regen_blinks_faster_as_healing_nears() {
        let breathe = pulse_hz(OutlineStyle::Regen, None).unwrap();
        let just_hit = pulse_hz(OutlineStyle::Regen, Some(0.0)).unwrap();
        let halfway = pulse_hz(OutlineStyle::Regen, Some(0.5)).unwrap();
        let nearly = pulse_hz(OutlineStyle::Regen, Some(0.95)).unwrap();
        assert!(breathe < just_hit && just_hit < halfway && halfway < nearly);
    }

    #[test]
    fn brightness_pulses_between_low_and_full() {
        assert_eq!(brightness(true, 0.0), PULSE_HIGH);
        assert!((brightness(true, 0.5) - PULSE_LOW).abs() < 1e-6);
        assert_eq!(brightness(false, 0.5), PULSE_HIGH);
    }

    #[test]
    fn a_run_outlines_exactly_its_special_bricks() {
        let mut app = app();
        let special = app
            .world_mut()
            .query::<&BrickClass>()
            .iter(app.world())
            .filter(|c| OutlineStyle::of(**c).is_some())
            .count();
        assert!(special > 0);
        assert_eq!(count::<With<BrickOutline>>(&mut app), special);
        for class in [
            BrickClass::Ceramic,
            BrickClass::Titanium,
            BrickClass::Tungsten,
        ] {
            let brick = brick_of(&mut app, class);
            assert_eq!(outline_of(&mut app, brick), None, "{class:?}");
        }
        let breach = brick_of(&mut app, BrickClass::Explosive(ExplosiveKind::Breach));
        let outline = outline_of(&mut app, breach).expect("breach has an outline");
        assert_eq!(strip_colors(&mut app, outline).len(), 8);
    }

    #[test]
    fn destroying_a_brick_takes_its_outline_with_it() {
        let mut app = app();
        let outlines = count::<With<BrickOutline>>(&mut app);
        let strips = count::<With<OutlineStrip>>(&mut app);
        let shield = brick_of(&mut app, BrickClass::Shield);
        let outline = outline_of(&mut app, shield).unwrap();
        hit_moving(&mut app, shield, Vec2::new(0.0, -300.0));
        app.update();
        assert!(app.world().get_entity(shield).is_err());
        assert!(app.world().get_entity(outline).is_err());
        assert_eq!(count::<With<BrickOutline>>(&mut app), outlines - 1);
        assert_eq!(count::<With<OutlineStrip>>(&mut app), strips - 4);

        // Leaving the run leaves no outline behind.
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::MainMenu);
        app.update();
        assert_eq!(count::<With<BrickOutline>>(&mut app), 0);
        assert_eq!(count::<With<OutlineStrip>>(&mut app), 0);
    }

    #[test]
    fn outlines_animate_while_playing_and_freeze_while_paused() {
        let mut app = app();
        let charge = brick_of(&mut app, BrickClass::Explosive(ExplosiveKind::Charge));
        let outline = outline_of(&mut app, charge).unwrap();
        app.update();
        app.update();
        let before = phase(&app, outline);
        assert!(before > 0.0, "pulsing while playing");

        tap(&mut app, KeyCode::KeyP);
        let paused_phase = phase(&app, outline);
        let paused_colors = strip_colors(&mut app, outline);
        for _ in 0..7 {
            app.update();
        }
        assert_eq!(phase(&app, outline), paused_phase, "frozen while paused");
        assert_eq!(strip_colors(&mut app, outline), paused_colors);

        tap(&mut app, KeyCode::KeyP);
        app.update();
        assert_ne!(phase(&app, outline), paused_phase, "resumes");
    }

    #[test]
    fn a_damaged_regen_outline_speeds_up_until_it_heals() {
        let mut app = app();
        let regen = brick_of(&mut app, BrickClass::Regen);
        let outline = outline_of(&mut app, regen).unwrap();
        // Phase advance over one 100 ms frame, in cycles.
        let step = |app: &mut App| {
            let a = phase(app, outline);
            app.update();
            (phase(app, outline) - a).rem_euclid(1.0)
        };
        let breathing = step(&mut app);
        app.world_mut().trigger(BrickDamaged {
            brick: regen,
            position: Vec2::ZERO,
            class: BrickClass::Regen,
        });
        app.world_mut().flush();
        let early = step(&mut app);
        for _ in 0..20 {
            app.update();
        }
        let late = step(&mut app);
        assert!(
            breathing < early && early < late,
            "{breathing} {early} {late}"
        );
        // Healed: back to breathing.
        for _ in 0..10 {
            app.update();
        }
        assert!(!app.world().entity(regen).contains::<RegenTimer>());
        assert!((step(&mut app) - breathing).abs() < 1e-4);
    }
}
