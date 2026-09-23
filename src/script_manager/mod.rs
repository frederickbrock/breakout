use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy_mod_scripting::prelude::*;

#[derive(Resource)]
pub struct ScriptRepository;

/// Lua scripting via bevy_mod_scripting. Native-only: on wasm the dependency is
/// not compiled at all (see Cargo.toml), so this plugin adds no scripting runtime
/// there, letting `main.rs` add it unconditionally.
#[derive(Debug)]
pub struct ScriptPlugin;

impl Plugin for ScriptPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ScriptRepository);

        #[cfg(not(target_arch = "wasm32"))]
        app.add_plugins(BMSPlugin);
    }
}
