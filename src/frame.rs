//! The steel frame in the two side panels: the "artificial walls" around the
//! playfield well.
//!
//! Each panel ([`FramePanel`]) is a plate-coloured sprite filling its side
//! panel ([`SIDE_PANEL_WIDTH`] × [`WORLD_HEIGHT`]), with coded [`FramePiece`]
//! children laid out by the pure [`frame_pieces`]: a girder down the
//! playfield edge, tie plates across it, and a bright lip whose inner edge
//! sits exactly on the physics wall ([`crate::world::wall_specs`]). Colours
//! are the `theme::FRAME_*` constants.
//!
//! The frame is global, like the walls: [`FramePlugin`] spawns it once at
//! `Startup`, so it shows on every screen and survives restarts. It sits at
//! [`FRAME_Z`], above the background and below gameplay and the HUD.
//!
//! Painted art replaces it through the usual sprite rule
//! ([`crate::sprites`]): once `frame_left.png` / `frame_right.png` load, the
//! panel draws that image at panel size (the art ships at 2×) and its coded
//! pieces are hidden. A missing or broken file keeps the coded frame.

use bevy::prelude::*;

use crate::theme;
use crate::world::{GAME_SCALE, PLAYFIELD_WIDTH, SIDE_PANEL_WIDTH, WORLD_HEIGHT};

/// Above the background (−10), below every gameplay entity and the HUD.
pub(crate) const FRAME_Z: f32 = -5.0;
/// The girder running down each panel's playfield edge.
const GIRDER_WIDTH: f32 = 14.0 * GAME_SCALE;
/// The bright lip on the wall line.
const EDGE_WIDTH: f32 = 2.0 * GAME_SCALE;
/// Tie plates across the girder, evenly spaced down the panel.
const TIE_HEIGHT: f32 = 4.0 * GAME_SCALE;
const TIE_SPACING: f32 = 60.0 * GAME_SCALE;

/// One side panel of the frame. `side` is −1 (left) or 1 (right).
#[derive(Component)]
pub(crate) struct FramePanel {
    pub(crate) side: f32,
}

/// A coded piece drawn over a panel's fill (girder, tie, edge lip). Hidden
/// once the panel draws its painted sprite.
#[derive(Component)]
pub(crate) struct FramePiece;

pub(crate) struct FramePlugin;

impl Plugin for FramePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_frame);
    }
}

/// Centre of the `side` panel in world space.
pub(crate) fn panel_centre(side: f32) -> Vec2 {
    Vec2::new(side * (PLAYFIELD_WIDTH + SIDE_PANEL_WIDTH) / 2.0, 0.0)
}

pub(crate) fn panel_size() -> Vec2 {
    Vec2::new(SIDE_PANEL_WIDTH, WORLD_HEIGHT)
}

/// The coded pieces of the `side` panel as (centre, size, colour), in the
/// panel's local space. The playfield edge is at local x = `-side * width / 2`.
pub(crate) fn frame_pieces(side: f32) -> Vec<(Vec2, Vec2, Color)> {
    let inner = -side * SIDE_PANEL_WIDTH / 2.0;
    // x of a strip `width` wide whose inner edge is on the playfield edge.
    let against_edge = |width: f32| inner + side * width / 2.0;

    let girder_x = against_edge(GIRDER_WIDTH);
    let mut pieces = vec![(
        Vec2::new(girder_x, 0.0),
        Vec2::new(GIRDER_WIDTH, WORLD_HEIGHT),
        theme::FRAME_GIRDER,
    )];
    let ties = (WORLD_HEIGHT / TIE_SPACING) as usize;
    let first = -WORLD_HEIGHT / 2.0 + TIE_SPACING / 2.0;
    pieces.extend((0..ties).map(|i| {
        (
            Vec2::new(girder_x, first + i as f32 * TIE_SPACING),
            Vec2::new(GIRDER_WIDTH, TIE_HEIGHT),
            theme::FRAME_TIE,
        )
    }));
    pieces.push((
        Vec2::new(against_edge(EDGE_WIDTH), 0.0),
        Vec2::new(EDGE_WIDTH, WORLD_HEIGHT),
        theme::FRAME_EDGE,
    ));
    pieces
}

fn spawn_frame(mut commands: Commands) {
    for side in [-1.0, 1.0] {
        let panel = commands
            .spawn((
                FramePanel { side },
                Sprite::from_color(theme::FRAME_PANEL, panel_size()),
                Transform::from_translation(panel_centre(side).extend(FRAME_Z)),
            ))
            .id();
        // Later pieces draw on top: girder, then ties, then the edge lip.
        for (i, (centre, size, color)) in frame_pieces(side).into_iter().enumerate() {
            commands.spawn((
                FramePiece,
                Sprite::from_color(color, size),
                Transform::from_translation(centre.extend(0.01 * (i + 1) as f32)),
                ChildOf(panel),
            ));
        }
    }
}

#[cfg(test)]
mod tests;
