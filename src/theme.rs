//! The Steelbreak palette: every colour the game draws with, in one place,
//! so the later swap to sprites (and any palette tweak) touches only this
//! file. Values follow the Steelbreak concept art.

use crate::bricks::BrickClass;
use bevy::prelude::*;

const fn hex(rgb: u32) -> Color {
    Color::srgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// Dark hangar void behind everything (the clear colour).
pub const VOID: Color = hex(0x080b0f);
/// Panel tone `#0c1118` at 75% opacity, laid over the frozen game behind
/// the pause and game-over menus.
pub const OVERLAY_DIM: Color = Color::srgba_u8(0x0c, 0x11, 0x18, 191);

/// The ball: steel grey.
pub const STEEL: Color = hex(0xb4bfcb);
/// The paddle's emitter body: cyan.
pub const EMITTER: Color = hex(0x4fd8ff);
/// The lighter "prongs" at each end of the paddle.
pub const EMITTER_PRONG: Color = hex(0xbff2ff);

/// Brick class face colours (the middle face tone of each class in the
/// Steelbreak style guide).
pub const CERAMIC: Color = hex(0xff7d58);
pub const TITANIUM: Color = hex(0x9fb5ca);
pub const TUNGSTEN: Color = hex(0xd69a4e);
/// Shared by all three explosive variants.
pub const EXPLOSIVE: Color = hex(0xe0303a);
pub const REGEN: Color = hex(0x3fcf6e);
pub const SHIELD: Color = hex(0x5fe3f5);
/// Brief flash on shield glass hit from below or the side.
pub const SHIELD_FLASH: Color = INK;
/// The placeholder burst where an explosive goes off (explosive glow).
pub const BLAST_FLASH: Color = hex(0xff9a92);
/// Reactor (power-up) bricks: reactor-core violet.
pub const REACTOR: Color = hex(0xb58cff);

/// Falling power-ups: cyan, readable against the void and distinct from the
/// violet bricks they drop from.
pub const POWER_UP: Color = EMITTER;

/// A sprite's tint when it draws its own image unmodified.
pub const UNTINTED: Color = Color::WHITE;

/// HUD values and headings.
pub const INK: Color = hex(0xdde6f1);
/// HUD labels ("SCORE", "LIVES").
pub const LABEL: Color = hex(0x8a9aab);

/// Menu buttons: panel-toned plates, cyan when pressed or focused.
pub const BUTTON_NORMAL: Color = hex(0x16202b);
pub const BUTTON_HOVERED: Color = hex(0x223244);
pub const BUTTON_PRESSED: Color = hex(0x1d4a5c);
pub const BORDER_NORMAL: Color = hex(0x2c3a48);
pub const BORDER_FOCUSED: Color = EMITTER;

/// How much darker a damaged brick is than its class colour.
const CRACK_DARKEN: f32 = 0.3;

/// The cracked look of a brick that survived a hit (a power-up brick's
/// violet becomes a darker violet).
pub fn cracked(color: Color) -> Color {
    color.darker(CRACK_DARKEN)
}

/// A full-health brick's colour.
pub fn brick_color(class: BrickClass) -> Color {
    match class {
        BrickClass::Ceramic => CERAMIC,
        BrickClass::Titanium => TITANIUM,
        BrickClass::Tungsten => TUNGSTEN,
        BrickClass::Reactor => REACTOR,
        BrickClass::Explosive(_) => EXPLOSIVE,
        BrickClass::Regen => REGEN,
        BrickClass::Shield => SHIELD,
    }
}

/// How a brick of `class` with `health` hits left looks: its class colour,
/// cracked (darker) below full health.
pub fn brick_face(class: BrickClass, health: u8) -> Color {
    let color = brick_color(class);
    if health < class.max_hits() {
        cracked(color)
    } else {
        color
    }
}

/// Shield glass's flash over its sprite: brightens the glass (a tint above
/// 1 scales the image up; the flat-colour fallback uses [`SHIELD_FLASH`]).
pub const SHIELD_FLASH_TINT: Color = Color::LinearRgba(LinearRgba::rgb(2.5, 2.5, 2.5));

/// The tint over a brick drawn with its sprite: none at full health, darker
/// once damaged (interim, until damage particles replace it), bright while
/// shield glass flashes.
pub fn brick_sprite_tint(class: BrickClass, health: u8, flashing: bool) -> Color {
    if flashing {
        SHIELD_FLASH_TINT
    } else if health < class.max_hits() {
        cracked(UNTINTED)
    } else {
        UNTINTED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_brick_sprite_is_untinted_at_full_health_and_darker_when_damaged() {
        let lum = |c: Color| c.to_linear().luminance();
        assert_eq!(brick_sprite_tint(BrickClass::Tungsten, 3, false), UNTINTED);
        let damaged = brick_sprite_tint(BrickClass::Tungsten, 2, false);
        assert!(lum(damaged) < lum(UNTINTED));
        assert_eq!(brick_sprite_tint(BrickClass::Tungsten, 1, false), damaged);
        assert_eq!(brick_sprite_tint(BrickClass::Regen, 2, false), UNTINTED);
        let flash = brick_sprite_tint(BrickClass::Shield, 1, true);
        assert!(lum(flash) > lum(UNTINTED));
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

    #[test]
    fn brick_face_cracks_only_below_full_health() {
        assert_eq!(brick_face(BrickClass::Tungsten, 3), TUNGSTEN);
        assert_eq!(brick_face(BrickClass::Tungsten, 2), cracked(TUNGSTEN));
        assert_eq!(brick_face(BrickClass::Tungsten, 1), cracked(TUNGSTEN));
        assert_eq!(brick_face(BrickClass::Ceramic, 1), CERAMIC);
    }

    #[test]
    fn a_cracked_brick_is_darker() {
        let lum = |c: Color| c.luminance();
        assert!(lum(cracked(REACTOR)) < lum(REACTOR));
    }
}
