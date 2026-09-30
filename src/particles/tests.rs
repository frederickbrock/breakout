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
        let interval = damage_emitter_interval(SMOKE_INTERVAL, 1.0, emitters, per_wave, lifetime);
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
