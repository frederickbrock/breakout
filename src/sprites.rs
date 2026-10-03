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
//! is skinned with its class's plate ([`BrickSprite::of`]) once that image
//! is loaded. The plate follows the damage ladder
//! ([`crate::bricks::damage_look`]):
//! - `<class>_intact.png` at full health
//! - `<class>_cracked.png` (tungsten only, by default)
//! - `<class>_broken.png` once the next hit would destroy it
//!
//! It's recomputed every frame from `BrickHealth` against the brick's own
//! `BrickMaxHits` (a level's `hits=` shifts the ladder), so a regen heal goes back
//! to intact. Every damage plate that ships is preloaded, not only the ones a
//! class's default hits reach. A missing damage plate keeps the intact one, and a missing
//! intact plate keeps the flat colour. Damage is never a tint (the smoke
//! particles sit on top), and shield deflects are shown by particles.
//!
//! [`SpritesPlugin`] loads every handle once at `Startup` into the
//! [`GameSprites`] resource (background, ball, paddle prongs and field,
//! power-up icon, one image per brick material and its damage plates, the two
//! frame panels) and spawns the global [`Background`], sized to the playfield well. The side panels hold the
//! steel frame ([`crate::frame`]), skinned with `frame_left`/`frame_right`.
//! A skinned entity is marked [`Skinned`].

use crate::ball::{Ball, BALL_SIZE};
use crate::bricks::grid::Brick;
use crate::bricks::grid::BrickHealth;
use crate::bricks::grid::BrickMaxHits;
use crate::bricks::{damage_look, BrickClass, DamageLook};
use crate::frame::{FramePanel, FramePiece};
use crate::paddle::{PaddleField, PaddleProng};
use crate::powerups::capsules::CapsuleIcon;
use crate::powerups::PowerUp;
use crate::theme;
use crate::world::{PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH};
use bevy::platform::collections::HashMap;
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

    /// A brick class drawing this sprite (explosives: any variant), for
    /// asking the damage ladder which plates its default hits need.
    #[cfg(test)]
    fn class(self) -> BrickClass {
        match self {
            Self::Ceramic => BrickClass::Ceramic,
            Self::Titanium => BrickClass::Titanium,
            Self::Tungsten => BrickClass::Tungsten,
            Self::Reactor => BrickClass::Reactor,
            Self::Explosive => BrickClass::Explosive(crate::bricks::ExplosiveKind::Charge),
            Self::Regen => BrickClass::Regen,
            Self::Shield => BrickClass::Shield,
        }
    }

    /// The damage plates that ship for this material, all preloaded: a
    /// level's `hits=` can put any brick on any rung of the ladder
    /// ([`damage_look`] of its own `BrickMaxHits`), not just the rungs its
    /// class's default hits reach. A look without a plate here draws the
    /// intact plate.
    pub fn damage_looks(self) -> &'static [DamageLook] {
        use DamageLook::{Broken, Cracked};
        match self {
            Self::Ceramic | Self::Explosive | Self::Shield => &[Cracked],
            Self::Titanium | Self::Tungsten | Self::Reactor | Self::Regen => &[Cracked, Broken],
        }
    }

    /// The path of a damage plate, relative to `assets/`:
    /// `sprites/bricks/<class>_cracked.png` / `_broken.png`.
    pub fn damage_path(self, look: DamageLook) -> String {
        let stem = self.path().trim_end_matches("_intact.png");
        let suffix = match look {
            DamageLook::Intact => "intact",
            DamageLook::Cracked => "cracked",
            DamageLook::Broken => "broken",
        };
        format!("{stem}_{suffix}.png")
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
    /// Every damage plate that ships, per material
    /// ([`BrickSprite::damage_looks`]).
    pub damaged: HashMap<(BrickSprite, DamageLook), Handle<Image>>,
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

    /// Every handle, e.g. to wait for them all to load.
    pub fn all(&self) -> impl Iterator<Item = &Handle<Image>> {
        [
            &self.background,
            &self.ball,
            &self.prong_left,
            &self.prong_right,
            &self.paddle_field,
            &self.power_up,
            &self.frame_left,
            &self.frame_right,
        ]
        .into_iter()
        .chain(&self.bricks)
        .chain(self.damaged.values())
    }

    /// The plate to draw for `look`, if it's loaded, else the intact plate
    /// if that's loaded. `None` keeps the flat colour.
    fn plate<'a>(
        &'a self,
        sprite: BrickSprite,
        look: DamageLook,
        images: &Assets<Image>,
    ) -> Option<&'a Handle<Image>> {
        let damaged = (look != DamageLook::Intact)
            .then(|| self.damaged.get(&(sprite, look)))
            .flatten()
            .and_then(|image| loaded(image, images));
        damaged.or_else(|| loaded(self.brick(sprite), images))
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
        damaged: BrickSprite::ALL
            .into_iter()
            .flat_map(|sprite| {
                sprite
                    .damage_looks()
                    .iter()
                    .map(move |&look| (sprite, look))
            })
            .map(|(sprite, look)| ((sprite, look), assets.load(sprite.damage_path(look))))
            .collect(),
        frame_left: assets.load(FRAME_LEFT_PATH),
        frame_right: assets.load(FRAME_RIGHT_PATH),
    });
}

/// Falling power-ups and the time capsules' icons share the power-up image.
type UnskinnedPowerUpIcon = (Or<(With<PowerUp>, With<CapsuleIcon>)>, Without<Skinned>);
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
    mut power_ups: Query<(Entity, &mut Sprite), UnskinnedPowerUpIcon>,
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

/// What [`skin_bricks`] reads and writes on each brick.
type BrickPlate = (
    Entity,
    &'static BrickClass,
    &'static BrickHealth,
    &'static BrickMaxHits,
    &'static mut Sprite,
    Has<Skinned>,
);

/// Each brick draws the plate its health calls for ([`damage_look`]) once
/// that image is loaded, falling back to the intact plate, then to its flat
/// colour. Checked every frame (cheap: it only writes on a change), so
/// damage, regen heals and late-loading images all show without extra
/// bookkeeping.
fn skin_bricks(
    mut commands: Commands,
    sprites: Res<GameSprites>,
    images: Res<Assets<Image>>,
    mut bricks: Query<BrickPlate, With<Brick>>,
) {
    for (entity, &class, health, max_hits, mut sprite, skinned) in &mut bricks {
        let look = damage_look(max_hits.0, health.0);
        let Some(image) = sprites.plate(BrickSprite::of(class), look, &images) else {
            continue;
        };
        if !skinned {
            apply(&mut sprite, image);
            commands.entity(entity).insert(Skinned);
        } else if sprite.image != *image {
            sprite.image = image.clone();
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
