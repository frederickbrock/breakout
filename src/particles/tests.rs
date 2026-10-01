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
        bounce: Handle::default(),
        flare: Handle::default(),
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
fn without_the_vfx_resources_nothing_spawns() {
    let mut app = app();
    let tungsten = brick_of(&mut app, BrickClass::Tungsten);
    hit(&mut app, tungsten);
    let paddle = paddle(&mut app);
    hit(&mut app, paddle);
    app.update();
    assert_eq!(count::<Spawners>(&mut app), 0);
}
