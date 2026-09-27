//! Image assets: one place that loads every sprite handle ([`GameSprites`])
//! and the global play-area background.
//!
//! Sprites live at `assets/sprites/<name>.png` (the art pipeline's layout).
//! Swapping a shape for a sprite later means adding a field here and using the
//! handle where the entity is spawned. A missing file only logs an error: the
//! sprite draws nothing and the game keeps running.
//!
//! Registered from `main()` rather than `add_game`, so the headless test app
//! (no renderer or image loaders) doesn't need it.

use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};
use bevy::prelude::*;

/// Path of the play-area background, relative to `assets/`.
const BACKGROUND_PATH: &str = "sprites/background.png";
/// Far behind every game entity (which all sit at z 0..1).
const BACKGROUND_Z: f32 = -10.0;

/// Handles to every image the game draws. Loaded once at startup.
#[derive(Resource)]
pub struct GameSprites {
    pub background: Handle<Image>,
}

/// The full-window background. Global (not scoped to a run), so the menus
/// show it too.
#[derive(Component)]
pub struct Background;

pub struct SpritesPlugin;

impl Plugin for SpritesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (load_sprites, spawn_background).chain());
    }
}

fn load_sprites(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GameSprites {
        background: assets.load(BACKGROUND_PATH),
    });
}

fn spawn_background(mut commands: Commands, sprites: Res<GameSprites>) {
    commands.spawn((
        Background,
        Sprite {
            image: sprites.background.clone(),
            custom_size: Some(Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, BACKGROUND_Z),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_loads_the_background_and_spawns_it_behind_everything() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .add_plugins(SpritesPlugin);
        app.update();

        let handle = app.world().resource::<GameSprites>().background.clone();
        assert_eq!(
            handle.path().map(|p| p.path().to_path_buf()),
            Some(BACKGROUND_PATH.into())
        );

        let world = app.world_mut();
        let (sprite, transform) = world
            .query_filtered::<(&Sprite, &Transform), With<Background>>()
            .single(world)
            .expect("exactly one background");
        assert_eq!(sprite.image, handle);
        assert_eq!(
            sprite.custom_size,
            Some(Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT))
        );
        assert_eq!(transform.translation.z, BACKGROUND_Z);
    }
}
