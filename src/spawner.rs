//! [`Spawner<T>`], a generic weighted registry of spawnable kinds
//! ([`Spawner::register`] a kind with a weight and colour, then
//! [`Spawner::pick`]; no timer).
//!
//! Reusable across domains (obstacles, brick respawns, ...) because Bevy
//! resources are keyed by concrete type: `Spawner<PowerUpKind>` and a
//! hypothetical `Spawner<ObstacleKind>` are automatically independent
//! resources (the same trick Bevy uses for `Time<T>`). It only decides *which
//! kind*; when to pick and actually spawning an entity stay domain-specific.

use bevy::prelude::*;
use rand::{Rng, RngExt};

/// One entry in a [`Spawner`]'s registry: what can be spawned, how likely it
/// is relative to the other registered entries, and what color to render it.
pub struct SpawnDef<T> {
    pub kind: T,
    pub weight: f32,
    pub color: Color,
}

/// What [`Spawner::pick`] returns.
pub struct SpawnResult<T> {
    pub kind: T,
    pub color: Color,
}

/// A generic weighted registry of "things that can be spawned". Because Bevy
/// resources are keyed by concrete type, a `Spawner<PowerUpKind>` and a
/// future `Spawner<ObstacleKind>` are automatically two independent
/// resources without writing that plumbing twice.
///
/// This only decides *which kind*. *When* to pick and actually spawning an
/// entity (what components, what physics) stay domain-specific — power-ups,
/// for instance, are picked for the power-up bricks at the start of a run and
/// spawned when such a brick breaks.
#[derive(Resource)]
pub struct Spawner<T: Copy + Send + Sync + 'static> {
    defs: Vec<SpawnDef<T>>,
}

// Manual impl: `derive(Default)` would require `T: Default`.
impl<T: Copy + Send + Sync + 'static> Default for Spawner<T> {
    fn default() -> Self {
        Self { defs: Vec::new() }
    }
}

impl<T: Copy + Send + Sync + 'static> Spawner<T> {
    /// Adds an entry to the spawn registry. Called by each domain's own
    /// plugin at build time, so the spawner needs no central list of every
    /// kind that exists.
    pub fn register(&mut self, kind: T, weight: f32, color: Color) {
        self.defs.push(SpawnDef {
            kind,
            weight,
            color,
        });
    }

    /// A weighted-random entry from the registry, or `None` if nothing with
    /// a positive weight is registered.
    pub fn pick(&self) -> Option<SpawnResult<T>> {
        choose_weighted(&self.defs, &mut rand::rng()).map(|def| SpawnResult {
            kind: def.kind,
            color: def.color,
        })
    }
}

impl<T: Copy + PartialEq + Send + Sync + 'static> Spawner<T> {
    /// Changes the weight of every entry for `kind` (e.g. from tuning). A
    /// kind that was never registered is left alone.
    pub fn set_weight(&mut self, kind: T, weight: f32) {
        for def in self.defs.iter_mut().filter(|def| def.kind == kind) {
            def.weight = weight;
        }
    }
}

fn choose_weighted<'a, T>(defs: &'a [SpawnDef<T>], rng: &mut impl Rng) -> Option<&'a SpawnDef<T>> {
    let total: f32 = defs.iter().map(|def| def.weight).sum();
    if total <= 0.0 {
        return None;
    }
    let mut roll = rng.random_range(0.0..total);
    for def in defs {
        if roll < def.weight {
            return Some(def);
        }
        roll -= def.weight;
    }
    defs.last()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_is_none_when_nothing_is_registered() {
        assert!(Spawner::<u8>::default().pick().is_none());
    }

    #[test]
    fn pick_only_returns_positively_weighted_kinds() {
        let mut spawner = Spawner::default();
        spawner.register(1u8, 1.0, Color::WHITE);
        spawner.register(2u8, 0.0, Color::BLACK);
        for _ in 0..100 {
            let pick = spawner.pick().expect("kind 1 has weight");
            assert_eq!(pick.kind, 1);
            assert_eq!(pick.color, Color::WHITE);
        }
    }

    #[test]
    fn set_weight_changes_what_pick_returns() {
        let mut spawner = Spawner::default();
        spawner.register(1u8, 1.0, Color::WHITE);
        spawner.register(2u8, 0.0, Color::BLACK);
        spawner.set_weight(1, 0.0);
        spawner.set_weight(2, 1.0);
        spawner.set_weight(3, 5.0); // not registered: ignored
        for _ in 0..50 {
            assert_eq!(spawner.pick().map(|p| p.kind), Some(2));
        }
    }
}
