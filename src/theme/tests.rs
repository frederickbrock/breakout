use super::*;

#[test]
fn a_brick_sprite_is_untinted_unless_shield_glass_flashes() {
    let lum = |c: Color| c.to_linear().luminance();
    assert_eq!(brick_sprite_tint(false), UNTINTED);
    assert!(lum(brick_sprite_tint(true)) > lum(UNTINTED));
}

#[test]
fn hex_decodes_the_spec_values() {
    assert_eq!(VOID, Color::srgb_u8(0x08, 0x0b, 0x0f));
    assert_eq!(EMITTER, Color::srgb_u8(0x4f, 0xd8, 0xff));
    assert_eq!(REACTOR, Color::srgb_u8(0xb5, 0x8c, 0xff));
}

#[test]
fn brick_class_colours_match_the_style_guide() {
    use crate::bricks::ExplosiveKind;
    assert_eq!(CERAMIC, Color::srgb_u8(0xff, 0x7d, 0x58));
    assert_eq!(TITANIUM, Color::srgb_u8(0x9f, 0xb5, 0xca));
    assert_eq!(TUNGSTEN, Color::srgb_u8(0xd6, 0x9a, 0x4e));
    assert_eq!(EXPLOSIVE, Color::srgb_u8(0xe0, 0x30, 0x3a));
    assert_eq!(REGEN, Color::srgb_u8(0x3f, 0xcf, 0x6e));
    assert_eq!(SHIELD, Color::srgb_u8(0x5f, 0xe3, 0xf5));
    for kind in [
        ExplosiveKind::Charge,
        ExplosiveKind::Breach,
        ExplosiveKind::Demolition,
    ] {
        assert_eq!(brick_color(BrickClass::Explosive(kind)), EXPLOSIVE);
    }
    assert_eq!(brick_color(BrickClass::Reactor), REACTOR);
}

#[test]
fn each_class_has_its_own_colour() {
    use crate::bricks::ExplosiveKind;
    let colours = [
        BrickClass::Ceramic,
        BrickClass::Titanium,
        BrickClass::Tungsten,
        BrickClass::Reactor,
        BrickClass::Explosive(ExplosiveKind::Charge),
        BrickClass::Regen,
        BrickClass::Shield,
    ]
    .map(brick_color);
    for (i, a) in colours.iter().enumerate() {
        for b in &colours[i + 1..] {
            assert_ne!(a, b);
        }
    }
}
