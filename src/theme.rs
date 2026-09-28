//! The Steelbreak palette: every colour the game draws with, in one place,
//! so the later swap to sprites (and any palette tweak) touches only this
//! file. Values follow the Steelbreak concept art.

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

/// Brick rows cycle through these, top to bottom.
pub const BRICK_ROWS: [Color; 3] = [CERAMIC, TITANIUM, TUNGSTEN];
pub const CERAMIC: Color = hex(0xff6a4d);
pub const TITANIUM: Color = hex(0xa9bacd);
pub const TUNGSTEN: Color = hex(0xffab4d);

/// Power-up bricks: reactor-core violet.
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

/// How much a multi-hit brick darkens each time it survives a hit.
const CRACK_DARKEN: f32 = 0.3;

/// The cracked look of a brick that survived a hit (a power-up brick's
/// violet becomes a darker violet).
pub fn cracked(color: Color) -> Color {
    color.darker(CRACK_DARKEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_decodes_the_spec_values() {
        assert_eq!(VOID, Color::srgb_u8(0x08, 0x0b, 0x0f));
        assert_eq!(EMITTER, Color::srgb_u8(0x4f, 0xd8, 0xff));
        assert_eq!(REACTOR, Color::srgb_u8(0xb5, 0x8c, 0xff));
    }

    #[test]
    fn a_cracked_brick_is_darker() {
        let lum = |c: Color| c.luminance();
        assert!(lum(cracked(REACTOR)) < lum(REACTOR));
    }
}
