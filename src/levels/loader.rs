//! The asset side of levels: [`LevelLoader`] (`.level` files into
//! [`LevelDef`]), [`CampaignLoader`] (`campaign.txt` into [`Campaign`]) and
//! [`LevelsPlugin`], which loads the campaign at startup, loads every level
//! it lists and keeps [`CampaignLevels`] in step with those assets.
//!
//! A loader error (a [`LevelError`](super::parse::LevelError)) is logged by
//! Bevy as "Failed to load asset '<path>' ...: line L, column C: ...", and
//! [`sync_campaign_levels`] then leaves that level out of [`CampaignLevels`]
//! (skipped, with a warning); with no playable level left it removes the
//! resource so runs play the built-in random board. Natively, saving a level
//! file reloads it (Bevy's `file_watcher`) and the new version applies the
//! next time that level starts.

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoadFailedEvent, AssetLoader, LoadContext};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use super::{parse_campaign, parse_level, Campaign, CampaignLevels, LevelDef};

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

/// The campaign, the levels it lists (in order) and the ones whose last
/// load failed.
#[derive(Resource, Default)]
pub(crate) struct LevelHandles {
    pub(crate) campaign: Handle<Campaign>,
    pub(crate) levels: Vec<Handle<LevelDef>>,
    pub(crate) failed: HashSet<AssetId<LevelDef>>,
}

/// Loads levels from files. Registered from `main()` only, so the headless
/// tests never read `assets/`. Its `Update` systems are deliberately not
/// gated on the game state: they only keep [`CampaignLevels`] current, and
/// the run reads a level only when it starts it, so a change never lands
/// mid-board.
pub struct LevelsPlugin;

impl Plugin for LevelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<LevelDef>()
            .init_asset::<Campaign>()
            .register_asset_loader(LevelLoader)
            .register_asset_loader(CampaignLoader)
            .init_resource::<LevelHandles>()
            .add_systems(Startup, load_campaign)
            .add_systems(Update, (follow_campaign, sync_campaign_levels).chain());
    }
}

fn load_campaign(server: Res<AssetServer>, mut handles: ResMut<LevelHandles>) {
    handles.campaign = server.load(CAMPAIGN_PATH);
}

/// When the campaign loads or changes, loads every level it lists.
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
    let wanted: Vec<Handle<LevelDef>> = campaigns
        .get(campaign)
        .map(|c| {
            c.levels
                .iter()
                .map(|file| server.load(format!("{LEVELS_DIR}/{file}")))
                .collect()
        })
        .unwrap_or_default();
    if wanted.is_empty() {
        warn!("{CAMPAIGN_PATH} lists no levels; playing the built-in random board");
    }
    if handles.levels != wanted {
        handles.levels = wanted;
    }
}

/// Rebuilds [`CampaignLevels`] from the campaign's levels, in order, whenever
/// one loads, reloads or fails, or the campaign lists different levels. A
/// level that failed to load is skipped; with none playable the resource is
/// removed, so runs fall back to the random board. While a listed level is
/// still loading the resource is left alone, so a half-loaded campaign never
/// starts at the wrong level.
pub(crate) fn sync_campaign_levels(
    mut commands: Commands,
    mut handles: ResMut<LevelHandles>,
    levels: Res<Assets<LevelDef>>,
    mut events: MessageReader<AssetEvent<LevelDef>>,
    mut failed: MessageReader<AssetLoadFailedEvent<LevelDef>>,
) {
    let changed = handles.is_changed();
    // Editing the failed set is bookkeeping, not a change of campaign.
    let handles = handles.bypass_change_detection();
    let tracked: HashSet<AssetId<LevelDef>> = handles.levels.iter().map(Handle::id).collect();
    let mut touched = false;
    for event in events.read() {
        let id = match *event {
            AssetEvent::LoadedWithDependencies { id } | AssetEvent::Modified { id } => id,
            _ => continue,
        };
        if tracked.contains(&id) {
            // A later valid save sends `Modified` again and restores the level.
            handles.failed.remove(&id);
            touched = true;
        }
    }
    // A failure wins over any earlier load in the same frame.
    for failure in failed.read().filter(|f| tracked.contains(&f.id)) {
        warn!("{}: skipping this level", failure.path);
        handles.failed.insert(failure.id);
        touched = true;
    }
    if !touched && !changed {
        return;
    }
    handles.failed.retain(|id| tracked.contains(id));
    let loading = handles
        .levels
        .iter()
        .any(|h| !levels.contains(h.id()) && !handles.failed.contains(&h.id()));
    if loading {
        return;
    }
    let playable: Vec<LevelDef> = handles
        .levels
        .iter()
        .filter(|h| !handles.failed.contains(&h.id()))
        .filter_map(|h| levels.get(h.id()).cloned())
        .collect();
    if playable.is_empty() {
        commands.remove_resource::<CampaignLevels>();
    } else {
        info!("campaign ready: {} level(s)", playable.len());
        commands.insert_resource(CampaignLevels(playable));
    }
}

#[cfg(test)]
mod tests;
