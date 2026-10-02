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
//!
//! Bricks follow the same rule: each spawns as its class's flat colour and
//! is skinned with its class's intact sprite ([`BrickSprite::of`]) once that
//! image is loaded, untinted; damage and shield deflects are shown by
//! particles.
//!
//! [`SpritesPlugin`] loads every handle once at `Startup` into the
//! [`GameSprites`] resource (background, ball, paddle prongs and field,
//! power-up icon, one image per brick material, the two frame panels) and spawns the global
//! [`Background`], sized to the playfield well. The side panels hold the
//! steel frame ([`crate::frame`]), skinned with `frame_left`/`frame_right`.
//! A skinned entity is marked [`Skinned`].

use crate::ball::{Ball, BALL_SIZE};
use crate::bricks::grid::Brick;
use crate::bricks::BrickClass;
use crate::frame::{FramePanel, FramePiece};
use crate::paddle::{PaddleField, PaddleProng};
use crate::powerups::PowerUp;
use crate::theme;
use crate::world::{PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH};
use bevy::prelude::*;

/// Paths relative to `assets/`.
const BACKGROUND_PATH: &str = "sprites/background.png";
const BALL_PATH: &str = "sprites/ball.png";
const PRONG_LEFT_PATH: &str = "sprites/paddle_prong_left.png";
const PRONG_RIGHT_PATH: &str = "sprites/paddle_prong_right.png";
const PADDLE_FIELD_PATH: &str = "sprites/paddle_field.png";
const POWER_UP_PATH: &str = "sprites/powerup.png";
const FRAME_LEFT_PATH: &str = "sprites/frame_left.png";
const FRAME_RIGHT_PATH: &str = "sprites/frame_right.png";
/// Which brick sprite a brick draws: one per material. The three explosive
/// variants share one plate (their outlines tell them apart).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BrickSprite {
    Ceramic,
    Titanium,
    Tungsten,
    Reactor,
    Explosive,
    Regen,
    Shield,
}

impl BrickSprite {
    /// Every brick sprite; also the order of [`GameSprites::bricks`].
    pub const ALL: [Self; 7] = [
        Self::Ceramic,
        Self::Titanium,
        Self::Tungsten,
        Self::Reactor,
        Self::Explosive,
        Self::Regen,
        Self::Shield,
    ];

    /// The sprite a brick of `class` draws.
    pub fn of(class: BrickClass) -> Self {
        match class {
            BrickClass::Ceramic => Self::Ceramic,
            BrickClass::Titanium => Self::Titanium,
            BrickClass::Tungsten => Self::Tungsten,
            BrickClass::Reactor => Self::Reactor,
            BrickClass::Explosive(_) => Self::Explosive,
            BrickClass::Regen => Self::Regen,
            BrickClass::Shield => Self::Shield,
        }
    }

    /// The intact sprite's path, relative to `assets/`.
    pub fn path(self) -> &'static str {
        match self {
            Self::Ceramic => "sprites/bricks/ceramic_intact.png",
            Self::Titanium => "sprites/bricks/titanium_intact.png",
            Self::Tungsten => "sprites/bricks/tungsten_intact.png",
            Self::Reactor => "sprites/bricks/reactor_intact.png",
            Self::Explosive => "sprites/bricks/explosive_intact.png",
            Self::Regen => "sprites/bricks/regen_intact.png",
            Self::Shield => "sprites/bricks/shield_intact.png",
        }
    }
}

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
    /// Intact brick plates, in [`BrickSprite::ALL`] order.
    pub bricks: [Handle<Image>; 7],
    /// The painted side-panel frame. Shipped at 2×, drawn at panel size;
    /// the right one is already mirrored in the file.
    pub frame_left: Handle<Image>,
    pub frame_right: Handle<Image>,
}

impl GameSprites {
    pub fn brick(&self, sprite: BrickSprite) -> &Handle<Image> {
        // `ALL` lists the variants in declaration order.
        &self.bricks[sprite as usize]
    }
}

/// The playfield-well background, sized to the well (the side panels stay
/// clear colour). Global (not scoped to a run), so the menus show it too.
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
        bricks: BrickSprite::ALL.map(|sprite| assets.load(sprite.path())),
        frame_left: assets.load(FRAME_LEFT_PATH),
        frame_right: assets.load(FRAME_RIGHT_PATH),
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
            (
                skin_ball,
                skin_paddle,
                skin_power_ups,
                skin_frame,
                skin_bricks,
            )
                .run_if(resource_exists::<GameSprites>),
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

/// Each side panel draws its painted frame once that image is loaded, at the
/// panel's size (so the 2× art is scaled down); its coded girder, ties and
/// edge lip are hidden. A panel whose file is missing keeps the coded frame.
fn skin_frame(
    mut commands: Commands,
    sprites: Res<GameSprites>,
    images: Res<Assets<Image>>,
    mut panels: Query<(Entity, &FramePanel, &mut Sprite, Option<&Children>), Without<Skinned>>,
    pieces: Query<(), With<FramePiece>>,
) {
    for (entity, panel, mut sprite, children) in &mut panels {
        let handle = if panel.side < 0.0 {
            &sprites.frame_left
        } else {
            &sprites.frame_right
        };
        let Some(image) = loaded(handle, &images) else {
            continue;
        };
        apply(&mut sprite, image);
        commands.entity(entity).insert(Skinned);
        for &child in children.into_iter().flatten() {
            if pieces.contains(child) {
                commands.entity(child).insert(Visibility::Hidden);
            }
        }
    }
}

/// Each brick draws its class's intact plate once that image is loaded; a
/// class whose file is missing keeps its flat colour.
fn skin_bricks(
    mut commands: Commands,
    sprites: Res<GameSprites>,
    images: Res<Assets<Image>>,
    mut bricks: Query<(Entity, &BrickClass, &mut Sprite), Unskinned<Brick>>,
) {
    for (entity, &class, mut sprite) in &mut bricks {
        if let Some(image) = loaded(sprites.brick(BrickSprite::of(class)), &images) {
            apply(&mut sprite, image);
            commands.entity(entity).insert(Skinned);
        }
    }
}

fn spawn_background(mut commands: Commands, sprites: Res<GameSprites>) {
    commands.spawn((
        Background,
        Sprite {
            image: sprites.background.clone(),
            custom_size: Some(Vec2::new(PLAYFIELD_WIDTH, PLAYFIELD_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, BACKGROUND_Z),
    ));
}

#[cfg(test)]
mod tests;
