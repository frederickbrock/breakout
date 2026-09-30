//! [`ScriptPlugin`], the Lua scripting entry point (wraps bevy_mod_scripting's
//! `BMSPlugin`).
//!
//! Native-only in effect: `bevy_mod_scripting` lives under the non-wasm
//! target dependencies in Cargo.toml, so it isn't compiled for wasm at all,
//! and only the `BMSPlugin` registration is cfg-gated here. `main.rs` adds
//! [`ScriptPlugin`] unconditionally. Any new code touching
//! `bevy_mod_scripting` must be gated the same way or the web build breaks.

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
