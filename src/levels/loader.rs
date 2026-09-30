//! The asset side of levels: [`LevelLoader`] (`.level` files into
//! [`LevelDef`]), [`CampaignLoader`] (`campaign.txt` into [`Campaign`]) and
//! [`LevelsPlugin`], which loads the campaign at startup, follows it to its
//! first level and keeps [`CurrentLevel`] in step with that asset.
//!
//! A loader error (a [`LevelError`](super::parse::LevelError)) is logged by
//! Bevy as "Failed to load asset '<path>' ...: line L, column C: ...", and
//! [`sync_current_level`] then removes [`CurrentLevel`] so the next run
//! plays the built-in random board. Natively, saving a level file reloads it
//! (Bevy's `file_watcher`) and the new version applies at the next run.

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoadFailedEvent, AssetLoader, LoadContext};
use bevy::prelude::*;

use super::{parse_campaign, parse_level, Campaign, CurrentLevel, LevelDef};

/// The campaign manifest, relative to `assets/`.
const CAMPAIGN_PATH: &str = "levels/campaign.txt";
/// Where the manifest's level file names live, relative to `assets/`.
const LEVELS_DIR: &str = "levels";

/// Loads `.level` files with [`parse_level`]. A file without `name:` is named
/// after its file stem.
#[derive(Default, TypePath)]
pub struct LevelLoader;

impl AssetLoader for LevelLoader {
    type Asset = LevelDef;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<LevelDef, BevyError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let mut def = parse_level(std::str::from_utf8(&bytes)?)?;
        if def.name.is_empty() {
            def.name = load_context
                .path()
                .path()
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
        Ok(def)
    }

    fn extensions(&self) -> &[&str] {
        &["level"]
    }
}

/// Loads the campaign manifest with [`parse_campaign`]. Bevy matches the
/// extension after the file name's first dot, so `campaign.txt` is claimed
/// as `txt` (no other plugin here loads `txt`).
#[derive(Default, TypePath)]
pub struct CampaignLoader;

impl AssetLoader for CampaignLoader {
    type Asset = Campaign;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Campaign, BevyError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(parse_campaign(std::str::from_utf8(&bytes)?))
    }

    fn extensions(&self) -> &[&str] {
        &["txt"]
    }
}

/// The campaign and the level it currently points at (its first entry).
#[derive(Resource, Default)]
pub(crate) struct LevelHandles {
    pub(crate) campaign: Handle<Campaign>,
    pub(crate) level: Option<Handle<LevelDef>>,
}

/// Loads levels from files. Registered from `main()` only, so the headless
/// tests never read `assets/`. Its `Update` systems are deliberately not
/// gated on the game state: they only keep [`CurrentLevel`] current, and
/// `start_run` reads it once per run, so a change never lands mid-board.
pub struct LevelsPlugin;

impl Plugin for LevelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<LevelDef>()
            .init_asset::<Campaign>()
            .register_asset_loader(LevelLoader)
            .register_asset_loader(CampaignLoader)
            .init_resource::<LevelHandles>()
            .add_systems(Startup, load_campaign)
            .add_systems(Update, (follow_campaign, sync_current_level).chain());
    }
}

fn load_campaign(server: Res<AssetServer>, mut handles: ResMut<LevelHandles>) {
    handles.campaign = server.load(CAMPAIGN_PATH);
}

/// When the campaign loads or changes, loads its first level.
pub(crate) fn follow_campaign(
    mut events: MessageReader<AssetEvent<Campaign>>,
    campaigns: Res<Assets<Campaign>>,
    server: Res<AssetServer>,
    mut handles: ResMut<LevelHandles>,
) {
    let campaign = handles.campaign.id();
    let touched = events
        .read()
        .filter(|event| event.is_loaded_with_dependencies(campaign) || event.is_modified(campaign))
        .count()
        > 0;
    if !touched {
        return;
    }
    match campaigns.get(campaign).and_then(|c| c.levels.first()) {
        Some(file) => {
            let level = server.load(format!("{LEVELS_DIR}/{file}"));
            if handles.level.as_ref() != Some(&level) {
                handles.level = Some(level);
            }
        }
        None => {
            warn!("{CAMPAIGN_PATH} lists no levels; playing the built-in random board");
            handles.level = None;
        }
    }
}

/// Copies the campaign's level into [`CurrentLevel`] whenever it loads,
/// reloads or the campaign points somewhere else; removes it when the load
/// failed or there is no level, so runs fall back to the random board.
pub(crate) fn sync_current_level(
    mut commands: Commands,
    handles: Res<LevelHandles>,
    levels: Res<Assets<LevelDef>>,
    mut events: MessageReader<AssetEvent<LevelDef>>,
    mut failed: MessageReader<AssetLoadFailedEvent<LevelDef>>,
) {
    let Some(handle) = &handles.level else {
        events.clear();
        failed.clear();
        if handles.is_changed() {
            commands.remove_resource::<CurrentLevel>();
        }
        return;
    };
    let id = handle.id();
    let touched = events
        .read()
        .filter(|event| event.is_loaded_with_dependencies(id) || event.is_modified(id))
        .count()
        > 0;
    // A failure wins over any earlier load in the same frame; a later valid
    // save sends `Modified` again and restores the level.
    let mut failed_now = false;
    for failure in failed.read().filter(|failure| failure.id == id) {
        warn!(
            "{}: playing the built-in random board instead",
            failure.path
        );
        failed_now = true;
    }
    if failed_now {
        commands.remove_resource::<CurrentLevel>();
        return;
    }
    if !touched && !handles.is_changed() {
        return;
    }
    match levels.get(id) {
        Some(def) => {
            info!(
                "level \"{}\" ready ({}x{})",
                def.name,
                def.grid.len(),
                def.cols()
            );
            commands.insert_resource(CurrentLevel(def.clone()));
        }
        None if handles.is_changed() => commands.remove_resource::<CurrentLevel>(),
        None => {}
    }
}

#[cfg(test)]
mod tests;
