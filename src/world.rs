use avian2d::prelude::*;
use bevy::prelude::*;

use crate::ball::{BallLook, BALL_SIZE};
use crate::theme;

// Game constants
/// Every gameplay size and speed is its old 900x650-window design value times
/// this, so the game looks and plays the same in the bigger world.
pub(crate) const GAME_SCALE: f32 = 1.5;
/// The logical world the camera always shows in full (letterboxed to fit the
/// window, see `view`).
pub(crate) const WORLD_WIDTH: f32 = 1920.0;
pub(crate) const WORLD_HEIGHT: f32 = 1080.0;
/// The centred playfield well: the walls sit on its left, right and top
/// edges, and the ball is lost below its bottom edge.
pub(crate) const PLAYFIELD_WIDTH: f32 = 1440.0;
pub(crate) const PLAYFIELD_HEIGHT: f32 = WORLD_HEIGHT;
/// The panel either side of the well (240), home of the HUD.
#[cfg(test)]
pub(crate) const SIDE_PANEL_WIDTH: f32 = (WORLD_WIDTH - PLAYFIELD_WIDTH) / 2.0;
pub(crate) const WALL_THICKNESS: f32 = 40.0 * GAME_SCALE;

pub(crate) fn setup_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.insert_resource(BallLook {
        mesh: meshes.add(Circle::new(BALL_SIZE / 2.0)),
        material: materials.add(theme::STEEL),
    });

    // Static walls the ball (and paddle) physically bounce off, instead of
    // manual clamp/reflect code. No bottom wall — a ball reaching the bottom
    // is a life lost, checked separately from physics.
    for (centre, size) in wall_specs() {
        commands.spawn((
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
            Transform::from_translation(centre.extend(0.0)),
        ));
    }
}

/// The left, right and top walls as (centre, size), with their inner faces
/// exactly on the playfield well's edges.
pub(crate) fn wall_specs() -> [(Vec2, Vec2); 3] {
    let half_w = PLAYFIELD_WIDTH / 2.0;
    let half_h = PLAYFIELD_HEIGHT / 2.0;
    let t = WALL_THICKNESS;
    let side = Vec2::new(t, PLAYFIELD_HEIGHT + 2.0 * t);
    [
        (Vec2::new(-half_w - t / 2.0, 0.0), side),
        (Vec2::new(half_w + t / 2.0, 0.0), side),
        (
            Vec2::new(0.0, half_h + t / 2.0),
            Vec2::new(PLAYFIELD_WIDTH + 2.0 * t, t),
        ),
    ]
}

#[cfg(test)]
mod tests;
