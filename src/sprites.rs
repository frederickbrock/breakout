//! Image assets: one place that loads every sprite handle ([`GameSprites`])
//! and the global play-area background.
//!
//! Sprites live at `assets/sprites/<name>.png` (the art pipeline's layout).
//!
//! Two plugins:
//! - [`SpritesPlugin`] loads the handles and draws the background. It's
//!   registered from `main()`, so the headless test app (no image loaders)
//!   doesn't need it.
//! - [`SkinPlugin`] (in `add_game`) swaps an entity's shape look for its
//!   sprite once that image has loaded. The ball, paddle pieces and power-ups
//!   always spawn as their `theme` shapes, so a missing or broken file just
//!   leaves the shape in place: no panic, nothing invisible. A new sprite is a
//!   [`GameSprites`] field plus a skin rule here.

use crate::powerups::PowerUp;
use crate::{theme, Ball, PaddleField, PaddleProng, BALL_SIZE, WINDOW_HEIGHT, WINDOW_WIDTH};
use bevy::prelude::*;

/// Paths relative to `assets/`.
const BACKGROUND_PATH: &str = "sprites/background.png";
const BALL_PATH: &str = "sprites/ball.png";
const PRONG_LEFT_PATH: &str = "sprites/paddle_prong_left.png";
const PRONG_RIGHT_PATH: &str = "sprites/paddle_prong_right.png";
const PADDLE_FIELD_PATH: &str = "sprites/paddle_field.png";
const POWER_UP_PATH: &str = "sprites/powerup.png";
/// Far behind every game entity (which all sit at z 0..1).
const BACKGROUND_Z: f32 = -10.0;

/// Handles to every image the game draws. Loaded once at startup.
#[derive(Resource)]
pub struct GameSprites {
    pub background: Handle<Image>,
    pub ball: Handle<Image>,
    /// Already mirrored in the file: no `flip_x` needed.
    pub prong_left: Handle<Image>,
    pub prong_right: Handle<Image>,
    /// Fully opaque glow core, stretched to fill between the prongs.
    pub paddle_field: Handle<Image>,
    /// One icon for every power-up kind, for now.
    pub power_up: Handle<Image>,
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
        ball: assets.load(BALL_PATH),
        prong_left: assets.load(PRONG_LEFT_PATH),
        prong_right: assets.load(PRONG_RIGHT_PATH),
        paddle_field: assets.load(PADDLE_FIELD_PATH),
        power_up: assets.load(POWER_UP_PATH),
    });
}

/// Entities with `T` still showing their shape look.
type Unskinned<T> = (With<T>, Without<Skinned>);
/// The paddle field still showing its shape look (disjoint from the prongs).
type UnskinnedField = (With<PaddleField>, Without<Skinned>, Without<PaddleProng>);

/// Marks an entity whose shape look has been swapped for its sprite.
#[derive(Component)]
pub struct Skinned;

pub struct SkinPlugin;

impl Plugin for SkinPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (skin_ball, skin_paddle, skin_power_ups).run_if(resource_exists::<GameSprites>),
        );
    }
}

/// `image`, if it has finished loading. A missing or failed file never shows
/// up in `Assets<Image>`, so the entity keeps its shape.
fn loaded<'a>(image: &'a Handle<Image>, images: &Assets<Image>) -> Option<&'a Handle<Image>> {
    images.contains(image).then_some(image)
}

/// Draw `image` in place of the sprite's flat colour, keeping its size.
fn apply(sprite: &mut Sprite, image: &Handle<Image>) {
    sprite.image = image.clone();
    sprite.color = theme::UNTINTED;
}

/// The ball spawns as a circle mesh; once `ball.png` is loaded it becomes a
/// 15×15 sprite.
fn skin_ball(
    mut commands: Commands,
    sprites: Res<GameSprites>,
    images: Res<Assets<Image>>,
    balls: Query<Entity, (With<Ball>, Without<Skinned>)>,
) {
    let Some(image) = loaded(&sprites.ball, &images) else {
        return;
    };
    for ball in &balls {
        commands
            .entity(ball)
            .remove::<(Mesh2d, MeshMaterial2d<ColorMaterial>)>()
            .insert((
                Sprite {
                    image: image.clone(),
                    custom_size: Some(Vec2::splat(BALL_SIZE)),
                    ..default()
                },
                Skinned,
            ));
    }
}

fn skin_paddle(
    mut commands: Commands,
    sprites: Res<GameSprites>,
    images: Res<Assets<Image>>,
    mut prongs: Query<(Entity, &PaddleProng, &mut Sprite), Without<Skinned>>,
    mut fields: Query<(Entity, &mut Sprite), UnskinnedField>,
) {
    for (entity, prong, mut sprite) in &mut prongs {
        let handle = if prong.side < 0.0 {
            &sprites.prong_left
        } else {
            &sprites.prong_right
        };
        if let Some(image) = loaded(handle, &images) {
            apply(&mut sprite, image);
            commands.entity(entity).insert(Skinned);
        }
    }
    if let Some(image) = loaded(&sprites.paddle_field, &images) {
        for (entity, mut sprite) in &mut fields {
            apply(&mut sprite, image);
            commands.entity(entity).insert(Skinned);
        }
    }
}

fn skin_power_ups(
    mut commands: Commands,
    sprites: Res<GameSprites>,
    images: Res<Assets<Image>>,
    mut power_ups: Query<(Entity, &mut Sprite), Unskinned<PowerUp>>,
) {
    let Some(image) = loaded(&sprites.power_up, &images) else {
        return;
    };
    for (entity, mut sprite) in &mut power_ups {
        apply(&mut sprite, image);
        commands.entity(entity).insert(Skinned);
    }
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

    use crate::test_support::*;
    use crate::BALL_SIZE;

    /// A run with `GameSprites` whose images are loaded unless listed in
    /// `missing` (a reserved-but-never-filled handle, like a failed load).
    fn app_with_sprites(missing: &[&str]) -> App {
        let mut app = app();
        app.init_asset::<Image>();
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        let mut image = |name: &str| {
            if missing.contains(&name) {
                images.reserve_handle()
            } else {
                images.add(Image::default())
            }
        };
        let sprites = GameSprites {
            background: image("background"),
            ball: image("ball"),
            prong_left: image("prong_left"),
            prong_right: image("prong_right"),
            paddle_field: image("paddle_field"),
            power_up: image("power_up"),
        };
        app.world_mut().insert_resource(sprites);
        app.update();
        app
    }

    fn the_ball(app: &mut App) -> Entity {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<Ball>>()
            .single(world)
            .unwrap()
    }

    #[test]
    fn loaded_sprites_replace_the_shapes() {
        let mut app = app_with_sprites(&[]);
        let sprites = app.world().resource::<GameSprites>();
        let (ball_img, left, right, field) = (
            sprites.ball.clone(),
            sprites.prong_left.clone(),
            sprites.prong_right.clone(),
            sprites.paddle_field.clone(),
        );

        let ball = the_ball(&mut app);
        let entity = app.world().entity(ball);
        assert!(!entity.contains::<Mesh2d>());
        let sprite = entity.get::<Sprite>().unwrap();
        assert_eq!(sprite.image, ball_img);
        assert_eq!(sprite.custom_size, Some(Vec2::splat(BALL_SIZE)));

        let world = app.world_mut();
        for (prong, sprite) in world.query::<(&PaddleProng, &Sprite)>().iter(world) {
            let expected = if prong.side < 0.0 { &left } else { &right };
            assert_eq!(&sprite.image, expected);
            assert_eq!(sprite.color, theme::UNTINTED);
        }
        let field_sprite = world
            .query_filtered::<&Sprite, With<PaddleField>>()
            .single(world)
            .unwrap();
        assert_eq!(field_sprite.image, field);
    }

    #[test]
    fn a_missing_ball_sprite_keeps_the_ball_as_a_circle() {
        let mut app = app_with_sprites(&["ball"]);
        let ball = the_ball(&mut app);
        let entity = app.world().entity(ball);
        assert!(entity.contains::<Mesh2d>());
        assert!(!entity.contains::<Sprite>());
        assert!(!entity.contains::<Skinned>());
        // The paddle still gets its sprites.
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<(), (With<PaddleProng>, With<Skinned>)>()
                .iter(world)
                .count(),
            2
        );
    }

    #[test]
    fn falling_power_ups_get_the_icon_or_keep_their_colour() {
        use crate::powerups::test_spawn_power_up;
        let mut app = app_with_sprites(&[]);
        let power_up = test_spawn_power_up(&mut app);
        app.update();
        let icon = app.world().resource::<GameSprites>().power_up.clone();
        assert_eq!(app.world().get::<Sprite>(power_up).unwrap().image, icon);

        let mut app = app_with_sprites(&["power_up"]);
        let power_up = test_spawn_power_up(&mut app);
        app.update();
        let sprite = app.world().get::<Sprite>(power_up).unwrap();
        assert_ne!(sprite.color, theme::UNTINTED, "keeps its flat colour");
    }
}
