//! Hand-over from the web page's loading splash to the game.
//!
//! On the web build `index.html` shows a Steelbreak splash while the wasm
//! downloads and starts (driven by `web/loader.js`). The splash must stay up
//! until the game has really drawn its main menu, which is after the sprites
//! have loaded, not when the wasm starts. [`hand_over_splash`] waits until
//! every [`GameSprites`] image and the tuning file ([`TuningHandle`], when
//! the tuning plugin is in the app) have settled (loaded or failed), lets
//! [`HAND_OVER_FRAMES`] more frames render, then tells the page once by
//! dispatching a `steelbreak-ready` event on `window`. The page then fades the
//! splash out. If an image never settles, it hands over anyway after
//! [`MAX_WAIT_FRAMES`], so the splash can't get stuck.
//!
//! The DOM call is wasm-only; natively (and in the headless tests) the plugin
//! runs the same bookkeeping and the call is a no-op.

use crate::sprites::GameSprites;
use crate::tuning::TuningHandle;
use bevy::prelude::*;

/// Frames rendered after the sprites settle before the splash goes, so the
/// menu is on screen under it when it fades.
pub const HAND_OVER_FRAMES: u32 = 3;
/// Frames after which the splash is handed over even if an image is still
/// pending (a few seconds at 60 fps).
pub const MAX_WAIT_FRAMES: u32 = 600;

pub struct WebSplashPlugin;

impl Plugin for WebSplashPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SplashHandOver>().add_systems(
            Last,
            hand_over_splash.run_if(|hand_over: Res<SplashHandOver>| !hand_over.done),
        );
    }
}

/// Progress of the hand-over; `done` once the page has been told.
#[derive(Resource, Debug, Default)]
pub struct SplashHandOver {
    /// Frames since the app started.
    pub frames: u32,
    /// Consecutive frames with every sprite settled.
    pub settled_frames: u32,
    pub done: bool,
}

/// Whether every sprite has finished loading, one way or the other. Before
/// [`GameSprites`] exists nothing has been asked for yet, so not settled.
fn sprites_settled(
    sprites: Option<&GameSprites>,
    images: Option<&Assets<Image>>,
    assets: &AssetServer,
) -> bool {
    let Some(sprites) = sprites else {
        return false;
    };
    sprites.all().all(|handle| {
        images.is_some_and(|images| images.contains(handle))
            || assets.load_state(handle).is_failed()
    })
}

/// Whether the tuning file has loaded or failed. Without [`TuningHandle`]
/// (no tuning plugin, e.g. the headless tests) there's nothing to wait for.
fn tuning_settled(tuning: Option<&TuningHandle>, assets: &AssetServer) -> bool {
    tuning.is_none_or(|handle| {
        let state = assets.load_state(&handle.0);
        state.is_loaded() || state.is_failed()
    })
}

fn hand_over_splash(
    mut hand_over: ResMut<SplashHandOver>,
    sprites: Option<Res<GameSprites>>,
    images: Option<Res<Assets<Image>>>,
    tuning: Option<Res<TuningHandle>>,
    assets: Res<AssetServer>,
) {
    hand_over.frames += 1;
    if sprites_settled(sprites.as_deref(), images.as_deref(), &assets)
        && tuning_settled(tuning.as_deref(), &assets)
    {
        hand_over.settled_frames += 1;
    } else {
        hand_over.settled_frames = 0;
    }
    if hand_over.settled_frames >= HAND_OVER_FRAMES || hand_over.frames >= MAX_WAIT_FRAMES {
        hand_over.done = true;
        notify_page();
    }
}

/// Tell the page the game is on screen (see `web/loader.js`).
#[cfg(target_arch = "wasm32")]
fn notify_page() {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Ok(event) = web_sys::Event::new("steelbreak-ready") {
        // The page may have no listener (e.g. a custom index.html); that's fine.
        let _ = window.dispatch_event(&event);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn notify_page() {}

#[cfg(test)]
mod tests;
