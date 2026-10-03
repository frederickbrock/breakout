use super::*;
use bevy::asset::AssetLoadError;

/// A bare app with only the level-syncing system: assets are added by hand,
/// nothing is read from files.
fn harness() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<LevelDef>()
        .init_resource::<LevelHandles>()
        .add_systems(Update, sync_campaign_levels);
    app.update();
    app
}

fn named(name: &str) -> LevelDef {
    LevelDef {
        name: name.into(),
        ..LevelDef::fallback()
    }
}

fn current(app: &App) -> Option<&LevelDef> {
    app.world()
        .get_resource::<CampaignLevels>()
        .and_then(|c| c.0.first())
}

/// Adds `def` as an asset and points the campaign's level at it.
fn track(app: &mut App, def: LevelDef) -> Handle<LevelDef> {
    let handle = app.world_mut().resource_mut::<Assets<LevelDef>>().add(def);
    app.world_mut().resource_mut::<LevelHandles>().levels = vec![handle.clone()];
    app.update();
    handle
}

#[test]
fn a_loaded_level_becomes_the_current_level() {
    let mut app = harness();
    assert_eq!(current(&app), None);
    track(&mut app, named("A"));
    assert_eq!(current(&app), Some(&named("A")));
}

#[test]
fn modifying_the_level_asset_updates_the_current_level() {
    let mut app = harness();
    let handle = track(&mut app, named("A"));
    if let Some(mut def) = app
        .world_mut()
        .resource_mut::<Assets<LevelDef>>()
        .get_mut(&handle)
    {
        def.name = "B".into();
    }
    // Bevy writes the `Modified` event at the end of this frame; the sync
    // reads it on the next.
    app.update();
    app.update();
    assert_eq!(current(&app).map(|d| d.name.as_str()), Some("B"));
}

#[test]
fn a_failed_level_load_removes_the_current_level() {
    let mut app = harness();
    let handle = track(&mut app, named("A"));
    app.world_mut()
        .write_message(AssetLoadFailedEvent::<LevelDef> {
            id: handle.id(),
            path: "levels/x.level".into(),
            error: AssetLoadError::AssetMetaReadError,
        });
    app.update();
    assert_eq!(current(&app), None);
}

#[test]
fn clearing_the_campaign_level_removes_the_current_level() {
    let mut app = harness();
    track(&mut app, named("A"));
    app.world_mut().resource_mut::<LevelHandles>().levels = vec![];
    app.update();
    assert_eq!(current(&app), None);
}

/// Every level the campaign holds, by name.
fn names(app: &App) -> Option<Vec<String>> {
    app.world()
        .get_resource::<CampaignLevels>()
        .map(|c| c.0.iter().map(|d| d.name.clone()).collect())
}

fn add(app: &mut App, def: LevelDef) -> Handle<LevelDef> {
    app.world_mut().resource_mut::<Assets<LevelDef>>().add(def)
}

#[test]
fn every_campaign_level_is_kept_in_order() {
    let mut app = harness();
    let a = add(&mut app, named("A"));
    let b = add(&mut app, named("B"));
    app.world_mut().resource_mut::<LevelHandles>().levels = vec![a, b];
    app.update();
    assert_eq!(names(&app), Some(vec!["A".into(), "B".into()]));
}

#[test]
fn a_level_that_failed_to_load_is_skipped() {
    let mut app = harness();
    let a = add(&mut app, named("A"));
    let b = add(&mut app, named("B"));
    app.world_mut().resource_mut::<LevelHandles>().levels = vec![a, b.clone()];
    app.update();
    app.world_mut()
        .write_message(AssetLoadFailedEvent::<LevelDef> {
            id: b.id(),
            path: "levels/b.level".into(),
            error: AssetLoadError::AssetMetaReadError,
        });
    app.update();
    assert_eq!(names(&app), Some(vec!["A".into()]));
}

#[test]
fn a_level_still_loading_leaves_the_campaign_alone() {
    let mut app = harness();
    let a = add(&mut app, named("A"));
    let pending = app.world().resource::<Assets<LevelDef>>().reserve_handle();
    app.world_mut().resource_mut::<LevelHandles>().levels = vec![a, pending];
    app.update();
    assert_eq!(names(&app), None);
}
