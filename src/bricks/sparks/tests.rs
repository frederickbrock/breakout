use super::*;
use crate::bricks::BrickClass;
use crate::test_support::*;

const PATH: Vec2 = Vec2::new(200.0, 80.0);

/// How far `p` is from the outline of the rounded rect `PATH` / `PATH_RADIUS`.
fn off_path(p: Vec2) -> f32 {
    let half = PATH / 2.0;
    let r = PATH_RADIUS;
    let q = p.abs() - (half - Vec2::splat(r));
    (q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - r).abs()
}

#[test]
fn the_path_is_a_closed_rounded_rect() {
    let half = PATH / 2.0;
    let start = perimeter_point(PATH, PATH_RADIUS, 0.0);
    assert!((start - Vec2::new(-half.x + PATH_RADIUS, half.y)).length() < 1e-3);
    let mut prev = start;
    let steps = 400;
    for i in 1..=steps {
        let p = perimeter_point(PATH, PATH_RADIUS, i as f32 / steps as f32);
        assert!(
            off_path(p) < 1e-2,
            "lap {}: {p} is off the path",
            i as f32 / steps as f32
        );
        // Even steps by arc length: no jumps anywhere round the loop.
        assert!((p - prev).length() < 2.0, "jump at step {i}: {prev} -> {p}");
        prev = p;
    }
    assert!((prev - start).length() < 1e-2, "it closes");
    // Halfway round is the opposite side.
    let mid = perimeter_point(PATH, PATH_RADIUS, 0.5);
    assert!((mid + start).length() < 1e-2, "{mid}");
}

#[test]
fn every_class_has_two_to_four_sparks_and_its_motion() {
    for class in SparkClass::ALL {
        let m = class.motion();
        assert!((2..=4).contains(&m.count), "{class:?}");
    }
    let (e, r, s, x) = (
        SparkClass::Explosive.motion(),
        SparkClass::Regen.motion(),
        SparkClass::Shield.motion(),
        SparkClass::Reactor.motion(),
    );
    assert!(
        e.laps_per_sec > s.laps_per_sec && s.laps_per_sec > r.laps_per_sec,
        "fast > glide > calm"
    );
    assert!(e.jitter > 0.0 && e.wobble > 0.0, "explosive jitters");
    assert!(s.jitter == 0.0 && s.wobble == 0.0, "shield glides");
    assert!(x.pulse_hz > 0.0 && x.wobble > 0.0, "reactor pulses");
    assert_eq!(r.jitter, 0.0);
}

#[test]
fn hit_bricks_spark_faster() {
    use SparkClass::*;
    for class in SparkClass::ALL {
        assert_eq!(
            damage_speedup(class, 2, 2, None),
            1.0,
            "{class:?} at full health"
        );
    }
    assert!(damage_speedup(Regen, 1, 2, Some(0.9)) > damage_speedup(Regen, 1, 2, Some(0.1)));
    assert!(damage_speedup(Regen, 1, 2, Some(0.0)) > 1.0);
    assert_eq!(damage_speedup(Reactor, 1, 2, None), 2.0);
}

/// Each outline's sparks: (outline style, how many).
fn sparks_per_outline(app: &mut App) -> Vec<(super::super::outline::OutlineStyle, usize)> {
    let outlines: Vec<(Entity, super::super::outline::OutlineStyle)> = app
        .world_mut()
        .query::<(Entity, &BrickOutline)>()
        .iter(app.world())
        .map(|(e, o)| (e, o.style))
        .collect();
    outlines
        .into_iter()
        .map(|(e, style)| {
            let n = app
                .world()
                .get::<Children>(e)
                .into_iter()
                .flatten()
                .filter(|&&c| app.world().get::<Spark>(c).is_some())
                .count();
            (style, n)
        })
        .collect()
}

#[test]
fn every_special_brick_gets_its_sparks_and_plain_bricks_none() {
    let mut app = app();
    let per = sparks_per_outline(&mut app);
    assert!(
        per.len() >= 6,
        "every special style is on the default board"
    );
    for (style, n) in &per {
        assert_eq!(*n, SparkClass::of(*style).motion().count, "{style:?}");
    }
    let total: usize = per.iter().map(|(_, n)| n).sum();
    assert_eq!(
        count::<With<Spark>>(&mut app),
        total,
        "every spark sits on an outline"
    );
}

#[test]
fn breaking_a_special_brick_takes_its_sparks_with_it() {
    let level = crate::levels::parse_level("grid:\nSC").unwrap();
    let mut app = app_with_level(level);
    let shield = brick_of(&mut app, BrickClass::Shield);
    assert_eq!(
        count::<With<Spark>>(&mut app),
        SparkClass::Shield.motion().count
    );
    hit_moving(&mut app, shield, Vec2::new(0.0, -450.0));
    app.update();
    assert!(app.world().get_entity(shield).is_err());
    assert_eq!(count::<With<Spark>>(&mut app), 0, "no orphan sparks");
}

fn laps(app: &mut App) -> Vec<f32> {
    app.world_mut()
        .query::<&Spark>()
        .iter(app.world())
        .map(|s| s.lap)
        .collect()
}

#[test]
fn sparks_move_while_playing_and_freeze_while_paused() {
    let level = crate::levels::parse_level("grid:\nS").unwrap();
    let mut app = app_with_level(level);
    let before = laps(&mut app);
    app.update();
    let moved = laps(&mut app);
    assert!(
        before.iter().zip(&moved).all(|(a, b)| a != b),
        "they travel"
    );

    tap(&mut app, KeyCode::KeyP);
    let paused = laps(&mut app);
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(laps(&mut app), paused, "frozen while paused");
}

#[test]
fn a_hit_reactor_sparks_twice_as_fast_as_an_unhit_one() {
    use crate::bricks::BrickCell;
    let level = crate::levels::parse_level("grid:\nPP").unwrap();
    let mut app = app_with_level(level);
    // Both reactors' sparks start in step (same laps, same ages).
    let reactors: Vec<(Entity, usize)> = app
        .world_mut()
        .query::<(Entity, &BrickCell)>()
        .iter(app.world())
        .map(|(e, c)| (e, c.col))
        .collect();
    let hit_one = reactors.iter().find(|(_, col)| *col == 0).unwrap().0;
    hit(&mut app, hit_one);

    let first_lap = |app: &mut App, brick: Entity| -> f32 {
        let outline = app.world().get::<Children>(brick).unwrap()[0];
        let spark = app
            .world()
            .get::<Children>(outline)
            .unwrap()
            .iter()
            .find(|&c| app.world().get::<Spark>(c).is_some())
            .unwrap();
        app.world().get::<Spark>(spark).unwrap().lap
    };
    let calm = reactors.iter().find(|(_, col)| *col == 1).unwrap().0;
    let (a0, b0) = (first_lap(&mut app, hit_one), first_lap(&mut app, calm));
    app.update();
    let (a1, b1) = (first_lap(&mut app, hit_one), first_lap(&mut app, calm));
    let (fast, slow) = ((a1 - a0).rem_euclid(1.0), (b1 - b0).rem_euclid(1.0));
    assert!((fast / slow - 2.0).abs() < 0.05, "{fast} vs {slow}");
}
