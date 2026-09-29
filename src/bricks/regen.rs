//! Regen alloy: a damaged regen brick heals back to full unless it's
//! finished within [`HEAL_SECS`].
//!
//! Reacts to [`BrickDamaged`] (fired for every non-lethal hit, ball or
//! explosion), so `on_ball_collision` needs no regen special case. Each
//! further non-lethal hit restarts the timer. The timer only ticks while
//! playing, so pausing freezes it.

use super::BrickClass;
use crate::game_state::PlayState;
use crate::{theme, BrickDamaged, BrickHealth};
use bevy::prelude::*;

/// How long a damaged regen brick waits before healing to full.
const HEAL_SECS: f32 = 3.0;

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

/// When the timer runs out, the brick is back to full hits and loses its
/// cracked look.
fn heal_regen_bricks(
    mut commands: Commands,
    time: Res<Time>,
    mut bricks: Query<(
        Entity,
        &mut RegenTimer,
        &BrickClass,
        &mut BrickHealth,
        &mut Sprite,
    )>,
) {
    for (entity, mut timer, &class, mut health, mut sprite) in &mut bricks {
        if timer.0.tick(time.delta()).is_finished() {
            health.0 = class.max_hits();
            sprite.color = theme::brick_face(class, health.0);
            commands.entity(entity).remove::<RegenTimer>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    /// Runs `secs` of play at the test app's 100 ms step.
    fn wait(app: &mut App, secs: f32) {
        for _ in 0..(secs * 10.0).round() as usize {
            app.update();
        }
    }

    fn health(app: &App, brick: Entity) -> u8 {
        app.world().get::<BrickHealth>(brick).unwrap().0
    }

    fn color(app: &App, brick: Entity) -> Color {
        app.world().get::<Sprite>(brick).unwrap().color
    }

    #[test]
    fn a_second_hit_within_three_seconds_breaks_it() {
        let mut app = app();
        let regen = brick_of(&mut app, BrickClass::Regen);
        hit(&mut app, regen);
        assert_eq!(health(&app, regen), 1);
        assert_eq!(color(&app, regen), theme::cracked(theme::REGEN));
        wait(&mut app, 2.0);
        hit(&mut app, regen);
        assert!(app.world().get_entity(regen).is_err());
    }

    #[test]
    fn left_alone_for_three_seconds_it_heals_to_full() {
        let mut app = app();
        let regen = brick_of(&mut app, BrickClass::Regen);
        hit(&mut app, regen);
        wait(&mut app, 2.5);
        assert_eq!(health(&app, regen), 1, "not yet");
        wait(&mut app, 1.0);
        assert_eq!(health(&app, regen), 2);
        assert_eq!(color(&app, regen), theme::REGEN, "no longer cracked");
        assert!(!app.world().entity(regen).contains::<RegenTimer>());

        // It needs two hits again.
        hit(&mut app, regen);
        assert!(app.world().get_entity(regen).is_ok());
        hit(&mut app, regen);
        assert!(app.world().get_entity(regen).is_err());
    }

    #[test]
    fn pausing_freezes_the_heal_timer() {
        let mut app = app();
        let regen = brick_of(&mut app, BrickClass::Regen);
        hit(&mut app, regen);
        wait(&mut app, 1.0);
        tap(&mut app, KeyCode::KeyP);
        wait(&mut app, 10.0);
        assert_eq!(health(&app, regen), 1, "paused for 10 s: still cracked");
        tap(&mut app, KeyCode::KeyP);
        // About 1.2 s had run before the pause, so ~1.8 s remain.
        wait(&mut app, 1.0);
        assert_eq!(health(&app, regen), 1);
        wait(&mut app, 1.5);
        assert_eq!(health(&app, regen), 2);
    }

    #[test]
    fn further_damage_restarts_the_timer() {
        let mut app = app();
        let regen = brick_of(&mut app, BrickClass::Regen);
        hit(&mut app, regen);
        wait(&mut app, 2.0);
        // Another non-lethal hit (e.g. explosion damage, sim-rdl.7.3).
        app.world_mut().trigger(BrickDamaged { brick: regen });
        app.world_mut().flush();
        wait(&mut app, 2.0);
        assert_eq!(health(&app, regen), 1, "timer restarted: 2 s < 3 s");
        wait(&mut app, 1.5);
        assert_eq!(health(&app, regen), 2);
    }

    #[test]
    fn other_classes_never_heal() {
        let mut app = app();
        let tungsten = brick_of(&mut app, BrickClass::Tungsten);
        hit(&mut app, tungsten);
        wait(&mut app, 5.0);
        assert_eq!(health(&app, tungsten), 2);
        assert!(!app.world().entity(tungsten).contains::<RegenTimer>());
    }
}
