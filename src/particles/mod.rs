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
//! | `BrickDestroyed` | `brick_break` shatter burst at the brick's centre | class face |
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

use crate::bricks::BrickClass;
use crate::game_state::{AppState, PlayState};
use crate::{theme, Brick, BrickDamaged, BrickDestroyed, BrickHealth};
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
}

/// One tint material per brick class and role, plus the smoke's.
#[derive(Resource, Clone)]
pub struct ParticleMaterials {
    pub glow: HashMap<BrickClass, Handle<ColorParticle2dMaterial>>,
    pub face: HashMap<BrickClass, Handle<ColorParticle2dMaterial>>,
    pub smoke: Handle<ColorParticle2dMaterial>,
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
    let smoke = tint(theme::SMOKE);
    commands.insert_resource(ParticleMaterials { glow, face, smoke });
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

fn burst(
    material: Handle<ColorParticle2dMaterial>,
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
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn heavier_damage_emits_more_often() {
        let light = damage_emitter_interval(SMOKE_INTERVAL, 1.0 / 3.0, 1, 1, 1.5);
        let heavy = damage_emitter_interval(SMOKE_INTERVAL, 2.0 / 3.0, 1, 1, 1.5);
        assert!(
            heavy < light,
            "tungsten at 1/3 hits smokes more than at 2/3"
        );
    }

    #[test]
    fn the_budget_slows_emitters_when_many_bricks_are_damaged() {
        let (per_wave, lifetime) = (2, 1.5);
        for emitters in [1, 70, 140, 1000] {
            let interval =
                damage_emitter_interval(SMOKE_INTERVAL, 1.0, emitters, per_wave, lifetime);
            let live = emitters as f32 * per_wave as f32 * lifetime / interval;
            assert!(live <= DAMAGE_PARTICLE_BUDGET + 1.0, "{emitters}: {live}");
        }
        // A handful of emitters aren't throttled.
        assert_eq!(
            damage_emitter_interval(SMOKE_INTERVAL, 1.0, 4, per_wave, lifetime),
            SMOKE_INTERVAL
        );
    }

    #[test]
    fn the_shipped_effect_files_parse() {
        for path in [HIT_PATH, SMOKE_PATH, SPARKS_PATH, BREAK_PATH] {
            let text = std::fs::read_to_string(format!("assets/{path}")).unwrap();
            let effect: Particle2dEffect =
                ron::de::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert!(effect.spawn_amount > 0, "{path}");
        }
    }

    /// A run with the VFX resources present (placeholder handles: no renderer
    /// in tests), so the observers spawn spawner entities we can count.
    fn app_with_vfx() -> App {
        let mut app = app();
        app.world_mut().insert_resource(ParticleEffects {
            hit: Handle::default(),
            smoke: Handle::default(),
            sparks: Handle::default(),
            shatter: Handle::default(),
        });
        let glow = all_classes()
            .into_iter()
            .map(|c| (c, Handle::default()))
            .collect();
        let face = all_classes()
            .into_iter()
            .map(|c| (c, Handle::default()))
            .collect();
        app.world_mut().insert_resource(ParticleMaterials {
            glow,
            face,
            smoke: Handle::default(),
        });
        app
    }

    type Spawners = With<ParticleSpawner<ColorParticle2dMaterial>>;

    fn bursts(app: &mut App) -> usize {
        count::<(Spawners, With<OneShot>)>(app)
    }

    fn damage_emitters_of(app: &mut App, brick: Entity) -> Vec<(EmitterRole, f32)> {
        let children: Vec<Entity> = app
            .world()
            .get::<Children>(brick)
            .map(|c| c.to_vec())
            .unwrap_or_default();
        children
            .into_iter()
            .filter_map(|c| app.world().get::<DamageEmitter>(c))
            .map(|e| (e.role, e.damage))
            .collect()
    }

    #[test]
    fn a_surviving_hit_fires_a_spark_burst_and_starts_smoking() {
        let mut app = app_with_vfx();
        let tungsten = brick_of(&mut app, BrickClass::Tungsten);
        hit(&mut app, tungsten);
        assert_eq!(bursts(&mut app), 1, "spark burst");
        app.update();
        let emitters = damage_emitters_of(&mut app, tungsten);
        assert_eq!(emitters.len(), 2, "smoke + sparks");
        assert!(emitters.iter().all(|(_, d)| (*d - 1.0 / 3.0).abs() < 1e-6));

        // Heavier damage: same emitters, higher damage.
        hit(&mut app, tungsten);
        app.update();
        let emitters = damage_emitters_of(&mut app, tungsten);
        assert_eq!(emitters.len(), 2);
        assert!(emitters.iter().all(|(_, d)| (*d - 2.0 / 3.0).abs() < 1e-6));
    }

    #[test]
    fn a_damaged_brick_keeps_its_intact_colour() {
        let mut app = app_with_vfx();
        let titanium = brick_of(&mut app, BrickClass::Titanium);
        hit(&mut app, titanium);
        app.update();
        assert_eq!(
            app.world().get::<Sprite>(titanium).unwrap().color,
            theme::TITANIUM
        );
    }

    #[test]
    fn breaking_a_brick_fires_a_shatter_burst() {
        let mut app = app_with_vfx();
        let ceramic = brick_of(&mut app, BrickClass::Ceramic);
        hit(&mut app, ceramic);
        assert!(app.world().get_entity(ceramic).is_err());
        assert_eq!(bursts(&mut app), 1, "shatter burst");
    }

    #[test]
    fn a_healed_regen_brick_stops_smoking() {
        let mut app = app_with_vfx();
        let regen = brick_of(&mut app, BrickClass::Regen);
        hit(&mut app, regen);
        app.update();
        assert_eq!(damage_emitters_of(&mut app, regen).len(), 2);
        for _ in 0..35 {
            app.update(); // past the 3 s heal
        }
        assert_eq!(app.world().get::<BrickHealth>(regen).unwrap().0, 2);
        assert!(damage_emitters_of(&mut app, regen).is_empty());
    }

    #[test]
    fn leaving_the_run_leaves_no_particle_spawners() {
        let mut app = app_with_vfx();
        let tungsten = brick_of(&mut app, BrickClass::Tungsten);
        let ceramic = brick_of(&mut app, BrickClass::Ceramic);
        hit(&mut app, tungsten);
        hit(&mut app, ceramic);
        app.update();
        assert!(count::<Spawners>(&mut app) >= 4, "bursts + emitters");

        tap(&mut app, KeyCode::Escape);
        crate::menu::test_helpers::press(&mut app, "Main menu");
        assert_eq!(app_state(&app), AppState::MainMenu);
        assert_eq!(count::<Spawners>(&mut app), 0);
    }

    #[test]
    fn without_the_vfx_resources_nothing_spawns() {
        let mut app = app();
        let tungsten = brick_of(&mut app, BrickClass::Tungsten);
        hit(&mut app, tungsten);
        app.update();
        assert_eq!(count::<Spawners>(&mut app), 0);
    }
}
