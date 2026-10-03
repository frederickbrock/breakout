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
        TRAIL_PATH,
        BOUNCE_PATH,
        FLARE_PATH,
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
        trail: Handle::default(),
        bounce: bounce_handle(),
        flare: Handle::default(),
        charge: Handle::default(),
        breach: Handle::default(),
        demolition: Handle::default(),
        debris: Handle::default(),
        blast_smoke: Handle::default(),
        glint: Handle::default(),
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
        trail: Handle::default(),
        bounce: Handle::default(),
        flare: Handle::default(),
        blast_red: Handle::default(),
        blast_orange: Handle::default(),
        debris: Handle::default(),
        glint: Handle::default(),
        glint_light: Handle::default(),
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

/// The placeholder for the ball's bounce sparks, distinct from every brick
/// burst (and from the generic ones' `Handle::default()`).
fn bounce_handle() -> Handle<Particle2dEffect> {
    Handle::Uuid(bevy::asset::uuid::Uuid::from_u128(1000), Default::default())
}

/// The effect each one-shot brick burst plays (the ball's bounce sparks,
/// which every brick hit also fires, and blasts and glass glints, which
/// have their own markers, are left out).
fn burst_effects(app: &mut App) -> Vec<Handle<Particle2dEffect>> {
    app.world_mut()
        .query_filtered::<&ParticleEffectHandle, (With<OneShot>, Without<BlastBurst>, Without<GlassGlint>)>()
        .iter(app.world())
        .map(|h| h.0.clone())
        .filter(|h| *h != bounce_handle())
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
    assert_eq!(bursts(&mut app), 2, "class spark burst + bounce sparks");
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
    assert_eq!(bursts(&mut app), 2, "shatter burst + bounce sparks");
}

#[test]
fn a_healed_regen_brick_stops_smoking() {
    let mut app = app_with_vfx();
    let regen = brick_of(&mut app, BrickClass::Regen);
    hit(&mut app, regen);
    app.update();
    assert_eq!(damage_emitters_of(&mut app, regen).len(), 2);
    // Past the heal (100 ms frames).
    for _ in 0..(crate::bricks::regen::HEAL_SECS * 10.0).round() as usize + 5 {
        app.update();
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

/// Whether each of the ball's trail emitters is active.
fn trail_states(app: &mut App) -> Vec<bool> {
    let ball = ball(app);
    app.world_mut()
        .query_filtered::<(&ParticleSpawnerState, &ChildOf), With<BallTrail>>()
        .iter(app.world())
        .filter(|(_, child_of)| child_of.parent() == ball)
        .map(|(state, _)| state.active)
        .collect()
}

#[test]
fn the_ball_trails_only_while_in_flight() {
    let mut app = app_with_vfx();
    app.update();
    assert_eq!(trail_states(&mut app), [false], "anchored: no trail");

    tap(&mut app, KeyCode::Space);
    assert_eq!(trail_states(&mut app), [true], "served: trailing");

    // Lost the ball: back on the paddle, the trail stops (and isn't doubled).
    move_ball_below_screen(&mut app);
    app.update();
    app.update();
    assert_eq!(trail_states(&mut app), [false]);
}

#[test]
fn a_wall_bounce_fires_a_spark_burst() {
    let mut app = app_with_vfx();
    let wall = app
        .world_mut()
        .query_filtered::<Entity, With<crate::world::Wall>>()
        .iter(app.world())
        .next()
        .unwrap();
    hit(&mut app, wall);
    assert_eq!(bursts(&mut app), 1, "bounce sparks");
}

fn flares(app: &mut App) -> Vec<(Entity, Transform)> {
    app.world_mut()
        .query_filtered::<(&ChildOf, &Transform), With<PaddleFlare>>()
        .iter(app.world())
        .map(|(child_of, t)| (child_of.parent(), *t))
        .collect()
}

#[test]
fn a_paddle_hit_flares_both_ways_along_the_top_edge() {
    let mut app = app_with_vfx();
    let paddle = paddle(&mut app);
    // The anchored ball sits over the paddle's centre.
    hit(&mut app, paddle);
    // Bounce sparks, plus a flare running to each end.
    assert_eq!(
        count::<(Spawners, With<OneShot>, Without<PaddleFlare>)>(&mut app),
        1,
        "bounce sparks"
    );
    let flares = flares(&mut app);
    assert_eq!(flares.len(), 2);
    for (parent, transform) in &flares {
        assert_eq!(*parent, paddle, "rides with the paddle");
        assert_eq!(transform.translation.y, PADDLE_HEIGHT / 2.0, "top edge");
        assert!(transform.translation.x.abs() < 1e-3, "from the hit point");
    }
    let rightward: Vec<f32> = flares
        .iter()
        .map(|(_, t)| (t.rotation * Vec3::X).x.round())
        .collect();
    assert!(rightward.contains(&1.0) && rightward.contains(&-1.0));
}

#[test]
fn a_hit_at_the_paddle_end_flares_one_way() {
    let mut app = app_with_vfx();
    let paddle = paddle(&mut app);
    let ball = ball(&mut app);
    app.world_mut()
        .get_mut::<Transform>(ball)
        .unwrap()
        .translation
        .x += crate::paddle::PADDLE_WIDTH;
    hit(&mut app, paddle);
    let flares = flares(&mut app);
    assert_eq!(flares.len(), 1);
    assert_eq!((flares[0].1.rotation * Vec3::X).x.round(), -1.0);
}

#[test]
fn the_flare_reaches_each_end_of_a_widened_paddle() {
    use crate::paddle::PADDLE_WIDTH;
    assert_eq!(
        flare_reaches(10.0, 0.0, PADDLE_WIDTH),
        [
            (-1.0, PADDLE_WIDTH / 2.0 + 10.0),
            (1.0, PADDLE_WIDTH / 2.0 - 10.0)
        ]
    );
    // Super-Sizer doubles the width: the flare runs twice as far.
    let wide = PADDLE_WIDTH * 2.0;
    assert_eq!(
        flare_reaches(0.0, 0.0, wide),
        [(-1.0, wide / 2.0), (1.0, wide / 2.0)]
    );

    let base = effect_file(FLARE_PATH);
    let normal = tuned_flare(&base, PADDLE_WIDTH / 2.0);
    let widened = tuned_flare(&base, PADDLE_WIDTH);
    let speed = |e: &Particle2dEffect| e.linear_speed.as_ref().unwrap().0;
    // The fastest particle (×2) covers the reach over the base lifetime.
    assert!((speed(&normal) * 2.0 * base.lifetime.0 - PADDLE_WIDTH / 2.0).abs() < 1e-3);
    assert!((speed(&widened) - 2.0 * speed(&normal)).abs() < 1e-3);
    assert_eq!(normal.spawn_amount, base.spawn_amount);
    assert_eq!(widened.spawn_amount, 2 * base.spawn_amount);
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
fn shield_glass_glints_when_it_deflects_and_shatters_when_it_breaks() {
    let mut app = app_with_vfx();
    let shield = brick_of(&mut app, BrickClass::Shield);
    hit_moving(&mut app, shield, Vec2::new(0.0, 450.0));
    assert_eq!(count::<With<GlassGlint>>(&mut app), 2, "glints, both tones");
    assert_eq!(bursts(&mut app), 3, "plus the bounce sparks, nothing else");

    hit_moving(&mut app, shield, Vec2::new(0.0, -450.0));
    assert!(app.world().get_entity(shield).is_err());
    assert_eq!(count::<With<GlassGlint>>(&mut app), 2, "no new glints");
    assert_eq!(
        bursts(&mut app),
        5,
        "the glass shatter and its bounce sparks"
    );
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
    assert_eq!(
        bursts(&mut app),
        1,
        "no plain-quad brick burst, just the bounce sparks"
    );

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
    let paddle = paddle(&mut app);
    hit(&mut app, paddle);
    let shield = brick_of(&mut app, BrickClass::Shield);
    hit_moving(&mut app, shield, Vec2::new(0.0, 450.0));
    let charge = brick_of(&mut app, BrickClass::Explosive(ExplosiveKind::Charge));
    hit(&mut app, charge);
    app.update();
    assert_eq!(count::<Spawners>(&mut app), 0);
}
