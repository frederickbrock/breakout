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
        Some(Vec2::new(PLAYFIELD_WIDTH, PLAYFIELD_HEIGHT))
    );
    assert_eq!(transform.translation.z, BACKGROUND_Z);
}

use crate::ball::{BALL_SIZE, BALL_SPEED};
use crate::bricks::grid::{BrickHealth, BRICK_HEIGHT, BRICK_WIDTH};
use crate::test_support::*;

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
        bricks: BrickSprite::ALL.map(|b| image(b.path())),
        damaged: BrickSprite::ALL
            .into_iter()
            .flat_map(|b| b.damage_looks().into_iter().map(move |l| (b, l)))
            .map(|(b, l)| ((b, l), image(&b.damage_path(l))))
            .collect(),
        frame_left: image("frame_left"),
        frame_right: image("frame_right"),
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

#[test]
fn time_capsule_icons_use_the_power_up_image() {
    use crate::powerups::capsules::CapsuleIcon;
    let mut app = app_with_sprites(&[]);
    app.world_mut().trigger(crate::powerups::PowerUpCollected {
        kind: crate::powerups::PowerUpKind::SuperSizer,
    });
    app.update(); // the capsule spawns
    app.update(); // and is skinned
    let icon = app.world().resource::<GameSprites>().power_up.clone();
    let images: Vec<_> = app
        .world_mut()
        .query_filtered::<&Sprite, With<CapsuleIcon>>()
        .iter(app.world())
        .map(|s| s.image.clone())
        .collect();
    assert_eq!(images, [icon]);
}

#[test]
fn each_brick_class_picks_its_own_intact_sprite() {
    use crate::bricks::ExplosiveKind::*;
    let cases = [
        (BrickClass::Ceramic, "ceramic"),
        (BrickClass::Titanium, "titanium"),
        (BrickClass::Tungsten, "tungsten"),
        (BrickClass::Reactor, "reactor"),
        (BrickClass::Explosive(Charge), "explosive"),
        (BrickClass::Explosive(Breach), "explosive"),
        (BrickClass::Explosive(Demolition), "explosive"),
        (BrickClass::Regen, "regen"),
        (BrickClass::Shield, "shield"),
    ];
    for (class, name) in cases {
        assert_eq!(
            BrickSprite::of(class).path(),
            format!("sprites/bricks/{name}_intact.png"),
            "{class:?}"
        );
    }
    // One handle slot per sprite, in `ALL` order.
    for (i, sprite) in BrickSprite::ALL.into_iter().enumerate() {
        assert_eq!(sprite as usize, i);
    }
}

#[test]
fn every_brick_sprite_file_ships() {
    for sprite in BrickSprite::ALL {
        let path = std::path::Path::new("assets").join(sprite.path());
        assert!(path.is_file(), "{} missing", path.display());
    }
}

fn brick_sprite(app: &App, brick: Entity) -> Sprite {
    app.world().get::<Sprite>(brick).unwrap().clone()
}

#[test]
fn every_brick_draws_its_class_sprite_untinted() {
    let mut app = app_with_sprites(&[]);
    let sprites = app.world().resource::<GameSprites>().bricks.clone();
    let world = app.world_mut();
    let bricks: Vec<_> = world
        .query_filtered::<(&BrickClass, &Sprite, Has<Skinned>), With<Brick>>()
        .iter(world)
        .map(|(c, s, skinned)| (*c, s.clone(), skinned))
        .collect();
    assert_eq!(bricks.len(), 70);
    for (class, sprite, skinned) in bricks {
        assert!(skinned, "{class:?}");
        assert_eq!(sprite.image, sprites[BrickSprite::of(class) as usize]);
        assert_eq!(sprite.color, theme::UNTINTED);
        assert_eq!(
            sprite.custom_size,
            Some(Vec2::new(BRICK_WIDTH, BRICK_HEIGHT))
        );
    }
}

#[test]
fn a_missing_brick_sprite_keeps_that_class_flat() {
    let mut app = app_with_sprites(&[BrickSprite::Ceramic.path()]);
    let ceramic = brick_of(&mut app, BrickClass::Ceramic);
    assert!(!app.world().entity(ceramic).contains::<Skinned>());
    assert_eq!(brick_sprite(&app, ceramic).color, theme::CERAMIC);
    let titanium = brick_of(&mut app, BrickClass::Titanium);
    assert!(app.world().entity(titanium).contains::<Skinned>());
}

/// The plate image a brick currently draws.
fn plate(app: &App, brick: Entity) -> Handle<Image> {
    brick_sprite(app, brick).image
}

fn damage_plate(app: &App, sprite: BrickSprite, look: DamageLook) -> Handle<Image> {
    app.world().resource::<GameSprites>().damaged[&(sprite, look)].clone()
}

fn intact_plate(app: &App, sprite: BrickSprite) -> Handle<Image> {
    app.world().resource::<GameSprites>().brick(sprite).clone()
}

#[test]
fn every_ladder_plate_exists_on_disk() {
    for sprite in BrickSprite::ALL {
        for look in sprite.damage_looks() {
            let path = format!("assets/{}", sprite.damage_path(look));
            assert!(std::path::Path::new(&path).exists(), "{path}");
        }
    }
    assert_eq!(
        BrickSprite::Tungsten.damage_path(DamageLook::Cracked),
        "sprites/bricks/tungsten_cracked.png"
    );
    assert!(BrickSprite::Ceramic.damage_looks().is_empty());
}

#[test]
fn tungsten_goes_intact_cracked_broken_then_breaks() {
    let mut app = app_with_sprites(&[]);
    let tungsten = brick_of(&mut app, BrickClass::Tungsten);
    assert_eq!(
        plate(&app, tungsten),
        intact_plate(&app, BrickSprite::Tungsten)
    );
    hit(&mut app, tungsten);
    app.update();
    let cracked = damage_plate(&app, BrickSprite::Tungsten, DamageLook::Cracked);
    assert_eq!(plate(&app, tungsten), cracked);
    hit(&mut app, tungsten);
    app.update();
    let broken = damage_plate(&app, BrickSprite::Tungsten, DamageLook::Broken);
    assert_eq!(plate(&app, tungsten), broken);
    assert_eq!(
        brick_sprite(&app, tungsten).color,
        theme::UNTINTED,
        "never tinted"
    );
    hit(&mut app, tungsten);
    assert!(
        app.world().get_entity(tungsten).is_err(),
        "broken means the next hit kills"
    );
}

#[test]
fn two_hit_bricks_go_straight_to_broken() {
    for (class, sprite) in [
        (BrickClass::Titanium, BrickSprite::Titanium),
        (BrickClass::Reactor, BrickSprite::Reactor),
        (BrickClass::Regen, BrickSprite::Regen),
    ] {
        let mut app = app_with_sprites(&[]);
        let brick = brick_of(&mut app, class);
        hit(&mut app, brick);
        app.update();
        assert_eq!(
            plate(&app, brick),
            damage_plate(&app, sprite, DamageLook::Broken),
            "{class:?}"
        );
        assert_eq!(brick_sprite(&app, brick).color, theme::UNTINTED);
    }
}

#[test]
fn a_healed_regen_brick_is_intact_again() {
    let mut app = app_with_sprites(&[]);
    let regen = brick_of(&mut app, BrickClass::Regen);
    hit(&mut app, regen);
    app.update();
    assert_eq!(
        plate(&app, regen),
        damage_plate(&app, BrickSprite::Regen, DamageLook::Broken)
    );
    for _ in 0..35 {
        app.update(); // past the 3 s heal
    }
    assert_eq!(app.world().get::<BrickHealth>(regen).unwrap().0, 2);
    assert_eq!(plate(&app, regen), intact_plate(&app, BrickSprite::Regen));
}

#[test]
fn a_missing_damage_plate_keeps_the_intact_one() {
    let missing = BrickSprite::Titanium.damage_path(DamageLook::Broken);
    let mut app = app_with_sprites(&[missing.as_str()]);
    let titanium = brick_of(&mut app, BrickClass::Titanium);
    hit(&mut app, titanium);
    app.update();
    assert_eq!(app.world().get::<BrickHealth>(titanium).unwrap().0, 1);
    assert_eq!(
        plate(&app, titanium),
        intact_plate(&app, BrickSprite::Titanium)
    );
}

#[test]
fn skinned_shield_glass_stays_untinted_when_it_deflects() {
    let mut app = app_with_sprites(&[]);
    let shield = brick_of(&mut app, BrickClass::Shield);
    hit_moving(&mut app, shield, Vec2::new(0.0, BALL_SPEED));
    app.update();
    assert_eq!(brick_sprite(&app, shield).color, theme::UNTINTED);
}

fn frame_panel(app: &mut App, side: f32) -> (Entity, Sprite, bool) {
    let world = app.world_mut();
    world
        .query::<(Entity, &FramePanel, &Sprite, Has<Skinned>)>()
        .iter(world)
        .find(|(_, panel, _, _)| panel.side == side)
        .map(|(entity, _, sprite, skinned)| (entity, sprite.clone(), skinned))
        .unwrap()
}

fn hidden_frame_pieces(app: &mut App, panel: Entity) -> (usize, usize) {
    let world = app.world_mut();
    let pieces: Vec<_> = world
        .query_filtered::<(&ChildOf, &Visibility), With<FramePiece>>()
        .iter(world)
        .filter(|(child_of, _)| child_of.parent() == panel)
        .map(|(_, visibility)| *visibility == Visibility::Hidden)
        .collect();
    (
        pieces.iter().filter(|&&hidden| hidden).count(),
        pieces.len(),
    )
}

#[test]
fn loaded_frame_art_replaces_the_coded_frame_at_panel_size() {
    let mut app = app_with_sprites(&[]);
    let sprites = app.world().resource::<GameSprites>();
    let (left_img, right_img) = (sprites.frame_left.clone(), sprites.frame_right.clone());
    for (side, image) in [(-1.0, left_img), (1.0, right_img)] {
        let (panel, sprite, skinned) = frame_panel(&mut app, side);
        assert!(skinned);
        assert_eq!(sprite.image, image);
        assert_eq!(sprite.color, theme::UNTINTED);
        assert_eq!(sprite.custom_size, Some(crate::frame::panel_size()));
        let (hidden, total) = hidden_frame_pieces(&mut app, panel);
        assert!(total > 0);
        assert_eq!(hidden, total, "coded pieces hidden under the art");
    }
}

#[test]
fn a_missing_frame_image_keeps_that_panels_coded_frame() {
    let mut app = app_with_sprites(&["frame_left"]);
    let (left, sprite, skinned) = frame_panel(&mut app, -1.0);
    assert!(!skinned);
    assert_eq!(sprite.color, theme::FRAME_PANEL);
    assert_eq!(hidden_frame_pieces(&mut app, left).0, 0);
    // The right one still skins.
    let (_, _, right_skinned) = frame_panel(&mut app, 1.0);
    assert!(right_skinned);
}
