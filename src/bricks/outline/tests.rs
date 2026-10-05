use super::*;
use crate::collision::BrickDamaged;
use crate::game_state::AppState;
use crate::test_support::*;

const SIZE: Vec2 = Vec2::new(BRICK_WIDTH, BRICK_HEIGHT);

fn outline_of(app: &mut App, brick: Entity) -> Option<Entity> {
    app.world_mut()
        .query::<(Entity, &ChildOf, &BrickOutline)>()
        .iter(app.world())
        .find(|(_, parent, _)| parent.parent() == brick)
        .map(|(e, _, _)| e)
}

fn phase(app: &App, outline: Entity) -> f32 {
    app.world().get::<BrickOutline>(outline).unwrap().phase
}

fn strip_colors(app: &mut App, outline: Entity) -> Vec<Color> {
    let children: Vec<Entity> = app.world().get::<Children>(outline).unwrap().to_vec();
    // Only the coded strips (the outline's sparks are children too).
    children
        .into_iter()
        .filter(|&c| app.world().get::<OutlineStrip>(c).is_some())
        .map(|c| app.world().get::<Sprite>(c).unwrap().color)
        .collect()
}

#[test]
fn only_special_classes_get_an_outline() {
    use ExplosiveKind::*;
    for class in [
        BrickClass::Ceramic,
        BrickClass::Titanium,
        BrickClass::Tungsten,
    ] {
        assert_eq!(OutlineStyle::of(class), None);
    }
    assert_eq!(
        OutlineStyle::of(BrickClass::Explosive(Charge)),
        Some(OutlineStyle::Charge)
    );
    assert_eq!(
        OutlineStyle::of(BrickClass::Explosive(Breach)),
        Some(OutlineStyle::Breach)
    );
    assert_eq!(
        OutlineStyle::of(BrickClass::Explosive(Demolition)),
        Some(OutlineStyle::Demolition)
    );
    assert_eq!(
        OutlineStyle::of(BrickClass::Regen),
        Some(OutlineStyle::Regen)
    );
    assert_eq!(
        OutlineStyle::of(BrickClass::Shield),
        Some(OutlineStyle::Shield)
    );
    assert_eq!(
        OutlineStyle::of(BrickClass::Reactor),
        Some(OutlineStyle::Reactor)
    );
}

#[test]
fn outline_colours_follow_the_spec() {
    assert_eq!(
        OutlineStyle::Charge.color(),
        Color::srgb_u8(0xff, 0x3b, 0x3b)
    );
    assert_eq!(OutlineStyle::Breach.color(), theme::OUTLINE_EXPLOSIVE);
    assert_eq!(OutlineStyle::Demolition.color(), theme::OUTLINE_EXPLOSIVE);
    assert_eq!(
        OutlineStyle::Regen.color(),
        Color::srgb_u8(0x3d, 0xff, 0x7a)
    );
    assert_eq!(
        OutlineStyle::Shield.color(),
        Color::srgb_u8(0x4f, 0xd8, 0xff)
    );
    assert_eq!(
        OutlineStyle::Reactor.color(),
        Color::srgb_u8(0xb5, 0x8c, 0xff)
    );
}

#[test]
fn every_strip_stays_on_the_brick() {
    use OutlineStyle::*;
    for style in [Charge, Breach, Demolition, Regen, Shield, Reactor] {
        for s in strips(style, SIZE) {
            let min = s.center - s.size / 2.0;
            let max = s.center + s.size / 2.0;
            assert!(min.cmpge(-SIZE / 2.0 - 0.01).all(), "{style:?} {s:?}");
            assert!(max.cmple(SIZE / 2.0 + 0.01).all(), "{style:?} {s:?}");
        }
    }
}

#[test]
fn the_explosive_variants_have_different_shapes_and_rates() {
    let charge = strips(OutlineStyle::Charge, SIZE);
    let breach = strips(OutlineStyle::Breach, SIZE);
    let demolition = strips(OutlineStyle::Demolition, SIZE);
    // Charge: one solid, evenly lit border.
    assert_eq!(charge.len(), 4);
    assert!(charge.iter().all(|s| s.weight == 1.0));
    // Breach: a dim border, bright only at the four side midpoints.
    let bright: Vec<_> = breach.iter().filter(|s| s.weight == 1.0).collect();
    assert_eq!(bright.len(), 4);
    assert!(bright
        .iter()
        .all(|s| s.center.x == 0.0 || s.center.y == 0.0));
    assert!(bright.iter().all(|s| s.size.max_element() < SIZE.y));
    // Demolition: two nested borders, the outer thicker than charge's.
    assert_eq!(demolition.len(), 8);
    assert!(demolition[0].size.y > charge[0].size.y);
    // Demolition pulses twice as fast.
    let hz = |s| pulse_hz(s, None).unwrap();
    assert_eq!(hz(OutlineStyle::Charge), 1.0);
    assert_eq!(hz(OutlineStyle::Breach), 1.0);
    assert_eq!(hz(OutlineStyle::Demolition), 2.0);
}

#[test]
fn shield_glass_is_brightest_along_its_top_edge() {
    let shield = strips(OutlineStyle::Shield, SIZE);
    let top = shield
        .iter()
        .max_by(|a, b| a.center.y.total_cmp(&b.center.y))
        .unwrap();
    assert_eq!(top.weight, 1.0);
    assert!(shield.iter().filter(|s| s.weight == 1.0).count() == 1);
    assert_eq!(pulse_hz(OutlineStyle::Shield, None), None, "steady");
    assert_eq!(pulse_hz(OutlineStyle::Reactor, None), None, "steady");
}

#[test]
fn a_damaged_regen_blinks_faster_as_healing_nears() {
    let breathe = pulse_hz(OutlineStyle::Regen, None).unwrap();
    let just_hit = pulse_hz(OutlineStyle::Regen, Some(0.0)).unwrap();
    let halfway = pulse_hz(OutlineStyle::Regen, Some(0.5)).unwrap();
    let nearly = pulse_hz(OutlineStyle::Regen, Some(0.95)).unwrap();
    assert!(breathe < just_hit && just_hit < halfway && halfway < nearly);
}

#[test]
fn brightness_pulses_between_low_and_full() {
    assert_eq!(brightness(true, 0.0), PULSE_HIGH);
    assert!((brightness(true, 0.5) - PULSE_LOW).abs() < 1e-6);
    assert_eq!(brightness(false, 0.5), PULSE_HIGH);
}

#[test]
fn a_run_outlines_exactly_its_special_bricks() {
    let mut app = app();
    let special = app
        .world_mut()
        .query::<&BrickClass>()
        .iter(app.world())
        .filter(|c| OutlineStyle::of(**c).is_some())
        .count();
    assert!(special > 0);
    assert_eq!(count::<With<BrickOutline>>(&mut app), special);
    for class in [
        BrickClass::Ceramic,
        BrickClass::Titanium,
        BrickClass::Tungsten,
    ] {
        let brick = brick_of(&mut app, class);
        assert_eq!(outline_of(&mut app, brick), None, "{class:?}");
    }
    let breach = brick_of(&mut app, BrickClass::Explosive(ExplosiveKind::Breach));
    let outline = outline_of(&mut app, breach).expect("breach has an outline");
    assert_eq!(strip_colors(&mut app, outline).len(), 8);
}

#[test]
fn destroying_a_brick_takes_its_outline_with_it() {
    let mut app = app();
    let outlines = count::<With<BrickOutline>>(&mut app);
    let strips = count::<With<OutlineStrip>>(&mut app);
    let shield = brick_of(&mut app, BrickClass::Shield);
    let outline = outline_of(&mut app, shield).unwrap();
    hit_moving(&mut app, shield, Vec2::new(0.0, -300.0));
    app.update();
    assert!(app.world().get_entity(shield).is_err());
    assert!(app.world().get_entity(outline).is_err());
    assert_eq!(count::<With<BrickOutline>>(&mut app), outlines - 1);
    assert_eq!(count::<With<OutlineStrip>>(&mut app), strips - 4);

    // Leaving the run leaves no outline behind.
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::MainMenu);
    app.update();
    assert_eq!(count::<With<BrickOutline>>(&mut app), 0);
    assert_eq!(count::<With<OutlineStrip>>(&mut app), 0);
}

#[test]
fn outlines_animate_while_playing_and_freeze_while_paused() {
    let mut app = app();
    let charge = brick_of(&mut app, BrickClass::Explosive(ExplosiveKind::Charge));
    let outline = outline_of(&mut app, charge).unwrap();
    app.update();
    app.update();
    let before = phase(&app, outline);
    assert!(before > 0.0, "pulsing while playing");

    tap(&mut app, KeyCode::KeyP);
    let paused_phase = phase(&app, outline);
    let paused_colors = strip_colors(&mut app, outline);
    for _ in 0..7 {
        app.update();
    }
    assert_eq!(phase(&app, outline), paused_phase, "frozen while paused");
    assert_eq!(strip_colors(&mut app, outline), paused_colors);

    tap(&mut app, KeyCode::KeyP);
    app.update();
    assert_ne!(phase(&app, outline), paused_phase, "resumes");
}

#[test]
fn a_damaged_regen_outline_speeds_up_until_it_heals() {
    let mut app = app();
    let regen = brick_of(&mut app, BrickClass::Regen);
    let outline = outline_of(&mut app, regen).unwrap();
    // Phase advance over one 100 ms frame, in cycles.
    let step = |app: &mut App| {
        let a = phase(app, outline);
        app.update();
        (phase(app, outline) - a).rem_euclid(1.0)
    };
    let breathing = step(&mut app);
    app.world_mut().trigger(BrickDamaged {
        brick: regen,
        position: Vec2::ZERO,
        class: BrickClass::Regen,
    });
    app.world_mut().flush();
    let early = step(&mut app);
    // The heal takes this many 100 ms frames.
    let heal_frames = (crate::bricks::regen::HEAL_SECS * 10.0).round() as usize;
    for _ in 0..heal_frames * 2 / 3 {
        app.update();
    }
    let late = step(&mut app);
    assert!(
        breathing < early && early < late,
        "{breathing} {early} {late}"
    );
    // Healed: back to breathing.
    for _ in 0..heal_frames / 3 + 5 {
        app.update();
    }
    assert!(!app.world().entity(regen).contains::<RegenTimer>());
    assert!((step(&mut app) - breathing).abs() < 1e-4);
}

#[test]
fn each_style_maps_to_its_painted_frame() {
    use ExplosiveKind::*;
    for (class, file) in [
        (BrickClass::Reactor, "outline_reactor.png"),
        (BrickClass::Regen, "outline_regen.png"),
        (BrickClass::Shield, "outline_shield.png"),
        (BrickClass::Explosive(Charge), "outline_charge.png"),
        (BrickClass::Explosive(Breach), "outline_breach.png"),
        (BrickClass::Explosive(Demolition), "outline_demolition.png"),
    ] {
        let style = OutlineStyle::of(class).unwrap();
        assert_eq!(style.path(), format!("sprites/bricks/{file}"), "{class:?}");
        assert!(
            std::path::Path::new(&format!("assets/{}", style.path())).exists(),
            "{file} ships"
        );
    }
    // `ALL` is in declaration order, which `GameSprites::outline` indexes by.
    for (i, style) in OutlineStyle::ALL.into_iter().enumerate() {
        assert_eq!(style as usize, i);
    }
}

#[test]
fn a_painted_frame_never_pulses_below_sixty_percent() {
    let mut lowest: f32 = 1.0;
    for step in 0..=100 {
        let b = painted_brightness(true, step as f32 / 100.0);
        assert!((PAINTED_PULSE_LOW..=1.0).contains(&b), "{b}");
        lowest = lowest.min(b);
    }
    assert!(
        (lowest - PAINTED_PULSE_LOW).abs() < 1e-3,
        "it still pulses down to 60%"
    );
    assert_eq!(painted_brightness(true, 0.0), 1.0);
    assert_eq!(
        painted_brightness(false, 0.37),
        1.0,
        "a steady frame stays full"
    );
}

#[test]
fn the_painted_frame_sits_four_art_pixels_outside_the_plate() {
    // 168×68 art around 160×60: 4 art px each side, at the plate's scale.
    let margin = (FRAME_SIZE - SIZE) / 2.0;
    assert!((margin.x - 4.0 * BRICK_WIDTH / 160.0).abs() < 1e-3);
    assert!((margin.y - 4.0 * BRICK_HEIGHT / 60.0).abs() < 1e-3);
}
