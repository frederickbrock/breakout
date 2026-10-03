//! Regen alloy: a damaged regen brick heals back to full unless it's
//! finished within [`HEAL_SECS`].
//!
//! Reacts to [`BrickDamaged`] (fired for every non-lethal hit, ball or
//! explosion), so `on_ball_collision` needs no regen special case. Each
//! further non-lethal hit restarts the timer. The timer only ticks while
//! playing, so pausing freezes it.
//!
//! A regen brick that survives a hit gets a [`RegenTimer`]; when it runs
//! out the brick heals to full health (and its damage particles stop).

use super::grid::BrickHealth;
use super::BrickClass;
use crate::collision::BrickDamaged;
use crate::game_state::PlayState;
use bevy::prelude::*;

/// How long a damaged regen brick waits before healing to full.
const HEAL_SECS: f32 = 6.0;

/// A damaged regen brick's countdown to healing.
#[derive(Component)]
pub struct RegenTimer(Timer);

impl RegenTimer {
    /// How far the countdown has run: 0 just after the hit, 1 at healing.
    pub fn fraction_elapsed(&self) -> f32 {
        self.0.fraction()
    }
}

impl Default for RegenTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(HEAL_SECS, TimerMode::Once))
    }
}

pub struct RegenPlugin;

impl Plugin for RegenPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(start_heal_timer).add_systems(
            Update,
            heal_regen_bricks.run_if(in_state(PlayState::Playing)),
        );
    }
}

/// A regen brick that survives a hit starts (or restarts) its heal timer.
fn start_heal_timer(on: On<BrickDamaged>, mut commands: Commands, classes: Query<&BrickClass>) {
    if classes.get(on.brick) == Ok(&BrickClass::Regen) {
        commands.entity(on.brick).insert(RegenTimer::default());
    }
}

/// When the timer runs out, the brick is back to full hits (its damage smoke
/// stops: `src/particles/` watches its health).
fn heal_regen_bricks(
    mut commands: Commands,
    time: Res<Time>,
    mut bricks: Query<(Entity, &mut RegenTimer, &BrickClass, &mut BrickHealth)>,
) {
    for (entity, mut timer, &class, mut health) in &mut bricks {
        if timer.0.tick(time.delta()).is_finished() {
            health.0 = class.max_hits();
            commands.entity(entity).remove::<RegenTimer>();
        }
    }
}

#[cfg(test)]
mod tests;
