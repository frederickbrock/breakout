//! [`Tuning`]: the game's tunable numbers as one runtime resource, loaded
//! from `assets/game.tuning.ron` in every build (native and web).
//!
//! `Tuning` nests a struct per group ([`BallTuning`], [`PaddleTuning`],
//! [`BrickTuning`], [`PowerUpTuning`]). [`Tuning::default`] is built from the
//! compiled consts, so it is today's behaviour, and every struct is
//! `#[serde(default)]`: a file with only some fields (or from an older build)
//! loads, and whatever it leaves out keeps its default. Distances and speeds
//! are in world units (already scaled by `GAME_SCALE`).
//!
//! Readers so far: bricks (hits per class, the random fill, the fallback
//! board's power-ups, regen heal time; new bricks and new heal timers use
//! the current values) and power-ups (drop gravity, Super-Sizer's duration,
//! width multiplier and spawn weight, re-applied to the spawner when `Tuning`
//! changes).
//!
//! Two plugins, split like the levels:
//! - [`add_game`](crate) inits `Tuning` to its defaults, so the headless tests
//!   see it without reading a file.
//! - [`TuningPlugin`] (from `main()` only) registers [`TuningLoader`] for
//!   `*.tuning.ron` (not plain `ron`, which bevy_enoki's effect loader
//!   claims), loads [`TUNING_PATH`] at startup and [`sync_tuning`] copies
//!   each load or reload into the resource. Natively, saving the file while
//!   the game runs reloads it (Bevy's `file_watcher`). A missing file or a
//!   parse error logs a warning and keeps the values the resource has.
//!
//! The web splash (`web_splash.rs`) also waits for the file to load or fail,
//! through [`TuningHandle`].

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoadFailedEvent, AssetLoader, LoadContext};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ball::{BallSpeed, BALL_MIN_VERTICAL_FRACTION, BALL_SPEED_SCALE};
use crate::bricks::{BrickClass, ExplosiveKind, FILL_WEIGHTS, REACTOR_BRICKS};
use crate::controls::{FOLLOW_GAIN, MAX_FOLLOW_SPEED, MAX_GAP_PER_FRAME};
use crate::paddle::{PADDLE_FORCE, PADDLE_LINEAR_DAMPING, PADDLE_MASS, PADDLE_WIDTH};
use crate::powerups::{BASE_GRAVITY, GRAVITY_STEP, MAX_GRAVITY};
use crate::run::STARTING_LIVES;

/// The tuning file, relative to `assets/`.
pub const TUNING_PATH: &str = "game.tuning.ron";

/// Every tunable value, grouped.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tuning {
    pub ball: BallTuning,
    pub paddle: PaddleTuning,
    pub bricks: BrickTuning,
    pub powerups: PowerUpTuning,
}

/// The ball, and the lives ("balls") a run starts with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BallTuning {
    /// Speed factor when a level doesn't set `speed_factor:`.
    pub speed_factor: f32,
    /// World units/s per unit of speed factor.
    pub speed_per_factor: f32,
    /// The smallest share of the ball's speed that is vertical.
    pub min_vertical_fraction: f32,
    /// Lives at the start of a run.
    pub lives: i32,
}

impl Default for BallTuning {
    fn default() -> Self {
        Self {
            speed_factor: BALL_SPEED_SCALE,
            speed_per_factor: BallSpeed::PER_FACTOR,
            min_vertical_fraction: BALL_MIN_VERTICAL_FRACTION,
            lives: STARTING_LIVES,
        }
    }
}

/// Paddle size and feel (keyboard force, mouse follow).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaddleTuning {
    pub width: f32,
    pub mass: f32,
    /// Keyboard push.
    pub force: f32,
    pub linear_damping: f32,
    /// Mouse follow: how quickly the gap to the cursor closes, per second.
    pub follow_gain: f32,
    /// Mouse follow: top speed.
    pub max_follow_speed: f32,
    /// Mouse follow: most of the gap one frame may close.
    pub max_gap_per_frame: f32,
}

impl Default for PaddleTuning {
    fn default() -> Self {
        Self {
            width: PADDLE_WIDTH,
            mass: PADDLE_MASS,
            force: PADDLE_FORCE,
            linear_damping: PADDLE_LINEAR_DAMPING,
            follow_gain: FOLLOW_GAIN,
            max_follow_speed: MAX_FOLLOW_SPEED,
            max_gap_per_frame: MAX_GAP_PER_FRAME,
        }
    }
}

/// Bricks: hits per class, the random fill and regen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrickTuning {
    /// Hits a full-health brick of each class takes (a level's `hits=` wins).
    pub hits: ClassHits,
    /// Relative weights of the classes a `?` cell can become.
    pub fill_weights: FillWeights,
    /// Power-up (reactor) bricks on the built-in random board (used when
    /// there is no campaign; a level's `powerups:` sets its own).
    pub reactor_bricks: usize,
    /// Seconds a damaged regen brick takes to heal.
    pub regen_heal_secs: f32,
}

impl Default for BrickTuning {
    fn default() -> Self {
        Self {
            hits: ClassHits::default(),
            fill_weights: FillWeights::default(),
            reactor_bricks: REACTOR_BRICKS,
            regen_heal_secs: crate::bricks::regen::HEAL_SECS,
        }
    }
}

/// Hits per brick class (the explosive variants share one value).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassHits {
    pub ceramic: u8,
    pub titanium: u8,
    pub tungsten: u8,
    pub reactor: u8,
    pub explosive: u8,
    pub regen: u8,
    pub shield: u8,
}

impl ClassHits {
    /// Hits for `class`.
    pub fn of(&self, class: BrickClass) -> u8 {
        match class {
            BrickClass::Ceramic => self.ceramic,
            BrickClass::Titanium => self.titanium,
            BrickClass::Tungsten => self.tungsten,
            BrickClass::Reactor => self.reactor,
            BrickClass::Explosive(_) => self.explosive,
            BrickClass::Regen => self.regen,
            BrickClass::Shield => self.shield,
        }
    }
}

impl Default for ClassHits {
    fn default() -> Self {
        Self {
            ceramic: BrickClass::Ceramic.max_hits(),
            titanium: BrickClass::Titanium.max_hits(),
            tungsten: BrickClass::Tungsten.max_hits(),
            reactor: BrickClass::Reactor.max_hits(),
            explosive: BrickClass::Explosive(ExplosiveKind::Charge).max_hits(),
            regen: BrickClass::Regen.max_hits(),
            shield: BrickClass::Shield.max_hits(),
        }
    }
}

/// The random fill's weight per class (reactors are placed separately).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FillWeights {
    pub ceramic: u32,
    pub titanium: u32,
    pub tungsten: u32,
    pub explosive_charge: u32,
    pub explosive_breach: u32,
    pub explosive_demolition: u32,
    pub regen: u32,
    pub shield: u32,
}

impl FillWeights {
    /// The fill table, in `bricks::FILL_WEIGHTS` order (the order the rng
    /// walks it, so the default table reproduces today's boards).
    pub fn table(&self) -> [(BrickClass, u32); 8] {
        FILL_WEIGHTS.map(|(class, _)| {
            let weight = match class {
                BrickClass::Ceramic => self.ceramic,
                BrickClass::Titanium => self.titanium,
                BrickClass::Tungsten => self.tungsten,
                BrickClass::Explosive(ExplosiveKind::Charge) => self.explosive_charge,
                BrickClass::Explosive(ExplosiveKind::Breach) => self.explosive_breach,
                BrickClass::Explosive(ExplosiveKind::Demolition) => self.explosive_demolition,
                BrickClass::Regen => self.regen,
                BrickClass::Shield => self.shield,
                BrickClass::Reactor => 0,
            };
            (class, weight)
        })
    }
}

impl Default for FillWeights {
    fn default() -> Self {
        let weight = |class: BrickClass| {
            FILL_WEIGHTS
                .iter()
                .find(|(c, _)| *c == class)
                .map_or(0, |(_, w)| *w)
        };
        Self {
            ceramic: weight(BrickClass::Ceramic),
            titanium: weight(BrickClass::Titanium),
            tungsten: weight(BrickClass::Tungsten),
            explosive_charge: weight(BrickClass::Explosive(ExplosiveKind::Charge)),
            explosive_breach: weight(BrickClass::Explosive(ExplosiveKind::Breach)),
            explosive_demolition: weight(BrickClass::Explosive(ExplosiveKind::Demolition)),
            regen: weight(BrickClass::Regen),
            shield: weight(BrickClass::Shield),
        }
    }
}

/// Falling power-ups and each kind's numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PowerUpTuning {
    /// Fall acceleration of a run's first drop.
    pub drop_gravity: f32,
    /// Added for each later drop in the run.
    pub drop_gravity_step: f32,
    /// The heaviest a drop gets.
    pub max_drop_gravity: f32,
    pub super_sizer: SuperSizerTuning,
}

impl Default for PowerUpTuning {
    fn default() -> Self {
        Self {
            drop_gravity: BASE_GRAVITY,
            drop_gravity_step: GRAVITY_STEP,
            max_drop_gravity: MAX_GRAVITY,
            super_sizer: SuperSizerTuning::default(),
        }
    }
}

/// Super-Sizer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SuperSizerTuning {
    /// Spawn weight relative to the other kinds.
    pub weight: f32,
    /// Paddle width while active, as a multiple of the normal width.
    pub width_multiplier: f32,
    /// Seconds it lasts.
    pub duration: f32,
}

impl Default for SuperSizerTuning {
    fn default() -> Self {
        use crate::powerups::super_sizer::{DURATION, WEIGHT, WIDTH_MULTIPLIER};
        Self {
            weight: WEIGHT,
            width_multiplier: WIDTH_MULTIPLIER,
            duration: DURATION,
        }
    }
}

/// Parses a tuning file (RON). Missing fields take their defaults.
pub fn parse_tuning(text: &str) -> Result<Tuning, ron::error::SpannedError> {
    ron::from_str(text)
}

/// A loaded tuning file.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct TuningAsset(pub Tuning);

/// Loads `*.tuning.ron` files with [`parse_tuning`].
#[derive(Default, TypePath)]
pub struct TuningLoader;

impl AssetLoader for TuningLoader {
    type Asset = TuningAsset;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<TuningAsset, BevyError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(TuningAsset(parse_tuning(std::str::from_utf8(&bytes)?)?))
    }

    fn extensions(&self) -> &[&str] {
        &["tuning.ron"]
    }
}

/// The tuning file being loaded (or loaded). Only exists when
/// [`TuningPlugin`] is in the app.
#[derive(Resource, Default)]
pub struct TuningHandle(pub Handle<TuningAsset>);

/// Loads the tuning file. Registered from `main()` only, so the headless
/// tests never read `assets/`.
pub struct TuningPlugin;

impl Plugin for TuningPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<TuningAsset>()
            .register_asset_loader(TuningLoader)
            .init_resource::<TuningHandle>()
            .add_systems(Startup, load_tuning)
            .add_systems(PreUpdate, sync_tuning);
    }
}

fn load_tuning(server: Res<AssetServer>, mut handle: ResMut<TuningHandle>) {
    handle.0 = server.load(TUNING_PATH);
}

/// Copies the tuning file into [`Tuning`] whenever it loads or reloads; a
/// failed load (missing file, bad RON) only warns.
pub(crate) fn sync_tuning(
    handle: Res<TuningHandle>,
    files: Res<Assets<TuningAsset>>,
    mut tuning: ResMut<Tuning>,
    mut events: MessageReader<AssetEvent<TuningAsset>>,
    mut failed: MessageReader<AssetLoadFailedEvent<TuningAsset>>,
) {
    let id = handle.0.id();
    let changed = events.read().any(|event| {
        event.is_added(id) || event.is_loaded_with_dependencies(id) || event.is_modified(id)
    });
    if changed {
        if let Some(file) = files.get(id) {
            if *tuning != file.0 {
                *tuning = file.0.clone();
            }
            info!("tuning loaded from {TUNING_PATH}");
        }
    }
    for failure in failed.read().filter(|f| f.id == id) {
        warn!(
            "{}: {}; keeping the current tuning values",
            failure.path, failure.error
        );
    }
}

#[cfg(test)]
mod tests;
