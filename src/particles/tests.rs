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
    for path in [
        HIT_PATH,
        SMOKE_PATH,
        SPARKS_PATH,
        BREAK_PATH,
        CHARGE_PATH,
        BREACH_PATH,
        DEMOLITION_PATH,
        DEBRIS_PATH,
        BLAST_SMOKE_PATH,
        GLINT_PATH,
    ] {
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
        charge: Handle::default(),
        breach: Handle::default(),
        demolition: Handle::default(),
        debris: Handle::default(),
        blast_smoke: Handle::default(),
        glint: Handle::default(),
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
        blast_red: Handle::default(),
        blast_orange: Handle::default(),
        debris: Handle::default(),
        glint: Handle::default(),
        glint_light: Handle::default(),
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

fn effect_file(path: &str) -> Particle2dEffect {
    let text = std::fs::read_to_string(format!("assets/{path}")).unwrap();
    ron::de::from_str(&text).unwrap()
}

#[test]
fn each_explosive_variant_has_a_blast_shaped_like_its_rule() {
    use BlastEffect as E;
    // Charge: a round burst, red and orange.
    let charge = blast_parts(ExplosiveKind::Charge);
    assert_eq!(
        charge,
        [
            (E::Charge, BlastTint::Red, 0.0),
            (E::Charge, BlastTint::Orange, 0.0)
        ]
    );
    assert_eq!(
        effect_file(CHARGE_PATH).direction.unwrap().1,
        1.0,
        "all round"
    );

    // Breach: four narrow jets, a quarter turn apart: a "+".
    let breach = blast_parts(ExplosiveKind::Breach);
    assert_eq!(breach.len(), 4);
    assert!(breach.iter().all(|(e, _, _)| *e == E::Breach));
    let mut quarters: Vec<i32> = breach
        .iter()
        .map(|(_, _, a)| (a / std::f32::consts::FRAC_PI_2).round() as i32)
        .collect();
    quarters.sort();
    assert_eq!(quarters, [0, 1, 2, 3]);
    assert!(
        effect_file(BREACH_PATH).direction.unwrap().1 < 0.05,
        "narrow"
    );

    // Demolition: bigger than a charge, with debris and lingering smoke.
    let demolition = blast_parts(ExplosiveKind::Demolition);
    let effects: Vec<E> = demolition.iter().map(|(e, _, _)| *e).collect();
    assert!(effects.contains(&E::Demolition));
    assert!(effects.contains(&E::Debris));
    assert!(effects.contains(&E::Smoke));
    let (big, small) = (effect_file(DEMOLITION_PATH), effect_file(CHARGE_PATH));
    assert!(big.spawn_amount > small.spawn_amount);
    assert!(big.linear_speed.unwrap().0 > small.linear_speed.unwrap().0);
    let smoke = effect_file(BLAST_SMOKE_PATH);
    assert!(smoke.lifetime.0 > 2.0 * big.lifetime.0, "lingers");
}

#[derive(Resource, Default)]
struct Explosions(Vec<ExplosiveKind>);

#[test]
fn every_explosion_in_a_chain_plays_its_blast() {
    for kind in [
        ExplosiveKind::Charge,
        ExplosiveKind::Breach,
        ExplosiveKind::Demolition,
    ] {
        let mut app = app_with_vfx();
        app.init_resource::<Explosions>().add_observer(
            |on: On<BrickExploded>, mut seen: ResMut<Explosions>| seen.0.push(on.kind),
        );
        let explosive = brick_of(&mut app, BrickClass::Explosive(kind));
        hit(&mut app, explosive);
        let exploded = app.world().resource::<Explosions>().0.clone();
        assert_eq!(exploded[0], kind, "the ball-hit brick goes first");
        // A random board may chain into more explosives; each plays its own.
        let parts: usize = exploded.iter().map(|k| blast_parts(*k).len()).sum();
        assert_eq!(count::<With<BlastBurst>>(&mut app), parts, "{kind:?}");
    }
}

#[test]
fn shield_glass_glints_when_it_deflects_and_shatters_when_it_breaks() {
    let mut app = app_with_vfx();
    let shield = brick_of(&mut app, BrickClass::Shield);
    hit_moving(&mut app, shield, Vec2::new(0.0, 450.0));
    assert_eq!(count::<With<GlassGlint>>(&mut app), 2, "glints, both tones");
    assert_eq!(bursts(&mut app), 2, "nothing else");

    hit_moving(&mut app, shield, Vec2::new(0.0, -450.0));
    assert!(app.world().get_entity(shield).is_err());
    assert_eq!(count::<With<GlassGlint>>(&mut app), 2, "no new glints");
    assert_eq!(bursts(&mut app), 3, "the glass shatter");
}

#[test]
fn without_the_vfx_resources_nothing_spawns() {
    let mut app = app();
    let tungsten = brick_of(&mut app, BrickClass::Tungsten);
    hit(&mut app, tungsten);
    let shield = brick_of(&mut app, BrickClass::Shield);
    hit_moving(&mut app, shield, Vec2::new(0.0, 450.0));
    let charge = brick_of(&mut app, BrickClass::Explosive(ExplosiveKind::Charge));
    hit(&mut app, charge);
    app.update();
    assert_eq!(count::<Spawners>(&mut app), 0);
}
