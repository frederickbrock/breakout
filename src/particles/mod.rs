//! The VFX layer: bevy_enoki particles for brick damage.
//!
//! **Gameplay fires events, particles observe.** Gameplay code never touches
//! particles. It triggers `BrickDamaged` / `BrickDestroyed` and changes
//! `BrickHealth`, and this module reacts:
//!
//! | trigger | effect file (`assets/particles/`) | colour |
//! |---|---|---|
//! | `BrickDamaged` | the class's `<class>_hit` burst at the contact point | its own `color_curve` |
//! | `BrickHealth` below max | child emitters: `brick_damage_smoke` + `brick_damage_sparks`, heavier with more damage | smoke grey / class glow |
//! | `BrickHealth` back at max (regen heal) | those child emitters removed | — |
//! | `BrickDestroyed` | the class's `<class>_break` burst at the brick's centre | its own `color_curve` |
//!
//! **Per-class hit and break bursts** (sim-rdl.7.8). Each brick material has
//! its own pair of effect files and its own particle sheet:
//! - `<class>_hit.particle.ron` and `<class>_break.particle.ron`
//! - the sheet `<class>.png`, a 4-frame 128×32 greyscale strip animated over
//!   each particle's life
//!
//! `<class>` is [`material_slug`]: `ceramic`, `titanium`, `tungsten`,
//! `reactor`, `explosive` (all three variants share it), `regen` or
//! `shield`. These files colour themselves with their `color_curve`, and the
//! sheet is drawn through a `SpriteParticle2dMaterial`. [`burst_look`]
//! picks the fallbacks:
//! - a sheet that isn't loaded (missing file): the same effect as plain
//!   quads (a white `ColorParticle2dMaterial`, so the curve's colour shows)
//! - an effect file that failed to load: pdp.1's generic `brick_hit` /
//!   `brick_break`, tinted by class
//!
//! The damage emitters and the generic effects are drawn white, and the
//! colour comes from the spawner's `ColorParticle2dMaterial` (one per class
//! and role, [`ParticleMaterials`]), which multiplies it. Editing a
//! `.particle.ron` while `cargo run` is live hot-reloads it.
//!
//! Two plugins:
//! - [`ParticlesPlugin`] (from `main()`; needs the renderer) adds
//!   `EnokiPlugin`, loads the effects and materials, and freezes particles
//!   while paused by pausing `Time<Virtual>` (bevy_enoki ticks on it) from
//!   `OnEnter(PlayState::Paused)` to `OnExit`.
//! - [`VfxPlugin`] (in `add_game`) spawns the spawners. It's a no-op without
//!   the resources above, so the headless tests run it with placeholder
//!   handles and count spawner entities.
//!
//! Every spawner is scoped to the run (a brick child, or
//! `DespawnOnExit(AppState::InGame)`), so leaving the run leaves none. A live
//! particle budget ([`damage_emitter_interval`]) slows the continuous
//! emitters when many bricks are damaged.
//!
//! The budget is [`DAMAGE_PARTICLE_BUDGET`], applied by stretching the damage
//! emitters' spawn interval rather than via `max_particles` (bevy_enoki stops
//! *moving* a spawner's particles once it's at that cap). Effect files are in
//! world units (sizes and speeds already × `GAME_SCALE`), and the
//! `brick_break` shatter falls with gravity. Pausing `Time<Virtual>` is safe
//! because gameplay gates on `PlayState::Playing` and physics has its own
//! clock. Headless tests insert placeholder `ParticleEffects` /
//! `ParticleMaterials` and count spawner entities.

use crate::bricks::grid::{Brick, BrickHealth};
use crate::bricks::BrickClass;
use crate::collision::{BrickDamaged, BrickDestroyed};
use crate::game_state::{AppState, PlayState};
use crate::theme;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_enoki::prelude::*;

const HIT_PATH: &str = "particles/brick_hit.particle.ron";
const SMOKE_PATH: &str = "particles/brick_damage_smoke.particle.ron";
const SPARKS_PATH: &str = "particles/brick_damage_sparks.particle.ron";
const BREAK_PATH: &str = "particles/brick_break.particle.ron";

/// In front of bricks (z 0) and the HUD's backdrop, behind menus.
const PARTICLE_Z: f32 = 0.6;
/// Upper bound on live particles from the continuous damage emitters, so a
/// board full of damaged bricks can't tank the frame rate (WSLg's software
/// renderer).
pub const DAMAGE_PARTICLE_BUDGET: f32 = 1500.0;
/// Seconds between damage-emitter waves at full damage (a brick on its last
/// hit); lighter damage emits proportionally less often.
const SMOKE_INTERVAL: f32 = 0.12;
const SPARKS_INTERVAL: f32 = 0.5;

/// Handles to the effect assets.
#[derive(Resource, Clone)]
pub struct ParticleEffects {
    pub hit: Handle<Particle2dEffect>,
    pub smoke: Handle<Particle2dEffect>,
    pub sparks: Handle<Particle2dEffect>,
    pub shatter: Handle<Particle2dEffect>,
    /// Each class's own hit and break bursts (explosive variants share).
    pub class_hit: HashMap<BrickClass, Handle<Particle2dEffect>>,
    pub class_break: HashMap<BrickClass, Handle<Particle2dEffect>>,
}

/// A brick class's sheet image and the sprite material drawing it.
#[derive(Clone)]
pub struct ClassSheet {
    pub image: Handle<Image>,
    pub material: Handle<SpriteParticle2dMaterial>,
}

/// One tint material per brick class and role, plus the smoke's.
#[derive(Resource, Clone)]
pub struct ParticleMaterials {
    pub glow: HashMap<BrickClass, Handle<ColorParticle2dMaterial>>,
    pub face: HashMap<BrickClass, Handle<ColorParticle2dMaterial>>,
    pub smoke: Handle<ColorParticle2dMaterial>,
    /// Each class's particle sheet, for its own hit and break bursts.
    pub sheets: HashMap<BrickClass, ClassSheet>,
    /// White: a class burst whose sheet is missing, drawn as plain quads in
    /// its `color_curve`'s colours.
    pub plain: Handle<ColorParticle2dMaterial>,
}

impl ParticleMaterials {
    fn glow(&self, class: BrickClass) -> Handle<ColorParticle2dMaterial> {
        self.glow.get(&class).cloned().unwrap_or_default()
    }

    fn face(&self, class: BrickClass) -> Handle<ColorParticle2dMaterial> {
        self.face.get(&class).cloned().unwrap_or_default()
    }
}

/// Every brick class, for building the per-class materials.
fn all_classes() -> [BrickClass; 9] {
    use crate::bricks::ExplosiveKind::*;
    [
        BrickClass::Ceramic,
        BrickClass::Titanium,
        BrickClass::Tungsten,
        BrickClass::Reactor,
        BrickClass::Explosive(Charge),
        BrickClass::Explosive(Breach),
        BrickClass::Explosive(Demolition),
        BrickClass::Regen,
        BrickClass::Shield,
    ]
}

/// Frames in each class sheet (one row).
const SHEET_FRAMES: u32 = 4;

/// The name a brick class's particle files go by. The three explosive
/// variants share `explosive`.
pub fn material_slug(class: BrickClass) -> &'static str {
    match class {
        BrickClass::Ceramic => "ceramic",
        BrickClass::Titanium => "titanium",
        BrickClass::Tungsten => "tungsten",
        BrickClass::Reactor => "reactor",
        BrickClass::Explosive(_) => "explosive",
        BrickClass::Regen => "regen",
        BrickClass::Shield => "shield",
    }
}

/// `particles/<class>_hit.particle.ron`.
pub fn class_hit_path(class: BrickClass) -> String {
    format!("particles/{}_hit.particle.ron", material_slug(class))
}

/// `particles/<class>_break.particle.ron`.
pub fn class_break_path(class: BrickClass) -> String {
    format!("particles/{}_break.particle.ron", material_slug(class))
}

/// `particles/<class>.png`, the class's 4-frame sheet.
pub fn class_sheet_path(class: BrickClass) -> String {
    format!("particles/{}.png", material_slug(class))
}

/// How a class hit/break burst is drawn, given whether its effect file
/// failed to load and whether its sheet image is loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BurstLook {
    /// The class's own effect, textured with its sheet.
    Textured,
    /// The class's own effect as plain quads (its sheet is missing).
    PlainQuads,
    /// pdp.1's generic effect, tinted by class (its effect file is missing).
    Generic,
}

pub fn burst_look(effect_failed: bool, sheet_loaded: bool) -> BurstLook {
    match (effect_failed, sheet_loaded) {
        (true, _) => BurstLook::Generic,
        (false, true) => BurstLook::Textured,
        (false, false) => BurstLook::PlainQuads,
    }
}

pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EnokiPlugin)
            .add_systems(Startup, load_effects)
            .add_systems(OnEnter(PlayState::Paused), freeze_particles)
            .add_systems(OnExit(PlayState::Paused), thaw_particles);
    }
}

fn load_effects(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorParticle2dMaterial>>,
    mut sprite_materials: ResMut<Assets<SpriteParticle2dMaterial>>,
) {
    let per_class = |path: fn(BrickClass) -> String| {
        all_classes()
            .into_iter()
            .map(|class| (class, assets.load(path(class))))
            .collect()
    };
    commands.insert_resource(ParticleEffects {
        hit: assets.load(HIT_PATH),
        smoke: assets.load(SMOKE_PATH),
        sparks: assets.load(SPARKS_PATH),
        shatter: assets.load(BREAK_PATH),
        class_hit: per_class(class_hit_path),
        class_break: per_class(class_break_path),
    });
    let sheets = all_classes()
        .into_iter()
        .map(|class| {
            let image: Handle<Image> = assets.load(class_sheet_path(class));
            let material = sprite_materials.add(SpriteParticle2dMaterial::new(
                image.clone(),
                SHEET_FRAMES,
                1,
            ));
            (class, ClassSheet { image, material })
        })
        .collect();
    let mut tint = |color: Color| materials.add(ColorParticle2dMaterial::new(color.into()));
    let glow = all_classes()
        .into_iter()
        .map(|class| (class, tint(theme::brick_glow(class))))
        .collect();
    let face = all_classes()
        .into_iter()
        .map(|class| (class, tint(theme::brick_color(class))))
        .collect();
    commands.insert_resource(ParticleMaterials {
        glow,
        face,
        smoke: tint(theme::SMOKE),
        sheets,
        plain: tint(theme::UNTINTED),
    });
}

/// bevy_enoki moves particles on `Time<Virtual>`; stopping it while paused
/// freezes every particle and spawner. (Gameplay systems gate on
/// `PlayState::Playing` and physics has its own clock, so nothing else
/// depends on virtual time advancing while paused.)
fn freeze_particles(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn thaw_particles(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

/// Marks a damaged brick's continuous emitter (a child of the brick).
#[derive(Component)]
pub struct DamageEmitter {
    role: EmitterRole,
    /// Fraction of the brick's hits taken, in (0, 1].
    damage: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EmitterRole {
    Smoke,
    Sparks,
}

/// Seconds between a damage emitter's spawn waves. It emits more often the
/// more damage the brick has taken (`damage` in (0, 1]), but never so often
/// that `emitters` of them would exceed [`DAMAGE_PARTICLE_BUDGET`] live
/// particles, given each wave's `particles_per_wave` and `lifetime`.
pub fn damage_emitter_interval(
    base: f32,
    damage: f32,
    emitters: usize,
    particles_per_wave: u32,
    lifetime: f32,
) -> f32 {
    let wanted = base / damage.max(0.05);
    // Live particles per emitter ≈ per_wave × lifetime / interval.
    let budgeted = particles_per_wave as f32 * lifetime * emitters as f32 / DAMAGE_PARTICLE_BUDGET;
    wanted.max(budgeted)
}

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spark_on_damage)
            .add_observer(shatter_on_break)
            .add_systems(
                Update,
                (sync_damage_emitters, tune_damage_emitters)
                    .chain()
                    .run_if(resource_exists::<ParticleEffects>)
                    .run_if(resource_exists::<ParticleMaterials>),
            );
    }
}

fn burst<M: Particle2dMaterial>(
    material: Handle<M>,
    effect: Handle<Particle2dEffect>,
    position: Vec2,
) -> impl Bundle {
    (
        ParticleSpawner(material),
        ParticleEffectHandle(effect),
        OneShot::Despawn,
        Transform::from_translation(position.extend(PARTICLE_Z)),
        DespawnOnExit(AppState::InGame),
    )
}

/// Which of a class's bursts to play.
#[derive(Clone, Copy)]
enum Burst {
    Hit,
    Break,
}

/// What a class burst needs to see whether its files loaded. Absent in
/// headless tests (no asset server state, no images).
type LoadInfo<'a> = (Option<Res<'a, AssetServer>>, Option<Res<'a, Assets<Image>>>);

/// Plays `class`'s own hit or break burst at `position`, or a fallback
/// ([`burst_look`]).
fn spawn_class_burst(
    commands: &mut Commands,
    effects: &ParticleEffects,
    materials: &ParticleMaterials,
    (server, images): &LoadInfo,
    class: BrickClass,
    which: Burst,
    position: Vec2,
) {
    let (own, generic, tint) = match which {
        Burst::Hit => (&effects.class_hit, &effects.hit, materials.glow(class)),
        Burst::Break => (
            &effects.class_break,
            &effects.shatter,
            materials.face(class),
        ),
    };
    let own = own.get(&class);
    let failed = own.is_none_or(|handle| {
        server
            .as_ref()
            .is_some_and(|server| server.get_load_state(handle).is_some_and(|s| s.is_failed()))
    });
    let sheet = materials.sheets.get(&class);
    let sheet_loaded = sheet.is_some_and(|sheet| {
        images
            .as_ref()
            .is_some_and(|images| images.contains(&sheet.image))
    });
    match (burst_look(failed, sheet_loaded), own, sheet) {
        (BurstLook::Textured, Some(own), Some(sheet)) => {
            commands.spawn(burst(sheet.material.clone(), own.clone(), position));
        }
        (BurstLook::PlainQuads, Some(own), _) => {
            commands.spawn(burst(materials.plain.clone(), own.clone(), position));
        }
        _ => {
            commands.spawn(burst(tint, generic.clone(), position));
        }
    }
}

fn spark_on_damage(
    on: On<BrickDamaged>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
    load: LoadInfo,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    let (class, position) = (on.class, on.position);
    spawn_class_burst(
        &mut commands,
        &effects,
        &materials,
        &load,
        class,
        Burst::Hit,
        position,
    );
}

fn shatter_on_break(
    on: On<BrickDestroyed>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
    load: LoadInfo,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    let (class, position) = (on.class, on.position);
    spawn_class_burst(
        &mut commands,
        &effects,
        &materials,
        &load,
        class,
        Burst::Break,
        position,
    );
}

/// Bricks whose health changed this frame.
type HealthChanged = (With<Brick>, Changed<BrickHealth>);

/// Keeps each brick's damage emitters in line with its health: added when it
/// drops below full, updated as it takes more, removed when it's back to
/// full (regen).
fn sync_damage_emitters(
    mut commands: Commands,
    effects: Res<ParticleEffects>,
    materials: Res<ParticleMaterials>,
    bricks: Query<(Entity, &BrickClass, &BrickHealth, Option<&Children>), HealthChanged>,
    mut emitters: Query<&mut DamageEmitter>,
) {
    for (brick, &class, health, children) in &bricks {
        let max = class.max_hits();
        let existing: Vec<Entity> = children
            .into_iter()
            .flatten()
            .copied()
            .filter(|child| emitters.contains(*child))
            .collect();
        if health.0 == 0 || health.0 >= max {
            for emitter in existing {
                commands.entity(emitter).despawn();
            }
            continue;
        }
        let damage = f32::from(max - health.0) / f32::from(max);
        if existing.is_empty() {
            // Local to the brick, just in front of it.
            let at = Transform::from_xyz(0.0, 0.0, PARTICLE_Z);
            commands.entity(brick).with_children(|brick| {
                brick.spawn((
                    DamageEmitter {
                        role: EmitterRole::Smoke,
                        damage,
                    },
                    ParticleSpawner(materials.smoke.clone()),
                    ParticleEffectHandle(effects.smoke.clone()),
                    at,
                ));
                brick.spawn((
                    DamageEmitter {
                        role: EmitterRole::Sparks,
                        damage,
                    },
                    ParticleSpawner(materials.glow(class)),
                    ParticleEffectHandle(effects.sparks.clone()),
                    at,
                ));
            });
        } else {
            for emitter in existing {
                if let Ok(mut emitter) = emitters.get_mut(emitter) {
                    emitter.damage = damage;
                }
            }
        }
    }
}

/// Sets each damage emitter's spawn interval from its brick's damage and the
/// particle budget. Runs every frame because bevy_enoki re-copies the effect
/// (and so the file's `spawn_rate`) on hot reload.
fn tune_damage_emitters(mut emitters: Query<(&DamageEmitter, &mut ParticleEffectInstance)>) {
    let count = emitters.iter().count();
    for (emitter, mut instance) in &mut emitters {
        let Some(effect) = instance.0.as_mut() else {
            continue;
        };
        let base = match emitter.role {
            EmitterRole::Smoke => SMOKE_INTERVAL,
            EmitterRole::Sparks => SPARKS_INTERVAL,
        };
        let lifetime = effect.lifetime.0 * (1.0 + effect.lifetime.1);
        let interval =
            damage_emitter_interval(base, emitter.damage, count, effect.spawn_amount, lifetime);
        if (effect.spawn_rate - interval).abs() > f32::EPSILON {
            effect.spawn_rate = interval;
        }
    }
}

#[cfg(test)]
mod tests;
