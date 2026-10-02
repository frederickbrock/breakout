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
        class_hit: all_classes()
            .into_iter()
            .map(|c| (c, effect_handle(c, Burst::Hit)))
            .collect(),
        class_break: all_classes()
            .into_iter()
            .map(|c| (c, effect_handle(c, Burst::Break)))
            .collect(),
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
        sheets: HashMap::default(),
        plain: Handle::default(),
    });
    app
}

/// A distinct placeholder per class material and burst (variants share).
fn effect_handle(class: BrickClass, which: Burst) -> Handle<Particle2dEffect> {
    let slug = all_classes()
        .iter()
        .position(|c| material_slug(*c) == material_slug(class))
        .unwrap() as u128;
    let id = 1 + slug * 2 + matches!(which, Burst::Break) as u128;
    Handle::Uuid(bevy::asset::uuid::Uuid::from_u128(id), Default::default())
}

/// The effect each one-shot spawner plays.
fn burst_effects(app: &mut App) -> Vec<Handle<Particle2dEffect>> {
    app.world_mut()
        .query_filtered::<&ParticleEffectHandle, With<OneShot>>()
        .iter(app.world())
        .map(|h| h.0.clone())
        .collect()
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

const CLASSES: [BrickClass; 7] = [
    BrickClass::Ceramic,
    BrickClass::Titanium,
    BrickClass::Tungsten,
    BrickClass::Reactor,
    BrickClass::Explosive(crate::bricks::ExplosiveKind::Charge),
    BrickClass::Regen,
    BrickClass::Shield,
];

#[test]
fn every_class_has_its_own_hit_break_and_sheet_files_on_disk() {
    let slugs: std::collections::HashSet<_> = all_classes().map(material_slug).into();
    assert_eq!(slugs.len(), 7, "explosive variants share one material");
    for class in all_classes() {
        for path in [class_hit_path(class), class_break_path(class)] {
            let text = std::fs::read_to_string(format!("assets/{path}"))
                .unwrap_or_else(|e| panic!("{path}: {e}"));
            let effect: Particle2dEffect =
                ron::de::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert!(effect.spawn_amount > 0, "{path}");
            assert!(effect.color_curve.is_some(), "{path} colours itself");
        }
        let sheet = class_sheet_path(class);
        assert!(
            std::path::Path::new(&format!("assets/{sheet}")).exists(),
            "{sheet}"
        );
    }
}

#[test]
fn a_missing_sheet_falls_back_to_plain_quads_and_a_missing_effect_to_generic() {
    assert_eq!(burst_look(false, true), BurstLook::Textured);
    assert_eq!(burst_look(false, false), BurstLook::PlainQuads);
    assert_eq!(burst_look(true, true), BurstLook::Generic);
    assert_eq!(burst_look(true, false), BurstLook::Generic);
}

#[test]
fn each_class_plays_its_own_hit_and_break() {
    for class in CLASSES {
        let mut app = app_with_vfx();
        let brick = brick_of(&mut app, class);
        let hits = class.max_hits();
        for _ in 1..hits {
            hit_moving(&mut app, brick, Vec2::new(0.0, -450.0));
        }
        hit_moving(&mut app, brick, Vec2::new(0.0, -450.0));
        let played = burst_effects(&mut app);
        // A brick that can survive a hit plays its hit burst first; the last
        // hit plays its break. An explosive's blast may break (and so burst)
        // others too, so look for this class's own.
        if hits > 1 {
            assert!(
                played.contains(&effect_handle(class, Burst::Hit)),
                "{class:?}"
            );
        }
        assert!(
            played.contains(&effect_handle(class, Burst::Break)),
            "{class:?}"
        );
        assert!(
            !played.contains(&Handle::default()),
            "{class:?}: no generic"
        );
    }
    let tungsten = effect_handle(BrickClass::Tungsten, Burst::Hit);
    assert_ne!(tungsten, effect_handle(BrickClass::Titanium, Burst::Hit));
}

#[test]
fn a_class_without_its_effect_file_plays_the_generic_burst() {
    let mut app = app_with_vfx();
    app.world_mut()
        .resource_mut::<ParticleEffects>()
        .class_break
        .remove(&BrickClass::Ceramic);
    let ceramic = brick_of(&mut app, BrickClass::Ceramic);
    hit(&mut app, ceramic);
    assert_eq!(burst_effects(&mut app), [Handle::default()]);
}

#[test]
fn a_loaded_sheet_draws_the_burst_textured() {
    type Textured = With<ParticleSpawner<SpriteParticle2dMaterial>>;
    let mut app = app_with_vfx();
    app.init_asset::<Image>()
        .init_asset::<SpriteParticle2dMaterial>();
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let material = app
        .world_mut()
        .resource_mut::<Assets<SpriteParticle2dMaterial>>()
        .add(SpriteParticle2dMaterial::new(
            image.clone(),
            SHEET_FRAMES,
            1,
        ));
    app.world_mut()
        .resource_mut::<ParticleMaterials>()
        .sheets
        .insert(BrickClass::Ceramic, ClassSheet { image, material });

    let ceramic = brick_of(&mut app, BrickClass::Ceramic);
    hit(&mut app, ceramic);
    assert_eq!(count::<Textured>(&mut app), 1);
    assert_eq!(bursts(&mut app), 0, "not a plain-quad burst");

    // Torn down with the run like every other spawner.
    tap(&mut app, KeyCode::Escape);
    crate::menu::test_helpers::press(&mut app, "Main menu");
    assert_eq!(count::<Textured>(&mut app), 0);
}

#[test]
fn without_the_vfx_resources_nothing_spawns() {
    let mut app = app();
    let tungsten = brick_of(&mut app, BrickClass::Tungsten);
    hit(&mut app, tungsten);
    app.update();
    assert_eq!(count::<Spawners>(&mut app), 0);
}
