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
the real game logic on `MinimalPlugins` via `test_support::app()` in `main.rs` (no window or
physics simulation; drive input with `test_support::tap`). Lint: `cargo clippy --all-targets -- -D warnings`.

## Architecture

A Breakout clone built on Bevy 0.19 with Avian2D (`avian2d`) driving physics — ball,
paddle, and bricks are real rigid bodies (`RigidBody::Dynamic`/`Static` + `Collider`),
not hand-rolled kinematics/AABB checks.

### Module layout

- `src/main.rs` — core game: `Ball`, `Paddle`, `Brick` entities/components, score/lives
  resources, the `App` wiring (`add_game`), starting a run (`start_run`), and the systems
  that react to physics (`ball_movement`, `paddle_movement`, `update_hud`).
- `src/game_state.rs` — the state machine: `AppState { MainMenu, Settings, InGame, GameOver }`
  (the app launches into `MainMenu`), the
  `InGame` sub-state `PlayState { Playing, Paused }` (P/Esc toggles it), the `GameOutcome`
  (won/lost) resource, and control of Avian's physics clock.
- `src/menu/` — menu screens and the reusable widget kit they share. `mod.rs` holds the kit:
  `menu_screen(state)` (state-scoped full-window root), `menu_list()` (a `MenuList` column
  whose children order is the keyboard navigation order), `menu_button(label)`, the
  `Focused` marker, and the `ButtonActivated` entity event fired on click or Enter/Space.
  Mouse hover/press looks, Up/Down/W/S focus (wrapping) and activation come for free; each
  button's behaviour is its own `.observe(...)` (e.g. `go_to(AppState::InGame)`), so a new
  screen (pause, game over) is a new file with its own plugin, not an edit to a shared
  match. The screens: `main_menu.rs` (Start/Settings/Quit — Quit is native-only),
  `settings.rs` (the "Paddle control: Mouse/Keyboard" toggle; Back or Esc returns), `pause.rs` (Resume / Main menu, shown
  over the frozen game while `PlayState::Paused`) and `game_over.rs` ("GAME OVER" or
  "YOU WIN!", final score, Play again / Main menu). Pause and game-over roots use
  `OVERLAY_DIM` as background so the game shows through.
- `src/theme.rs` — the Steelbreak palette: every colour the game draws with (void clear
  colour, steel ball, cyan emitter paddle and its prongs, ceramic/titanium/tungsten brick
  rows, reactor-violet power-up bricks and `cracked()`, power-up drops, HUD ink/label,
  menu buttons and the overlay dim). Use a `theme::` constant instead of a colour literal;
  the later sprite swap and palette tweaks touch only this file. These colours are also
  the fallback look when a sprite file is missing (see `src/sprites.rs`): the ball is a
  round `Mesh2d(Circle)` (handles in the `BallLook` resource, made in `setup_level`), and
  the paddle is drawn by three children (left prong, stretched `PaddleField`, right prong)
  laid out by the pure `paddle_pieces(width)` and kept in place by `place_paddle_pieces`
  as Super-Sizer changes `Paddle.width` (the parent keeps the one full-width collider and
  has no sprite of its own). The HUD is `SCORE `/`LIVES ` labels with the value in a `TextSpan`
  child (the markers sit on the span). The headless test app adds `AssetPlugin` plus
  `Mesh`/`ColorMaterial` assets for the ball.
- `src/sprites.rs` — image assets. `SpritesPlugin` (registered from `main()`, not
  `add_game`, since the headless test app has no image loaders) loads every handle once at
  `Startup` into the `GameSprites` resource (background, ball, paddle prongs and field,
  power-up icon) and spawns the global `Background`. `SkinPlugin` (in `add_game`, a no-op
  without `GameSprites`) swaps an entity's `theme` shape for its sprite once that image is
  in `Assets<Image>`, marking it `Skinned`. Everything spawns as its shape first, so a
  missing or broken file just leaves the shape (no panic, nothing invisible). Sprites live
  at `assets/sprites/<name>.png`; a new one is a `GameSprites` field plus a skin rule.
- `src/controls.rs` — player controls: the session-only `ControlSettings` resource
  (`PaddleControl::Mouse` by default, or `Keyboard`, toggled on the Settings screen) and
  the mouse side of paddle control. While `Playing` in Mouse mode, `track_cursor` turns
  cursor movement into a `PaddleTarget` (world X); `paddle_movement` then drives the
  paddle's `LinearVelocity.x` toward it (`clamp_paddle_x` keeps it between the walls for
  the current `Paddle.width`, `follow_velocity` is the capped proportional drive, limited to ~80% of the gap per frame so
  low frame rates don't overshoot), so Avian
  still resolves ball bounces. Arrow keys / A/D push with `ConstantForce` in both modes, and
  a held key clears the mouse target. Tests set `PaddleTarget` directly (no window).
- `src/bricks/` (`mod.rs`) — the brick model: `BrickClass` (`Ceramic`, `Titanium`, `Tungsten`,
  `Reactor` = the power-up brick, `Explosive(ExplosiveKind::{Charge, Breach, Demolition})`,
  `Regen`, `Shield`) with `max_hits()` (1/2/3/2/1/2/1), a `BrickCell { row, col }` on
  every brick (row 0 at the top), and the pure `generate_board(rng) -> [[BrickClass; 10]; 7]`:
  a weighted fill (ceramic 30, titanium 20, tungsten 12, explosive 16 split over the three
  variants, regen 12, shield 10), then one bounded patch pass that places exactly
  `REACTOR_BRICKS` (6) reactors and guarantees at least one of every other class and
  variant. Tested with a seeded rng. Per-class behaviours are submodules composed into
  `BricksPlugin`: `regen.rs` (a regen brick that survives a hit gets a 3 s `RegenTimer`,
  restarted by each further non-lethal hit; when it runs out the brick heals to full and
  loses its cracked look; ticks only while `Playing`) and `explosive.rs` (a ball-destroyed
  explosive sets off a blast: charge = 1 hit to the 8 around, breach = destroys the 4
  orthogonal, demolition = destroys the 8 around; charge/breach chain into explosives they
  destroy, demolition doesn't; blasts ignore shield glass's direction rule). The chain is
  resolved by the pure `resolve_blast(grid, origin, kind)` on a `BrickCell`-keyed snapshot,
  so each hit point is removed (and scored) once, then applied through the normal break path
  (`BrickDestroyed { by_blast: true }`, score, `broke_brick`, `BrickDamaged` for survivors)
  with a placeholder `BlastFlash` per explosion. `BricksPlugin` also runs the
  shield-glass flash timer
  (`ShieldFlash`, frozen while paused). Colours come from `theme::brick_color(class)` and
  `theme::brick_face(class, health)` (cracked below full health).
- `src/spawner.rs` — `Spawner<T>`, a generic weighted registry of spawnable kinds
  (`register(kind, weight, color)` + `pick()`, no timer). Reusable across any future domain (obstacles, brick respawns, etc.) because
  Bevy resources are keyed by concrete type: `Spawner<PowerUpKind>` and a hypothetical
  `Spawner<ObstacleKind>` are automatically independent resources (same trick Bevy itself
  uses for `Time<T>`/`Events<T>`). It only decides *which kind*; when to pick and
  actually spawning an entity (components, physics) stay domain-specific.
- `src/powerups/mod.rs` — the power-up framework: `PowerUp` component, `PowerUpCollected`
  event, `ActiveEffects` resource (the "game state" active effects land in — consumers
  recompute derived values from it every frame, so an effect expiring needs no explicit
  revert step), `PowerUpBrick` (each run `attach_reactor_power_ups`, a `RestartGame`
  observer, gives every reactor-class brick a kind picked from `PowerUpSpawner`),
  and the drop/fall/paddle-pickup systems. A power-up only ever appears when a power-up
  brick breaks: `drop_power_up` observes `BrickDestroyed` and spawns it at the brick's
  position (no timed drops).
- `src/script_manager/mod.rs` — `ScriptPlugin`, the Lua scripting entry point (wraps
  bevy_mod_scripting's `BMSPlugin`). Native-only in effect; see the wasm note below.
- `src/powerups/super_sizer.rs` — one concrete power-up (Super-Sizer: temporarily widens
  the paddle), fully self-contained as its own `Plugin`. **This is the pattern for adding a
  new power-up**: a new file with its own `Plugin` that (1) registers itself into the
  shared `PowerUpSpawner` registry (`Spawner<PowerUpKind>`) at `build()` time via
  `app.world_mut().resource_mut::<PowerUpSpawner>().register(...)`, (2) reacts to
  `PowerUpCollected` via its own `add_observer`, and (3) is composed in via
  `PowerUpsPlugin`'s `.add_plugins(...)`. No shared match statement to edit — only
  `PowerUpKind` needs a new enum variant centrally.

### Cross-cutting mechanisms worth knowing before touching gameplay code

- **Avian's collision events are observer-only, not message-queue.** `CollisionStart`/
  `CollisionEnd` derive `Message` but are dispatched purely via `World::trigger` — a
  `MessageReader<CollisionStart>` will silently never receive anything. Consume them with
  an observer (`On<CollisionStart>`), as `on_ball_collision` in `main.rs` does. Only the
  ball has `CollisionEventsEnabled`; Avian guarantees the enabled side always ends up as
  `collider1`, which is why `on_ball_collision` can assume `on.collider1` is the ball
  without checking.
- **Bricks have a `BrickClass` and `BrickHealth`** (spawned at the class's `max_hits()`).
  `on_ball_collision` scores 10 per hit and shows a surviving brick's cracked face; on the last hit it triggers
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
  has usually already been reflected. `record_ball_approach` copies it into the ball's
  `BallApproach` in `FixedPostUpdate` `PhysicsSystems::First`, just before each physics step.
  Shield glass takes damage only if `BallApproach.y < 0` (the ball was moving down at
  contact); otherwise it only flashes (no damage, no score).
- **Ball speed is deliberately kept at a controlled, constant magnitude**, not left to
  Avian's real momentum transfer — `ball_movement` renormalizes `LinearVelocity` back to
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
- **`RestartGame` is a crate-wide broadcast event**, not a resource `main.rs` reaches into.
  `start_run` (on `OnEnter(AppState::InGame)`, i.e. first launch and every restart) only
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
- **Lua scripting is native-only.** `bevy_mod_scripting` (`lua54`, which compiles mlua's
  bundled Lua C sources) lives under the non-wasm target dependencies in Cargo.toml, so it
  isn't compiled for wasm at all. `src/script_manager/mod.rs` cfg-gates only the
  `BMSPlugin` registration inside `ScriptPlugin::build`, so `main.rs` adds `ScriptPlugin`
  unconditionally and on wasm it registers no scripting runtime. Any new code touching
  `bevy_mod_scripting` must be gated the same way or the web build breaks.

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
