//! The console's Bricks and Power-ups sections, written against [`Controls`]
//! rather than egui directly, so a headless test can drive every control.

use std::ops::RangeInclusive;

use crate::tuning::{BrickTuning, PowerUpTuning, MAX_BLAST_RADIUS};

/// Marks a control whose value only reaches bricks spawned after the change.
pub(super) const NEXT_BOARD: &str = " · next board";

/// One way to show and edit a value: the egui panel, or a test double.
pub(super) trait Controls {
    fn f32(&mut self, label: &str, value: &mut f32, range: RangeInclusive<f32>);
    fn u32(&mut self, label: &str, value: &mut u32, range: RangeInclusive<u32>);
    fn toggle(&mut self, label: &str, value: &mut bool);
    /// A sub-heading inside a section.
    fn heading(&mut self, text: &str);
}

fn u8_control(c: &mut impl Controls, label: &str, value: &mut u8, range: RangeInclusive<u8>) {
    let mut v = u32::from(*value);
    c.u32(
        label,
        &mut v,
        u32::from(*range.start())..=u32::from(*range.end()),
    );
    *value = u8::try_from(v).unwrap_or(u8::MAX);
}

fn usize_control(
    c: &mut impl Controls,
    label: &str,
    value: &mut usize,
    range: RangeInclusive<u32>,
) {
    let mut v = u32::try_from(*value).unwrap_or(u32::MAX);
    c.u32(label, &mut v, range);
    *value = v as usize;
}

/// Every `Tuning.bricks` field. Hits, fill weights and the fallback board's
/// power-ups only reach new bricks, so they're marked [`NEXT_BOARD`].
pub(super) fn bricks(c: &mut impl Controls, b: &mut BrickTuning) {
    c.heading(&format!("Hits per class{NEXT_BOARD}"));
    let h = &mut b.hits;
    for (label, v) in [
        ("ceramic", &mut h.ceramic),
        ("titanium", &mut h.titanium),
        ("tungsten", &mut h.tungsten),
        ("reactor", &mut h.reactor),
        ("explosive", &mut h.explosive),
        ("regen", &mut h.regen),
        ("shield", &mut h.shield),
    ] {
        u8_control(c, label, v, 1..=9);
    }

    c.heading(&format!("Random fill weights{NEXT_BOARD}"));
    let w = &mut b.fill_weights;
    for (label, v) in [
        ("ceramic", &mut w.ceramic),
        ("titanium", &mut w.titanium),
        ("tungsten", &mut w.tungsten),
        ("charge", &mut w.explosive_charge),
        ("breach", &mut w.explosive_breach),
        ("demolition", &mut w.explosive_demolition),
        ("regen", &mut w.regen),
        ("shield", &mut w.shield),
    ] {
        c.u32(label, v, 0..=200);
    }

    c.heading("Other");
    usize_control(
        c,
        &format!("fallback board power-ups{NEXT_BOARD}"),
        &mut b.reactor_bricks,
        0..=20,
    );
    c.f32("regen heal (s)", &mut b.regen_heal_secs, 0.5..=30.0);

    c.heading("Blasts");
    let bl = &mut b.blast;
    for (kind, radius, chains) in [
        ("breach", &mut bl.breach.radius, &mut bl.breach.chains),
        ("charge", &mut bl.charge.radius, &mut bl.charge.chains),
        (
            "demolition",
            &mut bl.demolition.radius,
            &mut bl.demolition.chains,
        ),
    ] {
        c.u32(&format!("{kind} radius"), radius, 0..=MAX_BLAST_RADIUS);
        c.toggle(&format!("{kind} chains"), chains);
    }
}

/// Every `Tuning.powerups` field (all apply right away).
pub(super) fn powerups(c: &mut impl Controls, p: &mut PowerUpTuning) {
    c.heading("Drops");
    c.f32("fall gravity", &mut p.drop_gravity, 50.0..=2000.0);
    c.f32(
        "gravity step per drop",
        &mut p.drop_gravity_step,
        0.0..=200.0,
    );
    c.f32("max gravity", &mut p.max_drop_gravity, 100.0..=3000.0);
    c.f32(
        "drop chance (ordinary bricks)",
        &mut p.drop_chance,
        0.0..=1.0,
    );

    c.heading("Super-Sizer");
    let s = &mut p.super_sizer;
    c.f32("duration (s)", &mut s.duration, 1.0..=30.0);
    c.f32("width multiplier", &mut s.width_multiplier, 1.0..=3.0);
    c.f32("spawn weight", &mut s.weight, 0.0..=10.0);
}
