use bevy::prelude::*;
use bevy_mod_scripting::prelude::*;

#[derive(Resource)]
pub struct ScriptRepository;

#[derive(Debug)]
pub struct ScriptPlugin;

impl Plugin for ScriptPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ScriptRepository).add_plugins(BMSPlugin);
    }
}
