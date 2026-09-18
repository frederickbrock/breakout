use bevy::prelude::*;
use rand::{Rng, RngExt};
use std::ops::RangeInclusive;

/// One entry in a [`Spawner`]'s registry: what can be spawned, how likely it
/// is relative to the other registered entries, and what color to render it.
pub struct SpawnDef<T> {
    pub kind: T,
    pub weight: f32,
    pub color: Color,
}

/// What a [`Spawner`] produces when its timer fires.
pub struct SpawnResult<T> {
    pub kind: T,
    pub color: Color,
}

/// A generic, reusable "every so often, produce one of several weighted
/// things" engine. Because Bevy resources are keyed by concrete type, a
/// `Spawner<PowerUpKind>` and a future `Spawner<ObstacleKind>` are
/// automatically two independent resources — separate timer, registry, and
/// dispensed counter — without writing that plumbing twice.
///
/// This only decides *when* and *which kind*. Actually spawning an entity
/// (what components, what physics) stays domain-specific.
#[derive(Resource)]
pub struct Spawner<T: Copy + Send + Sync + 'static> {
    interval: RangeInclusive<f32>,
    timer: Timer,
    defs: Vec<SpawnDef<T>>,
    dispensed: u32,
}

impl<T: Copy + Send + Sync + 'static> Spawner<T> {
    pub fn new(interval: RangeInclusive<f32>) -> Self {
        let mut rng = rand::rng();
        Self {
            timer: Timer::from_seconds(rng.random_range(interval.clone()), TimerMode::Once),
            interval,
            defs: Vec::new(),
            dispensed: 0,
        }
    }

    /// Adds an entry to the spawn registry. Called by each domain's own
    /// plugin at build time, so the spawner needs no central list of every
    /// kind that exists.
    pub fn register(&mut self, kind: T, weight: f32, color: Color) {
        self.defs.push(SpawnDef { kind, weight, color });
    }

    pub fn dispensed(&self) -> u32 {
        self.dispensed
    }

    /// Re-randomizes the timer and zeroes the dispensed counter, but keeps
    /// the registered defs (those are only ever added once, at plugin build
    /// time).
    pub fn reset(&mut self) {
        let defs = std::mem::take(&mut self.defs);
        *self = Self::new(self.interval.clone());
        self.defs = defs;
    }

    /// Ticks the timer; if it just fired, re-randomizes the next interval,
    /// bumps the dispensed counter, and returns a weighted-random kind to
    /// spawn.
    pub fn tick(&mut self, dt: std::time::Duration) -> Option<SpawnResult<T>> {
        if !self.timer.tick(dt).is_finished() {
            return None;
        }

        let mut rng = rand::rng();
        self.timer = Timer::from_seconds(rng.random_range(self.interval.clone()), TimerMode::Once);
        self.dispensed += 1;

        choose_weighted(&self.defs, &mut rng).map(|def| SpawnResult {
            kind: def.kind,
            color: def.color,
        })
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
