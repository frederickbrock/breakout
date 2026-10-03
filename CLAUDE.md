# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

- Web build (primary/default distribution target): `trunk build` (output in `dist/`, which is
  git-ignored; CI builds its own for GitHub Pages)
- Web dev server: `trunk serve` then open the shown localhost URL (default http://localhost:8080)
- Native desktop dev loop (secondary): `cargo run` for a human. **Agents** use
  `scripts/native-run.sh --label <issue-id>` instead: it takes an exclusive lock on the shared
  X display, gives the window a unique title (`BREAKOUT_WINDOW_TITLE`, native only), prints
  the window id to drive by, and tears the process group down on exit. See `scripts/README.md`.
- Native release build: `cargo build --release`
- One-time setup for the web build: `rustup target add wasm32-unknown-unknown` and `cargo install --locked trunk`

Verifying the web build requires manually opening it in a browser (there is no X11-window
watch loop for wasm).

Single binary crate (`sim`), no workspace. Tests: `cargo test` — headless ECS tests that run
the real game logic on `MinimalPlugins` via `test_support::app()` in `src/test_support.rs` (no window or
physics simulation; drive input with `test_support::tap`). Lint: `cargo clippy --all-targets -- -D warnings`.

## Architecture

A Breakout clone built on Bevy 0.19 with Avian2D (`avian2d`) driving physics — ball,
paddle, and bricks are real rigid bodies (`RigidBody::Dynamic`/`Static` + `Collider`),
not hand-rolled kinematics/AABB checks.

### Module index

One line per file; each module's details live in its own `//!` doc comment.

- `src/main.rs` — entry point: engine plugins in `main()`, and `add_game` wiring every module in (wiring only).
- `src/world.rs` — the 1920×1080 world, the centred playfield well, `GAME_SCALE` and the walls.
- `src/frame.rs` — `FramePlugin`: the global steel frame in the two side panels (coded, skinned by `frame_left`/`frame_right`).
- `src/view.rs` — `ViewPlugin`: the camera that fits the world to the window, and `UiScale`.
- `src/web_splash.rs` — `WebSplashPlugin`: tells the web page's loading splash (`index.html`, `web/loader.js`) the menu is drawn.
- `src/game_state.rs` — `AppState` / `PlayState` state machine, `GameOutcome` and the physics clock.
- `src/run.rs` — starting and ending a run, `Score` / `Lives`, `RestartGame` and the HUD.
- `src/paddle.rs` — the paddle, its movement and its prong/field pieces; `PaddleMovementSet`.
- `src/ball.rs` — ball movement rules, the serve from the paddle and `BallApproach`.
- `src/collision.rs` — `on_ball_collision`, scoring, `BrickDamaged` / `BrickDestroyed`.
- `src/controls.rs` — `ControlSettings` (Mouse/Keyboard) and the mouse side of paddle control.
- `src/bricks/mod.rs` — `BrickClass`, `BrickCell`, `generate_board`, the `damage_look` ladder and `BricksPlugin`.
- `src/bricks/grid.rs` — the `Brick` / `BrickHealth` entities, grid layout and `spawn_bricks`.
- `src/bricks/regen.rs` — regen alloy: damaged bricks heal after a timer.
- `src/bricks/explosive.rs` — explosive bricks and their chained blasts.
- `src/bricks/outline.rs` — behaviour outlines drawn over special bricks.
- `src/menu/mod.rs` — `MenuPlugin` and the reusable menu widget kit.
- `src/menu/main_menu.rs` — title screen: Start, Settings, Quit (native only).
- `src/menu/settings.rs` — Settings screen: the paddle-control toggle.
- `src/menu/pause.rs` — pause menu over the frozen game.
- `src/menu/game_over.rs` — game-over / win screen with the final score.
- `src/plate.rs` — `BackingPlate`: the dark rounded plates behind the HUD and the power-up capsules.
- `src/theme.rs` — the Steelbreak palette; every colour the game draws with.
- `src/sprites.rs` — image assets: `GameSprites`, the background and sprite skinning (brick damage plates included).
- `src/particles/mod.rs` — the bevy_enoki VFX layer reacting to brick events.
- `src/spawner.rs` — `Spawner<T>`, a generic weighted registry of spawnable kinds.
- `src/powerups/mod.rs` — the power-up framework: drops, pickup and `ActiveEffects`.
- `src/powerups/super_sizer.rs` — Super-Sizer, and the pattern for adding a power-up.
- `src/powerups/capsules.rs` — time capsules: each active power-up's time left, in the right panel.
- `src/script_manager/mod.rs` — `ScriptPlugin`, Lua scripting (native only).
- `src/test_support.rs` — shared headless test helpers (`app`, `tap`, `click`, `hit`, ...).

### Cross-cutting mechanisms worth knowing before touching gameplay code

- **Avian's collision events are observer-only, not message-queue.** `CollisionStart`/
  `CollisionEnd` derive `Message` but are dispatched purely via `World::trigger` — a
  `MessageReader<CollisionStart>` will silently never receive anything. Consume them with
  an observer (`On<CollisionStart>`), as `on_ball_collision` in `collision.rs` does. Only the
  ball has `CollisionEventsEnabled`; Avian guarantees the enabled side always ends up as
  `collider1`, which is why `on_ball_collision` can assume `on.collider1` is the ball
  without checking.
- **Bricks have a `BrickClass` and `BrickHealth`** (spawned at the class's `max_hits()`).
  `on_ball_collision` scores 10 per hit; on the last hit it triggers
  `BrickDestroyed { brick, position }` *before* despawning, so observers can still read
  the brick; a hit it survives triggers `BrickDamaged { brick }` instead (regen reacts to
  that; blasts fire it too). `BrickDestroyed.by_blast` marks blast kills, which don't set
  off another blast (the chain is already resolved). Other modules (power-ups) hook brick breaks through that event rather than
  editing `on_ball_collision`. The run is won when a brick broke and none are left. Tests
  fake a ball contact with `test_support::hit(app, brick)`, which triggers `CollisionStart`
  exactly as Avian does (`hit_moving` also sets the ball's velocity; `brick_of(app, class)`
  finds a brick of a class).
- **Direction-dependent brick rules read `BallApproach`, not `LinearVelocity`.** Avian
  triggers `CollisionStart` after its solver, so the ball's `LinearVelocity` at the event
  has usually already been reflected. `record_ball_approach` (`ball.rs`) copies it into the ball's
  `BallApproach` in `FixedPostUpdate` `PhysicsSystems::First`, just before each physics step.
  Shield glass takes damage only if `BallApproach.y < 0` (the ball was moving down at
  contact); otherwise it only deflects (no damage, no score; `ShieldDeflected` fires for the glass-glint VFX).
- **Ball speed is deliberately kept at a controlled, constant magnitude**, not left to
  Avian's real momentum transfer — `ball_movement` (`ball.rs`) renormalizes `LinearVelocity` back to
  `BALL_SPEED` after every frame's bounce (with a minimum-vertical-component clamp to
  prevent a real observed failure mode: a ball moving near-perfectly horizontally between
  the side walls, below the bricks and above the paddle, can otherwise get stuck bouncing
  side-to-side forever, since nothing left in that lane can ever touch its Y velocity
  again).
- **The ball is served from the paddle.** At the start of a run and after every lost life
  the ball carries `Anchored` plus Avian's `RigidBodyDisabled` and `ColliderDisabled`
  (always together, via `anchored()`), so it has no velocity and nothing collides with it;
  `follow_paddle` keeps it centred on top of the paddle, and `launch_ball` (Space or left
  click, only while `Playing`) removes all three and sends it off at `BALL_SPEED`, 45°
  toward the side the paddle is moving (right if still). `ball_movement` ignores an anchored
  ball. It sits `BALL_ANCHOR_GAP` above the paddle so the serve doesn't start in contact and
  trigger the paddle-hit spin rule. Tests serve with `tap(&mut app, KeyCode::Space)` (or
  `test_support::click`) before anything that needs the ball in flight.
  On the pause menu, Space activates the focused button (Resume), and that press doesn't
  also serve: menu activation changes state a frame later, after input is cleared.
- **Game state is Bevy `States`, and physics only runs while `InGame/Playing`.**
  `game_state.rs` pauses `Time<Physics>` on leaving `PlayState::Playing` (and on entering
  `MainMenu`/`GameOver`) and resumes it on entering `Playing`, so pausing or ending a run
  freezes every rigid body — nobody zeroes velocities by hand. Non-physics gameplay systems
  (paddle input, `ball_movement`, power-up fall/pickup, effect timers) gate
  themselves with `run_if(in_state(PlayState::Playing))`; a new per-frame gameplay system
  must do the same or it keeps running while paused. The collision observer needs no gate:
  with the clock stopped, no collisions fire.
- **A run's entities are state-scoped.** Ball, paddle, bricks, HUD text and falling
  power-ups carry `DespawnOnExit(AppState::InGame)`, so leaving the run (game over) removes
  them. Walls and camera are global.
  `ball_movement` ends a run via `end_run` (inserts `GameOutcome`, sets `AppState::GameOver`);
  R on the game-over screen goes back to `InGame` (a shortcut for its Play again button);
  Main menu from the pause or game-over screen leaves `InGame`, so the run is torn down and
  the next Start begins fresh.
- **`RestartGame` is a crate-wide broadcast event**, not a resource `run.rs` reaches into.
  `start_run` (in `run.rs`, on `OnEnter(AppState::InGame)`, i.e. first launch and every restart) only
  resets what it directly owns (score, lives = `STARTING_LIVES`, ball/paddle/bricks/HUD) and
  fires `commands.trigger(RestartGame)`; each subsystem with its own state to reset
  (currently just power-ups: `reset_on_restart` clears drops and effects, and
  `attach_reactor_power_ups` equips the run's reactor bricks, which already exist then
  because `start_run` queues their spawns before the trigger) owns its own observer on
  that event. Adding a new stateful subsystem later means giving it its own
  `RestartGame` observer, not editing `start_run`.
- **`PaddleMovementSet` / `TickActiveEffects`** are ordering-only `SystemSet`s that let a
  system in one module (e.g. Super-Sizer's paddle-width recompute) declare `.before(...)`/
  `.after(...)` relative to core systems in another module, without `main.rs` manually
  interleaving individual power-up systems into its own `Update` chain.
- `main.rs::main()` sets `WAYLAND_DISPLAY` to an empty string before building the `App` —
  this is a deliberate WSLg workaround (Wayland + the llvmpipe software renderer hit a
  surface-lost bug on this setup; forcing winit onto X11 fixes it), not dead code to clean
  up. It is now compiled out on wasm via `#[cfg(not(target_arch = "wasm32"))]` — the
  workaround is desktop-only and irrelevant in the browser.
- **wasm enablement is entirely target-gated Cargo.toml / cargo config**, with no gameplay
  logic changes: on `wasm32` avian2d drops its default `parallel` feature (rayon +
  `bevy/multi_threaded` do not build for the browser), bevy gains the `webgl2` renderer
  feature, and getrandom's browser-crypto `wasm_js` backend is turned on for rand via
  `--cfg getrandom_backend="wasm_js"` in `.cargo/config.toml` (scoped to the wasm target
  only, so native builds are untouched).
- **Assets on both targets.** `assets/` is served as-is: natively Bevy reads it from the
  repo root, and `index.html`'s `<link data-trunk rel="copy-dir" href="assets" />` copies it
  into `dist/` for the browser. `main()` sets `AssetPlugin { meta_check:
  AssetMetaCheck::Never }` because no `.meta` files ship (otherwise the web build requests
  one per asset and 404s). Hot reload (bevy's `file_watcher` feature) is enabled only in the
  non-wasm target dependencies, so editing a PNG under `cargo run` updates it live and the
  wasm build doesn't pull the watcher in.
- **Particles: per-class hit/break bursts.** Each brick material has
  `assets/particles/<class>_hit.particle.ron`, `<class>_break.particle.ron` and a
  4-frame 128×32 greyscale sheet `<class>.png`. `<class>` is `particles::material_slug`:
  ceramic, titanium, tungsten, reactor, explosive (all variants), regen or shield.
  Those effects colour themselves via `color_curve`. A missing sheet draws plain quads,
  and a missing effect file falls back to the generic `brick_hit` / `brick_break`.
  Effect files hot-reload under `cargo run`.
- **Lua scripting is native-only.** `bevy_mod_scripting` (`lua54`, which compiles mlua's
  bundled Lua C sources) lives under the non-wasm target dependencies in Cargo.toml, so it
  isn't compiled for wasm at all. `src/script_manager/mod.rs` cfg-gates only the
  `BMSPlugin` registration inside `ScriptPlugin::build`, so `main.rs` adds `ScriptPlugin`
  unconditionally and on wasm it registers no scripting runtime. Any new code touching
  `bevy_mod_scripting` must be gated the same way or the web build breaks.

## Conventions for parallel work

- Tests live in `<module>/tests.rs` via `#[cfg(test)] mod tests;` (e.g. `src/ball.rs` →
  `src/ball/tests.rs`), never inline in a file over ~150 lines.
- Shared headless test helpers go in `src/test_support.rs`.
- A new feature gets its own module with its own `Plugin`, added to `add_game` with one line;
  `main.rs` is wiring only.
- Import items from their owning module (`crate::ball::Ball`), not from the crate root.
- Document a module in its own `//!` comment and add one line to the module index above, not
  a paragraph.

## Beads Workflow Integration

This project uses [beads_rust](https://github.com/Dicklesworthstone/beads_rust) (`br`/`bd`) for issue tracking. Issues are stored in `.beads/` and tracked in git.

### Essential Commands

```bash
# View ready issues (open, unblocked, not deferred)
br ready              # or: bd ready

# List and search
br list --status=open # All open issues
br show <id>          # Full issue details with dependencies
br search "keyword"   # Full-text search

# Create and update
br create --title="..." --description="..." --type=task --priority=2
br update <id> --status=in_progress
br close <id> --reason="Completed"
br close <id1> <id2>  # Close multiple issues at once

# Sync with git
br sync --flush-only  # Export DB to JSONL
br sync --status      # Check sync status
```

### Workflow Pattern

1. **Start**: Run `br ready` to find actionable work
2. **Claim**: Use `br update <id> --status=in_progress`
3. **Work**: Implement the task
4. **Complete**: Use `br close <id>`
5. **Sync**: Always run `br sync --flush-only` at session end

### Key Concepts

- **Dependencies**: Issues can block other issues. `br ready` shows only open, unblocked work.
- **Priority**: P0=critical, P1=high, P2=medium, P3=low, P4=backlog (use numbers 0-4, not words)
- **Types**: task, bug, feature, epic, chore, docs, question
- **Blocking**: `br dep add <issue> <depends-on>` to add dependencies

### Session Protocol

**Before ending any session, run this checklist:**

```bash
git status              # Check what changed
git add <files>         # Stage code changes
br sync --flush-only    # Export beads changes to JSONL
git commit -m "..."     # Commit everything
git push                # Push to remote
```

### Best Practices

- Check `br ready` at session start to find available work
- Update status as you work (in_progress → closed)
- Create new issues with `br create` when you discover tasks
- Use descriptive titles and set appropriate priority/type
- Always sync before ending session

<!-- end-br-agent-instructions -->
