use super::*;
use crate::test_support::*;
use crate::theme;

/// Runs `secs` of play at the test app's 100 ms step.
fn wait(app: &mut App, secs: f32) {
    for _ in 0..(secs * 10.0).round() as usize {
        app.update();
    }
}

fn health(app: &App, brick: Entity) -> u8 {
    app.world().get::<BrickHealth>(brick).unwrap().0
}

fn color(app: &App, brick: Entity) -> Color {
    app.world().get::<Sprite>(brick).unwrap().color
}

#[test]
fn a_second_hit_before_it_heals_breaks_it() {
    let mut app = app();
    let regen = brick_of(&mut app, BrickClass::Regen);
    hit(&mut app, regen);
    assert_eq!(health(&app, regen), 1);
    assert_eq!(
        color(&app, regen),
        theme::REGEN,
        "damage doesn't recolour it"
    );
    wait(&mut app, HEAL_SECS - 1.0);
    hit(&mut app, regen);
    assert!(app.world().get_entity(regen).is_err());
}

#[test]
fn left_alone_for_the_heal_time_it_heals_to_full() {
    let mut app = app();
    let regen = brick_of(&mut app, BrickClass::Regen);
    hit(&mut app, regen);
    wait(&mut app, HEAL_SECS - 0.5);
    assert_eq!(health(&app, regen), 1, "not yet");
    wait(&mut app, 1.0);
    assert_eq!(health(&app, regen), 2);
    assert_eq!(color(&app, regen), theme::REGEN);
    assert!(!app.world().entity(regen).contains::<RegenTimer>());

    // It needs two hits again.
    hit(&mut app, regen);
    assert!(app.world().get_entity(regen).is_ok());
    hit(&mut app, regen);
    assert!(app.world().get_entity(regen).is_err());
}

#[test]
fn pausing_freezes_the_heal_timer() {
    let mut app = app();
    let regen = brick_of(&mut app, BrickClass::Regen);
    hit(&mut app, regen);
    wait(&mut app, 1.0);
    tap(&mut app, KeyCode::KeyP);
    wait(&mut app, 10.0);
    assert_eq!(health(&app, regen), 1, "paused for 10 s: still damaged");
    tap(&mut app, KeyCode::KeyP);
    // About 1.2 s had run before the pause, so ~HEAL_SECS - 1.2 s remain.
    wait(&mut app, HEAL_SECS - 2.0);
    assert_eq!(health(&app, regen), 1);
    wait(&mut app, 1.5);
    assert_eq!(health(&app, regen), 2);
}

#[test]
fn further_damage_restarts_the_timer() {
    let mut app = app();
    let regen = brick_of(&mut app, BrickClass::Regen);
    hit(&mut app, regen);
    wait(&mut app, 2.0);
    // Another non-lethal hit (e.g. explosion damage, sim-rdl.7.3).
    app.world_mut().trigger(BrickDamaged {
        brick: regen,
        position: Vec2::ZERO,
        class: BrickClass::Regen,
    });
    app.world_mut().flush();
    wait(&mut app, HEAL_SECS - 1.0);
    assert_eq!(health(&app, regen), 1, "timer restarted, not yet up");
    wait(&mut app, 1.5);
    assert_eq!(health(&app, regen), 2);
}

#[test]
fn other_classes_never_heal() {
    let mut app = app();
    let tungsten = brick_of(&mut app, BrickClass::Tungsten);
    hit(&mut app, tungsten);
    wait(&mut app, 5.0);
    assert_eq!(health(&app, tungsten), 2);
    assert!(!app.world().entity(tungsten).contains::<RegenTimer>());
}
