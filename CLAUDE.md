# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

- Web build (primary/default distribution target): `trunk build` (output in `dist/`)
- Web dev server: `trunk serve` then open the shown localhost URL (default http://localhost:8080)
- Native desktop dev loop (secondary): `cargo run`
- Native release build: `cargo build --release`
- One-time setup for the web build: `rustup target add wasm32-unknown-unknown` and `cargo install --locked trunk`

Verifying the web build requires manually opening it in a browser (there is no X11-window
watch loop for wasm).

Single binary crate (`sim`), no workspace, no test suite or lint config in the repo currently.

## Architecture

A Breakout clone built on Bevy 0.19 with Avian2D (`avian2d`) driving physics — ball,
paddle, and bricks are real rigid bodies (`RigidBody::Dynamic`/`Static` + `Collider`),
not hand-rolled kinematics/AABB checks.

### Module layout

- `src/main.rs` — core game: `Ball`, `Paddle`, `Brick` entities/components, score/lives/
  game-status resources, the `App` wiring, and the systems that react to physics
  (`ball_movement`, `paddle_movement`, `restart_game`, `update_ui`).
- `src/spawner.rs` — `Spawner<T>`, a generic "every N seconds, produce one weighted-random
  thing" engine. Reusable across any future domain (obstacles, brick respawns, etc.) because
  Bevy resources are keyed by concrete type: `Spawner<PowerUpKind>` and a hypothetical
  `Spawner<ObstacleKind>` are automatically independent resources (same trick Bevy itself
  uses for `Time<T>`/`Events<T>`). It only decides *when* and *which kind*; actually
  spawning an entity (components, physics) stays domain-specific.
- `src/powerups/mod.rs` — the power-up framework: `PowerUp` component, `PowerUpCollected`
  event, `ActiveEffects` resource (the "game state" active effects land in — consumers
  recompute derived values from it every frame, so an effect expiring needs no explicit
  revert step), and the generic spawn/physics/paddle-collision systems.
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
- **Ball speed is deliberately kept at a controlled, constant magnitude**, not left to
  Avian's real momentum transfer — `ball_movement` renormalizes `LinearVelocity` back to
  `BALL_SPEED` after every frame's bounce (with a minimum-vertical-component clamp to
  prevent a real observed failure mode: a ball moving near-perfectly horizontally between
  the side walls, below the bricks and above the paddle, can otherwise get stuck bouncing
  side-to-side forever, since nothing left in that lane can ever touch its Y velocity
  again).
- **`GameStatus` does not pause Avian's physics step** — the engine keeps simulating
  regardless of our game state, so code that transitions to `Lost`/`Won` must explicitly
  zero `LinearVelocity` itself (see the two spots in `ball_movement`) or entities keep
  drifting/falling off-screen after the game "ends."
- **`RestartGame` is a crate-wide broadcast event**, not a resource `main.rs` reaches into.
  `restart_game` only resets what it directly owns (score/lives/ball/paddle/bricks) and
  fires `commands.trigger(RestartGame)`; each subsystem with its own state to reset
  (currently just power-ups, via `reset_on_restart` in `powerups/mod.rs`) owns its own
  observer on that event. Adding a new stateful subsystem later means giving it its own
  `RestartGame` observer, not editing `restart_game`.
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
