//! The VFX layer: bevy_enoki particles for brick damage.
//!
//! **Gameplay fires events, particles observe.** Gameplay code never touches
//! particles. It triggers `BrickDamaged` / `BrickDestroyed` and changes
//! `BrickHealth`, and this module reacts:
//!
//! | trigger | effect file (`assets/particles/`) | colour |
//! |---|---|---|
//! | `BrickDamaged` | `brick_hit` spark burst at the contact point | class glow |
//! | `BrickHealth` below max | child emitters: `brick_damage_smoke` + `brick_damage_sparks`, heavier with more damage | smoke grey / class glow |
//! | `BrickHealth` back at max (regen heal) | those child emitters removed | — |
//! | `BrickDestroyed` | `brick_break` shatter burst at the brick's centre | class face (glass cyan for shield) |
//! | `BrickExploded` | the variant's blast ([`blast_parts`]): charge round, breach a "+" of four jets, demolition big + debris + lingering smoke | `BLAST_RED` / `BLAST_ORANGE` / `BLAST_DEBRIS` / smoke |
//! | `ShieldDeflected` | `glass_glint` burst at the contact point | `GLASS_GLINT` + `GLASS_GLINT_LIGHT` |
//!
//! Effect files are drawn white; the colour comes from the spawner's
//! `ColorParticle2dMaterial` (one per class and role, [`ParticleMaterials`]),
//! which multiplies it. Editing a `.particle.ron` while `cargo run` is live
//! hot-reloads it.
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

use crate::bricks::explosive::BrickExploded;
use crate::bricks::grid::{Brick, BrickHealth};
use crate::bricks::{BrickClass, ExplosiveKind};
use crate::collision::{BrickDamaged, BrickDestroyed, ShieldDeflected};
use crate::game_state::{AppState, PlayState};
use crate::theme;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_enoki::prelude::*;

const HIT_PATH: &str = "particles/brick_hit.particle.ron";
const SMOKE_PATH: &str = "particles/brick_damage_smoke.particle.ron";
const SPARKS_PATH: &str = "particles/brick_damage_sparks.particle.ron";
const BREAK_PATH: &str = "particles/brick_break.particle.ron";
const CHARGE_PATH: &str = "particles/blast_charge.particle.ron";
const BREACH_PATH: &str = "particles/blast_breach.particle.ron";
const DEMOLITION_PATH: &str = "particles/blast_demolition.particle.ron";
const DEBRIS_PATH: &str = "particles/blast_debris.particle.ron";
const BLAST_SMOKE_PATH: &str = "particles/blast_smoke.particle.ron";
const GLINT_PATH: &str = "particles/glass_glint.particle.ron";

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
    pub charge: Handle<Particle2dEffect>,
    pub breach: Handle<Particle2dEffect>,
    pub demolition: Handle<Particle2dEffect>,
    pub debris: Handle<Particle2dEffect>,
    pub blast_smoke: Handle<Particle2dEffect>,
    pub glint: Handle<Particle2dEffect>,
}

impl ParticleEffects {
    fn blast(&self, effect: BlastEffect) -> Handle<Particle2dEffect> {
        match effect {
            BlastEffect::Charge => &self.charge,
            BlastEffect::Breach => &self.breach,
            BlastEffect::Demolition => &self.demolition,
            BlastEffect::Debris => &self.debris,
            BlastEffect::Smoke => &self.blast_smoke,
        }
        .clone()
    }
}

/// One tint material per brick class and role, plus the smoke's, the
/// blasts' and the glass glints'.
#[derive(Resource, Clone)]
pub struct ParticleMaterials {
    pub glow: HashMap<BrickClass, Handle<ColorParticle2dMaterial>>,
    pub face: HashMap<BrickClass, Handle<ColorParticle2dMaterial>>,
    pub smoke: Handle<ColorParticle2dMaterial>,
    pub blast_red: Handle<ColorParticle2dMaterial>,
    pub blast_orange: Handle<ColorParticle2dMaterial>,
    pub debris: Handle<ColorParticle2dMaterial>,
    pub glint: Handle<ColorParticle2dMaterial>,
    pub glint_light: Handle<ColorParticle2dMaterial>,
}

impl ParticleMaterials {
    fn glow(&self, class: BrickClass) -> Handle<ColorParticle2dMaterial> {
        self.glow.get(&class).cloned().unwrap_or_default()
    }

    fn face(&self, class: BrickClass) -> Handle<ColorParticle2dMaterial> {
        self.face.get(&class).cloned().unwrap_or_default()
    }

    fn blast(&self, tint: BlastTint) -> Handle<ColorParticle2dMaterial> {
        match tint {
            BlastTint::Red => &self.blast_red,
            BlastTint::Orange => &self.blast_orange,
            BlastTint::Debris => &self.debris,
            BlastTint::Smoke => &self.smoke,
        }
        .clone()
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
) {
    commands.insert_resource(ParticleEffects {
        hit: assets.load(HIT_PATH),
        smoke: assets.load(SMOKE_PATH),
        sparks: assets.load(SPARKS_PATH),
        shatter: assets.load(BREAK_PATH),
        charge: assets.load(CHARGE_PATH),
        breach: assets.load(BREACH_PATH),
        demolition: assets.load(DEMOLITION_PATH),
        debris: assets.load(DEBRIS_PATH),
        blast_smoke: assets.load(BLAST_SMOKE_PATH),
        glint: assets.load(GLINT_PATH),
    });
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
        blast_red: tint(theme::BLAST_RED),
        blast_orange: tint(theme::BLAST_ORANGE),
        debris: tint(theme::BLAST_DEBRIS),
        glint: tint(theme::GLASS_GLINT),
        glint_light: tint(theme::GLASS_GLINT_LIGHT),
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
            .add_observer(blast_on_explosion)
            .add_observer(glint_on_deflect)
            .add_systems(
                Update,
                (sync_damage_emitters, tune_damage_emitters)
                    .chain()
                    .run_if(resource_exists::<ParticleEffects>)
                    .run_if(resource_exists::<ParticleMaterials>),
            );
    }
}

fn burst(
    material: Handle<ColorParticle2dMaterial>,
    effect: Handle<Particle2dEffect>,
    position: Vec2,
) -> impl Bundle {
    burst_at(
        material,
        effect,
        Transform::from_translation(position.extend(PARTICLE_Z)),
    )
}

/// A one-shot burst placed (and turned) by `transform`.
fn burst_at(
    material: Handle<ColorParticle2dMaterial>,
    effect: Handle<Particle2dEffect>,
    transform: Transform,
) -> impl Bundle {
    (
        ParticleSpawner(material),
        ParticleEffectHandle(effect),
        OneShot::Despawn,
        transform,
        DespawnOnExit(AppState::InGame),
    )
}

fn spark_on_damage(
    on: On<BrickDamaged>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    commands.spawn(burst(
        materials.glow(on.class),
        effects.hit.clone(),
        on.position,
    ));
}

fn shatter_on_break(
    on: On<BrickDestroyed>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    commands.spawn(burst(
        materials.face(on.class),
        effects.shatter.clone(),
        on.position,
    ));
}

/// The effect files a blast is built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlastEffect {
    Charge,
    Breach,
    Demolition,
    Debris,
    Smoke,
}

/// A blast part's tint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlastTint {
    Red,
    Orange,
    Debris,
    Smoke,
}

/// One spawner of a blast: its effect, its tint, and the angle (radians)
/// it's turned to (the effect file's direction is rotated by it).
pub type BlastPart = (BlastEffect, BlastTint, f32);

/// A `kind` explosion's blast, shaped like its rule: charge a round
/// red/orange burst over its 8 neighbours, breach four jets along the
/// orthogonals (a "+"), demolition a big burst with debris and lingering
/// smoke.
pub fn blast_parts(kind: ExplosiveKind) -> Vec<BlastPart> {
    use std::f32::consts::FRAC_PI_2;
    use BlastEffect as E;
    use BlastTint as T;
    match kind {
        ExplosiveKind::Charge => vec![(E::Charge, T::Red, 0.0), (E::Charge, T::Orange, 0.0)],
        ExplosiveKind::Breach => (0..4)
            .map(|quarter| {
                let tint = if quarter % 2 == 0 { T::Red } else { T::Orange };
                (E::Breach, tint, quarter as f32 * FRAC_PI_2)
            })
            .collect(),
        ExplosiveKind::Demolition => vec![
            (E::Demolition, T::Red, 0.0),
            (E::Demolition, T::Orange, 0.0),
            (E::Debris, T::Debris, 0.0),
            (E::Smoke, T::Smoke, 0.0),
        ],
    }
}

/// Marks a blast spawner.
#[derive(Component)]
pub struct BlastBurst;

/// Each explosion, the chain's included (one `BrickExploded` per exploding
/// brick, in chain order), plays its variant's blast where it went off.
fn blast_on_explosion(
    on: On<BrickExploded>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    for (effect, tint, angle) in blast_parts(on.kind) {
        let at = Transform::from_translation(on.position.extend(PARTICLE_Z))
            .with_rotation(Quat::from_rotation_z(angle));
        commands.spawn((
            BlastBurst,
            burst_at(materials.blast(tint), effects.blast(effect), at),
        ));
    }
}

/// Marks a glass-glint spawner.
#[derive(Component)]
pub struct GlassGlint;

/// Shield glass deflected the ball: cyan glints at the contact point, in
/// its two glass tones.
fn glint_on_deflect(
    on: On<ShieldDeflected>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    for material in [&materials.glint, &materials.glint_light] {
        commands.spawn((
            GlassGlint,
            burst(material.clone(), effects.glint.clone(), on.position),
        ));
    }
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
