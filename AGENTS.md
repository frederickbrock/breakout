<!-- br-agent-instructions-v1 -->
# Shared agent rules (breakout)

Rust + Bevy 0.19 + Avian2D, one crate (`sim`). Web build (trunk) is primary, native is secondary.

- Build: `cargo build`; web: `trunk build`
- Test: `cargo test` (headless ECS tests)
- Lint: `cargo clippy --all-targets -- -D warnings`
- Run natively as an agent: `scripts/native-run.sh --label <issue-id>`, never a bare `cargo run`

## Beads (`br`)

Issues live in `.beads/`. The team protocol is the `beads-queue` skill (stages,
claim, hand-off, bounce, escalate). Follow it, not generic `br` docs.

- Workers never `git add`, commit or push `.beads/`. Only the pm syncs `.beads/issues.jsonl`.
- Never `br close` your own issue. The pr-manager closes it after the merge.
- Pass `--actor "<role>@<worktree>"` on every `br` write.
<!-- end-br-agent-instructions -->
