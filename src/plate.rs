//! Dark backing plates: rounded, semi-transparent panels that keep the HUD
//! and the power-up capsules readable over the busy painted frame art.
//!
//! An entity with a [`BackingPlate`] gets a rounded-rectangle mesh of that
//! size in [`theme::HUD_PLATE`] from [`dress_plates`], so the HUD and the
//! capsules only decide where a plate goes and how big it is (their pure
//! `*_plate_rect` helpers). Plates sit at world [`PLATE_Z`]: above the frame
//! (`frame::FRAME_Z`), below the HUD text and capsule contents (z 1).
//! They don't depend on the frame art, so they look the same on the coded
//! frame.

use bevy::prelude::*;

use crate::theme;

/// World z of a backing plate: over the frame, under the text on it.
pub(crate) const PLATE_Z: f32 = 0.5;
/// Corner radius of a plate.
pub(crate) const PLATE_RADIUS: f32 = 8.0;
/// Gap between a plate's edge and the text it backs.
pub(crate) const PLATE_PADDING: f32 = 10.0;
/// Points per quarter-circle corner.
const CORNER_SEGMENTS: usize = 6;

/// A backing plate `size` wide and tall, centred on the entity.
#[derive(Component, Debug)]
pub(crate) struct BackingPlate {
    pub(crate) size: Vec2,
}

/// The plates' shared material.
#[derive(Resource)]
struct PlateMaterial(Handle<ColorMaterial>);

pub(crate) struct PlatePlugin;

impl Plugin for PlatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, make_plate_material).add_systems(
            PostUpdate,
            dress_plates.run_if(resource_exists::<PlateMaterial>),
        );
    }
}

fn make_plate_material(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    commands.insert_resource(PlateMaterial(materials.add(theme::HUD_PLATE)));
}

/// The outline of a `size` rectangle centred on the origin with corners
/// rounded by `radius` (clamped to half the shorter side), counter-clockwise.
pub(crate) fn rounded_rect_vertices(size: Vec2, radius: f32) -> Vec<Vec2> {
    let half = size / 2.0;
    let r = radius.clamp(0.0, half.min_element());
    let inner = half - Vec2::splat(r);
    // Corner centres, counter-clockwise from top-right, with each arc's start angle.
    let corners = [
        (Vec2::new(inner.x, inner.y), 0.0),
        (Vec2::new(-inner.x, inner.y), 0.25),
        (Vec2::new(-inner.x, -inner.y), 0.5),
        (Vec2::new(inner.x, -inner.y), 0.75),
    ];
    corners
        .iter()
        .flat_map(|&(centre, start)| {
            (0..=CORNER_SEGMENTS).map(move |i| {
                let turn = start + 0.25 * i as f32 / CORNER_SEGMENTS as f32;
                centre + r * Vec2::from_angle(turn * std::f32::consts::TAU)
            })
        })
        .collect()
}

/// Gives each new [`BackingPlate`] its rounded mesh and the plate material.
fn dress_plates(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    material: Res<PlateMaterial>,
    plates: Query<(Entity, &BackingPlate), Without<Mesh2d>>,
) {
    for (entity, plate) in &plates {
        let mesh = match ConvexPolygon::new(rounded_rect_vertices(plate.size, PLATE_RADIUS)) {
            Ok(polygon) => meshes.add(polygon),
            // Degenerate (zero-sized) plate: a plain rectangle.
            Err(_) => meshes.add(Rectangle::from_size(plate.size)),
        };
        commands
            .entity(entity)
            .insert((Mesh2d(mesh), MeshMaterial2d(material.0.clone())));
    }
}

#[cfg(test)]
mod tests;
