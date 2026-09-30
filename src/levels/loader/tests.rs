use super::*;
use bevy::asset::AssetLoadError;

/// A bare app with only the level-syncing system: assets are added by hand,
/// nothing is read from files.
fn harness() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<LevelDef>()
        .init_resource::<LevelHandles>()
        .add_systems(Update, sync_current_level);
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
    app.world().get_resource::<CurrentLevel>().map(|c| &c.0)
}

/// Adds `def` as an asset and points the campaign's level at it.
fn track(app: &mut App, def: LevelDef) -> Handle<LevelDef> {
    let handle = app.world_mut().resource_mut::<Assets<LevelDef>>().add(def);
    app.world_mut().resource_mut::<LevelHandles>().level = Some(handle.clone());
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
    app.world_mut().resource_mut::<LevelHandles>().level = None;
    app.update();
    assert_eq!(current(&app), None);
}
