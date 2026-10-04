use super::*;
use crate::bricks::DamageLook;
use crate::sprites::{BrickSprite, GameSprites};
use crate::test_support::launch;

/// A `GameSprites` whose every handle is `handle`.
fn sprites_of(handle: Handle<Image>) -> GameSprites {
    GameSprites {
        background: handle.clone(),
        ball: handle.clone(),
        prong_left: handle.clone(),
        prong_right: handle.clone(),
        paddle_field: handle.clone(),
        power_up: handle.clone(),
        bricks: std::array::from_fn(|_| handle.clone()),
        damaged: [((BrickSprite::ALL[0], DamageLook::Cracked), handle.clone())].into(),
        frame_left: handle.clone(),
        frame_right: handle.clone(),
        parallax: std::array::from_fn(|_| handle.clone()),
        outlines: std::array::from_fn(|_| handle.clone()),
    }
}

/// The launched app with images available and its hand-over counters reset.
fn app_with_images() -> App {
    let mut app = launch();
    app.init_asset::<Image>();
    *app.world_mut().resource_mut::<SplashHandOver>() = SplashHandOver::default();
    app
}

fn done(app: &App) -> bool {
    app.world().resource::<SplashHandOver>().done
}

fn run_frames(app: &mut App, n: u32) {
    for _ in 0..n {
        app.update();
    }
}

#[test]
fn hands_over_a_few_frames_after_every_sprite_has_loaded() {
    let mut app = app_with_images();
    let loaded = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    app.insert_resource(sprites_of(loaded));

    run_frames(&mut app, HAND_OVER_FRAMES - 1);
    assert!(!done(&app), "the menu needs a few frames on screen first");
    app.update();
    assert!(done(&app));
}

#[test]
fn waits_while_a_sprite_is_still_loading() {
    let mut app = app_with_images();
    // Never added to `Assets<Image>` and not failed: still pending.
    app.insert_resource(sprites_of(Handle::default()));

    run_frames(&mut app, HAND_OVER_FRAMES * 10);
    assert!(!done(&app));
}

#[test]
fn waits_until_the_sprites_are_asked_for() {
    let mut app = app_with_images();
    run_frames(&mut app, HAND_OVER_FRAMES * 10);
    assert!(!done(&app));
}

#[test]
fn hands_over_anyway_if_loading_never_settles() {
    let mut app = app_with_images();
    app.insert_resource(sprites_of(Handle::default()));

    run_frames(&mut app, MAX_WAIT_FRAMES);
    assert!(done(&app), "the splash must never get stuck");
}

#[test]
fn hands_over_only_once() {
    let mut app = app_with_images();
    let loaded = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    app.insert_resource(sprites_of(loaded));
    run_frames(&mut app, HAND_OVER_FRAMES);
    let frames = app.world().resource::<SplashHandOver>().frames;

    run_frames(&mut app, 5);
    assert_eq!(
        app.world().resource::<SplashHandOver>().frames,
        frames,
        "the system stops running once the page has been told"
    );
}

#[test]
fn waits_while_the_tuning_file_is_still_loading() {
    let mut app = app_with_images();
    let loaded = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    app.insert_resource(sprites_of(loaded));
    // Not loaded by the asset server and not failed: still pending.
    app.insert_resource(TuningHandle::default());

    run_frames(&mut app, HAND_OVER_FRAMES * 10);
    assert!(!done(&app), "the tuning file hasn't settled");

    app.world_mut().remove_resource::<TuningHandle>();
    run_frames(&mut app, HAND_OVER_FRAMES);
    assert!(done(&app));
}

#[test]
fn hands_over_once_the_real_tuning_file_has_loaded() {
    let mut app = crate::test_support::launch_with(|app| {
        app.init_asset::<Image>()
            .add_plugins(crate::tuning::TuningPlugin);
    });
    *app.world_mut().resource_mut::<SplashHandOver>() = SplashHandOver::default();
    let loaded = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    app.insert_resource(sprites_of(loaded));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !done(&app) {
        assert!(std::time::Instant::now() < deadline, "never handed over");
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let handle = app.world().resource::<TuningHandle>().0.clone();
    assert!(app
        .world()
        .resource::<AssetServer>()
        .load_state(&handle)
        .is_loaded());
}
