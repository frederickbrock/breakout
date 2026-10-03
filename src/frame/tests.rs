use super::*;
use crate::run::Lives;
use crate::test_support::*;
use crate::world::wall_specs;

fn panels(app: &mut App) -> Vec<(f32, Sprite, Vec3)> {
    let world = app.world_mut();
    let mut panels: Vec<_> = world
        .query::<(&FramePanel, &Sprite, &Transform)>()
        .iter(world)
        .map(|(p, s, t)| (p.side, s.clone(), t.translation))
        .collect();
    panels.sort_by(|a, b| a.0.total_cmp(&b.0));
    panels
}

#[test]
fn both_side_panels_spawn_with_the_coded_steel_look() {
    let mut app = launch();
    let panels = panels(&mut app);
    assert_eq!(panels.len(), 2);
    for (side, sprite, at) in panels {
        assert_eq!(sprite.color, theme::FRAME_PANEL);
        assert_eq!(sprite.custom_size, Some(panel_size()));
        assert_eq!(at.truncate(), panel_centre(side));
        assert_eq!(at.z, FRAME_Z);
    }
    // Every panel has its girder, ties and edge lip.
    let pieces = frame_pieces(-1.0).len() * 2;
    assert_eq!(count::<With<FramePiece>>(&mut app), pieces);
}

#[test]
fn the_panels_fill_the_space_beside_the_playfield_well() {
    for side in [-1.0, 1.0] {
        let centre = panel_centre(side);
        let size = panel_size();
        let inner = centre.x - side * size.x / 2.0;
        let outer = centre.x + side * size.x / 2.0;
        assert_eq!(inner, side * PLAYFIELD_WIDTH / 2.0);
        assert_eq!(outer, side * crate::world::WORLD_WIDTH / 2.0);
        assert_eq!(size.y, WORLD_HEIGHT);
    }
}

#[test]
fn the_edge_lip_sits_exactly_on_the_physics_wall() {
    let [(left, left_size), (right, right_size), _] = wall_specs();
    let wall_face = [
        (-1.0, left.x + left_size.x / 2.0),
        (1.0, right.x - right_size.x / 2.0),
    ];
    for (side, face) in wall_face {
        let (centre, size, color) = *frame_pieces(side).last().unwrap();
        assert_eq!(color, theme::FRAME_EDGE);
        let inner_edge = panel_centre(side).x + centre.x - side * size.x / 2.0;
        assert!(
            (inner_edge - face).abs() < 1e-3,
            "{side}: {inner_edge} vs {face}"
        );
    }
}

#[test]
fn frame_pieces_stay_inside_their_panel_and_use_theme_colours() {
    let half = panel_size() / 2.0;
    let colours = [theme::FRAME_GIRDER, theme::FRAME_TIE, theme::FRAME_EDGE];
    for side in [-1.0, 1.0] {
        for (centre, size, color) in frame_pieces(side) {
            assert!(colours.contains(&color));
            let (lo, hi) = (centre - size / 2.0, centre + size / 2.0);
            assert!(lo.x >= -half.x - 1e-3 && hi.x <= half.x + 1e-3, "{side}");
            assert!(lo.y >= -half.y - 1e-3 && hi.y <= half.y + 1e-3, "{side}");
        }
    }
}

#[test]
fn the_frame_stays_through_play_game_over_and_play_again() {
    // Main menu.
    let mut app = launch();
    assert_eq!(panels(&mut app).len(), 2);
    // In play and paused.
    app.world_mut()
        .resource_mut::<NextState<crate::game_state::AppState>>()
        .set(crate::game_state::AppState::InGame);
    app.update();
    assert_eq!(panels(&mut app).len(), 2);
    tap(&mut app, KeyCode::KeyP);
    assert_eq!(panels(&mut app).len(), 2);
    tap(&mut app, KeyCode::KeyP);
    // Game over, then play again.
    app.world_mut().resource_mut::<Lives>().0 = 1;
    tap(&mut app, KeyCode::Space);
    move_ball_below_screen(&mut app);
    app.update();
    app.update();
    assert_eq!(app_state(&app), crate::game_state::AppState::GameOver);
    assert_eq!(panels(&mut app).len(), 2);
    tap(&mut app, KeyCode::KeyR);
    assert_eq!(app_state(&app), crate::game_state::AppState::InGame);
    assert_eq!(panels(&mut app).len(), 2);
    assert_eq!(
        count::<With<FramePiece>>(&mut app),
        frame_pieces(-1.0).len() * 2
    );
}

/// Decodes a shipped PNG under `assets/` (width, height, RGBA8 pixels).
fn shipped_png(path: &str) -> (u32, u32, Vec<u8>) {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
    let bytes = std::fs::read(format!("assets/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"));
    let image = Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    )
    .unwrap_or_else(|e| panic!("{path}: {e}"));
    let rgba = image.convert(bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb);
    let image = rgba.unwrap_or(image);
    (
        image.width(),
        image.height(),
        image.data.unwrap_or_default(),
    )
}

#[test]
fn the_shipped_frame_art_is_2x_the_panel_and_mirrored() {
    let (lw, lh, left) = shipped_png("sprites/frame_left.png");
    let (rw, rh, right) = shipped_png("sprites/frame_right.png");
    // Drawn at panel size, so the 2× art isn't stretched.
    let size = panel_size() * 2.0;
    assert_eq!((lw, lh), (size.x as u32, size.y as u32));
    assert_eq!((rw, rh), (lw, lh));
    // The right panel is the left one flipped horizontally.
    let px = |data: &[u8], x: u32, y: u32| {
        let i = ((y * lw + x) * 4) as usize;
        data[i..i + 4].to_vec()
    };
    for y in (0..lh).step_by(37) {
        for x in (0..lw).step_by(7) {
            assert_eq!(px(&left, x, y), px(&right, lw - 1 - x, y), "({x}, {y})");
        }
    }
    // Opaque right up to the playfield-facing edge: no gap at the well.
    for y in (0..lh).step_by(37) {
        assert_eq!(px(&left, lw - 1, y)[3], 255, "left inner edge at y {y}");
    }
}
