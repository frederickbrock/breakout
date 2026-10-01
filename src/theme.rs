//! The Steelbreak palette: every colour the game draws with, in one place,
//! so the later swap to sprites (and any palette tweak) touches only this
//! file. Values follow the Steelbreak concept art.
//!
//! Use a `theme::` constant instead of a colour literal. These colours are
//! also the fallback look when a sprite file is missing (see
//! [`crate::sprites`]): the ball is a round mesh in [`STEEL`], the paddle's
//! prongs and field are [`EMITTER_PRONG`] and [`EMITTER`] sprites, and each
//! brick is its [`brick_color`].
//!
//! A brick keeps its class colour whatever its health; damage is shown by
//! particles.

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

/// Behaviour outlines (coded placeholders until the sim-rdl.7.5 art): the
/// border drawn over a special brick so its behaviour reads at a glance.
pub const OUTLINE_EXPLOSIVE: Color = hex(0xff3b3b);
pub const OUTLINE_REGEN: Color = hex(0x3dff7a);
pub const OUTLINE_SHIELD: Color = hex(0x4fd8ff);
pub const OUTLINE_REACTOR: Color = hex(0xb58cff);

/// Outline pulse rates, in pulses per second.
pub const CHARGE_PULSE_HZ: f32 = 1.0;
pub const BREACH_PULSE_HZ: f32 = 1.0;
pub const DEMOLITION_PULSE_HZ: f32 = 2.0;
/// A healthy regen brick's slow breathing.
pub const REGEN_BREATHE_HZ: f32 = 0.3;
/// A damaged regen brick's blink: from this rate just after the hit...
pub const REGEN_BLINK_START_HZ: f32 = 1.0;
/// ...up to this one just before it heals.
pub const REGEN_BLINK_END_HZ: f32 = 6.0;

/// Brick class glow colours (the seam/glow column of the style guide):
/// hit sparks and damage sparks.
pub const CERAMIC_GLOW: Color = hex(0xff6a4d);
pub const TITANIUM_GLOW: Color = hex(0x7fc4ff);
pub const TUNGSTEN_GLOW: Color = hex(0xffab4d);
pub const REACTOR_GLOW: Color = hex(0xb58cff);
pub const EXPLOSIVE_GLOW: Color = hex(0xff3b3b);
pub const REGEN_GLOW: Color = hex(0x3dff7a);
pub const SHIELD_GLOW: Color = hex(0x4fd8ff);
/// Light smoke rising from a damaged brick.
pub const SMOKE: Color = hex(0x8a9aab);
/// The ball's fading trail while it's in flight: steel blue.
pub const BALL_TRAIL: Color = hex(0xa9dcff);
/// The small spark burst where the ball bounces off a wall, brick or the
/// paddle.
pub const BOUNCE_SPARK: Color = hex(0xdff4ff);

/// A brick class's glow colour (its sparks).
pub fn brick_glow(class: BrickClass) -> Color {
    match class {
        BrickClass::Ceramic => CERAMIC_GLOW,
        BrickClass::Titanium => TITANIUM_GLOW,
        BrickClass::Tungsten => TUNGSTEN_GLOW,
        BrickClass::Reactor => REACTOR_GLOW,
        BrickClass::Explosive(_) => EXPLOSIVE_GLOW,
        BrickClass::Regen => REGEN_GLOW,
        BrickClass::Shield => SHIELD_GLOW,
    }
}

/// A brick's colour. Damage doesn't change it: it shows as particles
/// (`src/particles/`).
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

/// Shield glass's flash over its sprite: brightens the glass (a tint above
/// 1 scales the image up; the flat-colour fallback uses [`SHIELD_FLASH`]).
pub const SHIELD_FLASH_TINT: Color = Color::LinearRgba(LinearRgba::rgb(2.5, 2.5, 2.5));

/// The tint over a brick drawn with its sprite: none, or bright while shield
/// glass flashes. Damage never tints a brick; particles show it.
pub fn brick_sprite_tint(flashing: bool) -> Color {
    if flashing {
        SHIELD_FLASH_TINT
    } else {
        UNTINTED
    }
}

#[cfg(test)]
mod tests;
