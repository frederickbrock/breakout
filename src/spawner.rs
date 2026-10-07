//! [`Spawner<T>`], a generic weighted registry of spawnable kinds
//! ([`Spawner::register`] a kind with a weight and colour, then
//! [`Spawner::pick`]; no timer).
//!
//! An entry can be limited to once per *cycle* ([`SpawnDef::once_per_cycle`]):
//! after [`Spawner::record`] it is skipped by `pick` until
//! [`Spawner::new_cycle`]. What a cycle is belongs to the domain (a level, for
//! power-ups). An entry can also carry a `tint` ([`SpawnDef::tinted`]) for its
//! skinned image, apart from its plain `color`.
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
    /// The tint for the kind's skinned image (white: the image as-is).
    pub tint: Color,
    /// At most once per cycle (see [`Spawner::record`]).
    pub once: bool,
}

impl<T> SpawnDef<T> {
    /// Draws the kind's skinned image with `tint`.
    pub fn tinted(&mut self, tint: Color) -> &mut Self {
        self.tint = tint;
        self
    }

    /// Limits the kind to once per cycle.
    pub fn once_per_cycle(&mut self) -> &mut Self {
        self.once = true;
        self
    }
}

/// What [`Spawner::pick`] returns.
pub struct SpawnResult<T> {
    pub kind: T,
    pub color: Color,
    pub tint: Color,
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
    /// Once-per-cycle kinds already spawned this cycle.
    used: Vec<T>,
}

// Manual impl: `derive(Default)` would require `T: Default`.
impl<T: Copy + Send + Sync + 'static> Default for Spawner<T> {
    fn default() -> Self {
        Self {
            defs: Vec::new(),
            used: Vec::new(),
        }
    }
}

impl<T: Copy + PartialEq + Send + Sync + 'static> Spawner<T> {
    /// Adds an entry to the spawn registry. Called by each domain's own
    /// plugin at build time, so the spawner needs no central list of every
    /// kind that exists. Returns the entry for [`SpawnDef::tinted`] /
    /// [`SpawnDef::once_per_cycle`].
    pub fn register(&mut self, kind: T, weight: f32, color: Color) -> &mut SpawnDef<T> {
        self.defs.push(SpawnDef {
            kind,
            weight,
            color,
            tint: Color::WHITE,
            once: false,
        });
        self.defs.last_mut().expect("just pushed")
    }

    /// A weighted-random entry from the registry, skipping once-per-cycle
    /// kinds already used this cycle; `None` if nothing available has a
    /// positive weight.
    pub fn pick(&self) -> Option<SpawnResult<T>> {
        let available: Vec<&SpawnDef<T>> = self
            .defs
            .iter()
            .filter(|def| self.is_available(def.kind))
            .collect();
        choose_weighted(&available, &mut rand::rng()).map(|def| SpawnResult {
            kind: def.kind,
            color: def.color,
            tint: def.tint,
        })
    }

    /// False for a once-per-cycle kind already recorded this cycle.
    pub fn is_available(&self, kind: T) -> bool {
        !self.used.contains(&kind)
    }

    /// Notes that `kind` was spawned: a once-per-cycle kind is then skipped
    /// until [`Spawner::new_cycle`].
    pub fn record(&mut self, kind: T) {
        let once = self.defs.iter().any(|def| def.kind == kind && def.once);
        if once && !self.used.contains(&kind) {
            self.used.push(kind);
        }
    }

    /// Starts a new cycle: every kind is available again.
    pub fn new_cycle(&mut self) {
        self.used.clear();
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

fn choose_weighted<'a, T>(defs: &[&'a SpawnDef<T>], rng: &mut impl Rng) -> Option<&'a SpawnDef<T>> {
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
    defs.last().copied()
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
    fn a_once_per_cycle_kind_is_picked_once_until_the_next_cycle() {
        let mut spawner = Spawner::default();
        spawner.register(1u8, 1.0, Color::WHITE).once_per_cycle();
        spawner.register(2u8, 0.0, Color::BLACK);
        assert_eq!(spawner.pick().map(|p| p.kind), Some(1));
        spawner.record(1);
        assert!(!spawner.is_available(1));
        assert!(spawner.pick().is_none(), "only kind 2 is left, at weight 0");
        spawner.new_cycle();
        assert!(spawner.is_available(1));
        assert_eq!(spawner.pick().map(|p| p.kind), Some(1));
    }

    #[test]
    fn a_kind_without_once_is_never_used_up() {
        let mut spawner = Spawner::default();
        spawner.register(1u8, 1.0, Color::WHITE);
        for _ in 0..5 {
            spawner.record(1);
        }
        assert!(spawner.is_available(1));
        assert_eq!(spawner.pick().map(|p| p.kind), Some(1));
    }

    #[test]
    fn pick_carries_the_tint() {
        let mut spawner = Spawner::default();
        spawner
            .register(1u8, 1.0, Color::BLACK)
            .tinted(Color::WHITE);
        let pick = spawner.pick().unwrap();
        assert_eq!((pick.color, pick.tint), (Color::BLACK, Color::WHITE));
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
