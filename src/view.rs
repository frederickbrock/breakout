//! How the world reaches the window: one camera that always shows the whole
//! [`WORLD_WIDTH`] × [`WORLD_HEIGHT`] world, scaled uniformly to fit with
//! clear-colour bars in the spare space, and a [`UiScale`] that follows the
//! window so the menus keep their size relative to the world.

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::{GAME_SCALE, WORLD_HEIGHT, WORLD_WIDTH};

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiScale>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, follow_window_size);
    }
}

/// An orthographic projection that fits the whole world in any window: the
/// tighter axis fills it and the other gets letterbox or pillarbox bars.
pub fn world_projection() -> Projection {
    Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::AutoMin {
            min_width: WORLD_WIDTH,
            min_height: WORLD_HEIGHT,
        },
        ..OrthographicProjection::default_2d()
    })
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, world_projection()));
}

/// The UI scale for a window `width` × `height` (logical px): how big the
/// world is drawn, times [`GAME_SCALE`], so a 1280×720 window gives 1.0 and a
/// 1920×1080 one 1.5.
pub fn ui_scale_for(width: f32, height: f32) -> f32 {
    (width / WORLD_WIDTH).min(height / WORLD_HEIGHT) * GAME_SCALE
}

fn follow_window_size(
    windows: Query<&Window, (With<PrimaryWindow>, Changed<Window>)>,
    mut ui_scale: ResMut<UiScale>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let (width, height) = (window.width(), window.height());
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let scale = ui_scale_for(width, height);
    if ui_scale.0 != scale {
        ui_scale.0 = scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::app;
    use bevy::window::WindowResolution;

    #[test]
    fn the_camera_fits_the_whole_world() {
        let mut app = app();
        let world = app.world_mut();
        let projection = world
            .query_filtered::<&Projection, With<Camera2d>>()
            .single(world)
            .expect("exactly one camera");
        let Projection::Orthographic(ortho) = projection else {
            panic!("orthographic camera, got {projection:?}");
        };
        assert!(matches!(
            ortho.scaling_mode,
            ScalingMode::AutoMin { min_width, min_height }
                if min_width == WORLD_WIDTH && min_height == WORLD_HEIGHT
        ));
    }

    #[test]
    fn the_ui_scale_follows_the_tighter_window_axis() {
        assert_eq!(ui_scale_for(1280.0, 720.0), 1.0);
        assert_eq!(ui_scale_for(1920.0, 1080.0), 1.5);
        assert_eq!(ui_scale_for(3840.0, 1080.0), 1.5);
        assert_eq!(ui_scale_for(1280.0, 2000.0), 1.0);
    }

    #[test]
    fn ui_scale_tracks_the_primary_window_size() {
        let mut app = app();
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: WindowResolution::new(1920, 1080),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        app.update();
        assert_eq!(app.world().resource::<UiScale>().0, 1.5);

        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set(1280.0, 720.0);
        app.update();
        assert_eq!(app.world().resource::<UiScale>().0, 1.0);
    }
}
