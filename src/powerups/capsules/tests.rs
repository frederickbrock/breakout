use super::super::{ActiveEffect, PowerUpCollected};
use super::*;
use crate::test_support::*;

fn effect(duration: f32, elapsed: f32) -> ActiveEffect {
    let mut timer = Timer::from_seconds(duration, TimerMode::Once);
    timer.tick(std::time::Duration::from_secs_f32(elapsed));
    ActiveEffect {
        kind: PowerUpKind::SuperSizer,
        timer,
    }
}

/// Test-injects effects of the given durations (seconds), in pickup order.
/// Only Super-Sizer exists today, so they're all that kind.
fn inject(app: &mut App, durations: &[f32]) {
    let mut active = app.world_mut().resource_mut::<ActiveEffects>();
    for &duration in durations {
        active.0.push(effect(duration, 0.0));
    }
}

/// Each capsule, by slot: (slot, centre y, fill width, fill colour, label).
fn capsules(app: &mut App) -> Vec<(usize, f32, f32, Color, String)> {
    let mut found: Vec<_> = app
        .world_mut()
        .query::<(&Capsule, &Transform, Entity)>()
        .iter(app.world())
        .map(|(c, t, e)| (c.slot, t.translation.y, e))
        .collect();
    found.sort_by_key(|(slot, _, _)| *slot);
    found
        .into_iter()
        .map(|(slot, y, entity)| {
            let fill = descendant::<CapsuleFill>(app, entity);
            let sprite = app.world().get::<Sprite>(fill).unwrap();
            let width = sprite.custom_size.unwrap().x;
            let color = sprite.color;
            let label = descendant::<CapsuleLabel>(app, entity);
            let text = app.world().get::<Text2d>(label).unwrap().0.clone();
            (slot, y, width, color, text)
        })
        .collect()
}

fn descendant<C: Component>(app: &App, root: Entity) -> Entity {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if app.world().get::<C>(entity).is_some() {
            return entity;
        }
        if let Some(children) = app.world().get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    panic!("no {} under the capsule", std::any::type_name::<C>());
}

fn assert_close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-3, "{a} != {b}");
}

#[test]
fn views_follow_active_effects_in_pickup_order() {
    assert!(capsule_views(&ActiveEffects::default()).is_empty());
    let active = ActiveEffects(vec![effect(10.0, 4.0), effect(5.0, 0.0)]);
    let views = capsule_views(&active);
    assert_eq!(views.len(), 2);
    assert_close(views[0].fraction, 0.6);
    assert_close(views[0].secs_left, 6.0);
    assert_eq!(views[0].label(), "6.0s");
    assert_close(views[1].fraction, 1.0);
}

#[test]
fn the_fill_turns_amber_and_blinks_in_the_last_two_seconds() {
    let view = |secs_left: f32| CapsuleView {
        kind: PowerUpKind::SuperSizer,
        fraction: secs_left / 7.0,
        secs_left,
    };
    assert_eq!(view(5.0).fill_color(), theme::CAPSULE_FILL);
    assert_eq!(view(2.1).fill_color(), theme::CAPSULE_FILL);
    let lit = view(1.9).fill_color(); // 7.6 blink cycles left: lit half
    let dim = view(1.8).fill_color(); // 7.2: dim half
    assert_eq!(lit, theme::CAPSULE_WARN);
    assert_eq!(dim, theme::CAPSULE_WARN.with_alpha(BLINK_DIM));
}

#[test]
fn catching_super_sizer_shows_a_draining_capsule_and_catching_it_again_refills_it() {
    let mut app = app();
    assert!(capsules(&mut app).is_empty());
    app.world_mut().trigger(PowerUpCollected {
        kind: PowerUpKind::SuperSizer,
    });
    app.update();
    let shown = capsules(&mut app);
    assert_eq!(shown.len(), 1);
    let (slot, y, full, color, _) = shown[0].clone();
    assert_eq!((slot, y), (0, slot_y(0)));
    assert!(full > FILL_WIDTH * 0.95, "nearly full: {full}");
    assert_eq!(color, theme::CAPSULE_FILL);
    let icon = app
        .world_mut()
        .query_filtered::<Entity, With<CapsuleIcon>>()
        .iter(app.world())
        .count();
    assert_eq!(icon, 1);

    for _ in 0..20 {
        app.update(); // 2 s
    }
    let (_, _, drained, _, label) = capsules(&mut app)[0].clone();
    assert!(drained < full - FILL_WIDTH * 0.25, "{drained} vs {full}");
    assert_eq!(label, "4.9s");

    // Caught again: the same capsule refills, no second one.
    app.world_mut().trigger(PowerUpCollected {
        kind: PowerUpKind::SuperSizer,
    });
    app.update();
    let shown = capsules(&mut app);
    assert_eq!(shown.len(), 1);
    assert!(shown[0].2 > FILL_WIDTH * 0.95);
}

#[test]
fn a_capsule_warns_then_disappears_when_its_effect_ends() {
    let mut app = app();
    inject(&mut app, &[2.5]);
    app.update(); // 2.4 s left
    assert_eq!(capsules(&mut app)[0].3, theme::CAPSULE_FILL);
    for _ in 0..5 {
        app.update(); // 1.9 s left
    }
    let color = capsules(&mut app)[0].3;
    assert_eq!(color.with_alpha(1.0), theme::CAPSULE_WARN);
    for _ in 0..20 {
        app.update();
    }
    assert!(capsules(&mut app).is_empty());
}

#[test]
fn capsules_stack_in_pickup_order_and_close_the_gap() {
    let mut app = app();
    // The middle one runs out first.
    inject(&mut app, &[20.0, 1.0, 10.0]);
    app.update();
    let shown = capsules(&mut app);
    assert_eq!(shown.len(), 3);
    let labels: Vec<&str> = shown.iter().map(|c| c.4.as_str()).collect();
    assert_eq!(labels, ["19.9s", "0.9s", "9.9s"]);
    for (slot, capsule) in shown.iter().enumerate() {
        assert_eq!((capsule.0, capsule.1), (slot, slot_y(slot)));
    }
    assert!(slot_y(0) > slot_y(1), "newest at the bottom");

    for _ in 0..10 {
        app.update();
    }
    let shown = capsules(&mut app);
    assert_eq!(shown.len(), 2);
    let labels: Vec<&str> = shown.iter().map(|c| c.4.as_str()).collect();
    assert_eq!(labels, ["18.9s", "8.9s"]);
    assert_eq!(shown[1].1, slot_y(1), "moved up into the gap");
}

#[test]
fn pausing_freezes_the_capsules() {
    let mut app = app();
    inject(&mut app, &[5.0]);
    app.update();
    tap(&mut app, KeyCode::KeyP);
    let paused = capsules(&mut app);
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(capsules(&mut app), paused);
    tap(&mut app, KeyCode::KeyP);
    app.update();
    assert!(capsules(&mut app)[0].2 < paused[0].2);
}

#[test]
fn a_new_run_starts_with_no_capsules() {
    let mut app = app();
    inject(&mut app, &[5.0, 6.0]);
    app.update();
    assert_eq!(capsules(&mut app).len(), 2);

    tap(&mut app, KeyCode::Escape);
    crate::menu::test_helpers::press(&mut app, "Main menu");
    assert!(capsules(&mut app).is_empty(), "torn down with the run");
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::InGame);
    app.update();
    app.update();
    assert!(capsules(&mut app).is_empty(), "a new run has none");
}
