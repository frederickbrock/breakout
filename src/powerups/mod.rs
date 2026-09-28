mod super_sizer;

use crate::bricks::BrickClass;
use crate::game_state::{AppState, PlayState};
use crate::spawner::Spawner;
use crate::{Brick, BrickDestroyed, Paddle, RestartGame, PADDLE_HEIGHT, WINDOW_HEIGHT};
use bevy::prelude::*;

const POWER_UP_SIZE: f32 = 24.0;
const BASE_GRAVITY: f32 = 140.0;
const GRAVITY_STEP: f32 = 20.0;
const MAX_GRAVITY: f32 = 420.0;

pub type PowerUpSpawner = Spawner<PowerUpKind>;

/// Every power-up type. Adding a new power-up: add a variant here, and a new
/// file/module (see [`super_sizer`]) with its own `Plugin` that registers
/// itself with [`PowerUpSpawner`] and reacts to [`PowerUpCollected`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PowerUpKind {
    SuperSizer,
}

#[derive(Component)]
pub struct PowerUp {
    kind: PowerUpKind,
    velocity: Vec2,
    gravity: f32,
}

/// A brick carrying a power-up: attached to each reactor-class brick at the
/// start of a run by [`attach_reactor_power_ups`], dropped by
/// [`drop_power_up`] when it breaks.
#[derive(Component)]
pub struct PowerUpBrick {
    kind: PowerUpKind,
    color: Color,
}

/// Power-ups dropped so far this run; each successive drop falls a little
/// faster.
#[derive(Resource, Default)]
struct PowerUpDrops(u32);

/// Fires when a falling [`PowerUp`] is caught by the paddle. Each power-up
/// kind reacts to this via its own observer (see [`super_sizer`]).
#[derive(Event)]
pub struct PowerUpCollected {
    pub kind: PowerUpKind,
}

struct ActiveEffect {
    kind: PowerUpKind,
    timer: Timer,
}

/// The "game state" power-up effects land in. Systems that care about a given
/// effect (e.g. paddle width) recompute their derived value from this list
/// every frame, so an effect expiring is just "timer runs out, no longer
/// counted" — there's no separate revert step to maintain.
#[derive(Resource, Default)]
pub struct ActiveEffects(Vec<ActiveEffect>);

impl ActiveEffects {
    fn clear(&mut self) {
        self.0.clear();
    }

    fn is_active(&self, kind: PowerUpKind) -> bool {
        self.0.iter().any(|effect| effect.kind == kind)
    }

    fn refresh_or_insert(&mut self, kind: PowerUpKind, duration: f32) {
        if let Some(effect) = self.0.iter_mut().find(|effect| effect.kind == kind) {
            effect.timer = Timer::from_seconds(duration, TimerMode::Once);
        } else {
            self.0.push(ActiveEffect {
                kind,
                timer: Timer::from_seconds(duration, TimerMode::Once),
            });
        }
    }
}

/// Systems that tick down every active power-up effect's timer are ordered
/// relative to this set, so any power-up's own "recompute derived state from
/// `ActiveEffects`" system can declare `.after(TickActiveEffects)` without
/// needing to know `tick_active_effects`'s function name.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TickActiveEffects;

pub struct PowerUpsPlugin;

impl Plugin for PowerUpsPlugin {
    fn build(&self, app: &mut App) {
        // The registry must exist before `SuperSizerPlugin` registers into it.
        app.init_resource::<PowerUpSpawner>()
            .init_resource::<PowerUpDrops>()
            .init_resource::<ActiveEffects>()
            .add_observer(reset_on_restart)
            .add_observer(attach_reactor_power_ups)
            .add_observer(drop_power_up)
            // Everything that moves power-ups or counts down their timers
            // runs only while playing, so pausing or ending the run freezes
            // falling power-ups and active effects alike.
            .add_systems(
                Update,
                (power_up_physics, power_up_paddle_collision)
                    .chain()
                    .run_if(in_state(PlayState::Playing)),
            )
            .add_systems(
                Update,
                tick_active_effects
                    .in_set(TickActiveEffects)
                    .before(crate::PaddleMovementSet)
                    .run_if(in_state(PlayState::Playing)),
            )
            .add_plugins(super_sizer::SuperSizerPlugin);
    }
}

/// At the start of every run, gives each reactor-class brick (the board
/// generator places exactly `bricks::REACTOR_BRICKS`) a weighted-random
/// power-up from [`PowerUpSpawner`]. Runs on [`RestartGame`], which
/// `start_run` triggers after queuing the brick spawns, so the new run's
/// bricks already exist. Health and colour come from the class.
fn attach_reactor_power_ups(
    _restart: On<RestartGame>,
    mut commands: Commands,
    spawner: Res<PowerUpSpawner>,
    bricks: Query<(Entity, &BrickClass), With<Brick>>,
) {
    for (entity, class) in &bricks {
        if *class != BrickClass::Reactor {
            continue;
        }
        let Some(pick) = spawner.pick() else {
            return;
        };
        commands.entity(entity).insert(PowerUpBrick {
            kind: pick.kind,
            color: pick.color,
        });
    }
}

/// A broken power-up brick drops its power-up where it stood. Each
/// successive drop in a run is a little heavier.
fn drop_power_up(
    on: On<BrickDestroyed>,
    mut commands: Commands,
    mut drops: ResMut<PowerUpDrops>,
    bricks: Query<&PowerUpBrick>,
) {
    let Ok(power_up_brick) = bricks.get(on.brick) else {
        return;
    };
    let gravity = (BASE_GRAVITY + drops.0 as f32 * GRAVITY_STEP).min(MAX_GRAVITY);
    drops.0 += 1;
    commands.spawn((
        Sprite::from_color(power_up_brick.color, Vec2::splat(POWER_UP_SIZE)),
        Transform::from_xyz(on.position.x, on.position.y, 0.5),
        PowerUp {
            kind: power_up_brick.kind,
            velocity: Vec2::ZERO,
            gravity,
        },
        DespawnOnExit(AppState::InGame),
    ));
}

fn power_up_physics(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut PowerUp)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut power_up) in &mut query {
        power_up.velocity.y -= power_up.gravity * dt;
        transform.translation.y += power_up.velocity.y * dt;

        if transform.translation.y < -WINDOW_HEIGHT / 2.0 - POWER_UP_SIZE {
            commands.entity(entity).despawn();
        }
    }
}

fn power_up_paddle_collision(
    mut commands: Commands,
    paddle_query: Query<(&Transform, &Paddle)>,
    power_up_query: Query<(Entity, &Transform, &PowerUp)>,
) {
    let Ok((paddle_transform, paddle)) = paddle_query.single() else {
        return;
    };
    let paddle_left = paddle_transform.translation.x - paddle.width / 2.0;
    let paddle_right = paddle_transform.translation.x + paddle.width / 2.0;
    let paddle_top = paddle_transform.translation.y + PADDLE_HEIGHT / 2.0;
    let paddle_bottom = paddle_transform.translation.y - PADDLE_HEIGHT / 2.0;
    let half = POWER_UP_SIZE / 2.0;

    for (entity, transform, power_up) in &power_up_query {
        let overlaps_x = transform.translation.x + half >= paddle_left
            && transform.translation.x - half <= paddle_right;
        let overlaps_y = transform.translation.y - half <= paddle_top
            && transform.translation.y + half >= paddle_bottom;

        if overlaps_x && overlaps_y {
            commands.entity(entity).despawn();
            commands.trigger(PowerUpCollected {
                kind: power_up.kind,
            });
        }
    }
}

fn tick_active_effects(time: Res<Time>, mut active: ResMut<ActiveEffects>) {
    let dt = time.delta();
    active
        .0
        .retain_mut(|effect| !effect.timer.tick(dt).is_finished());
}

fn reset_on_restart(
    _restart: On<RestartGame>,
    mut commands: Commands,
    mut drops: ResMut<PowerUpDrops>,
    mut active: ResMut<ActiveEffects>,
    power_up_query: Query<Entity, With<PowerUp>>,
) {
    drops.0 = 0;
    active.clear();
    for entity in &power_up_query {
        commands.entity(entity).despawn();
    }
}

/// Spawns a falling power-up the way [`drop_power_up`] does (flat-colour
/// sprite included), for tests in other modules.
#[cfg(test)]
pub(crate) fn test_spawn_power_up(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            Sprite::from_color(crate::theme::POWER_UP, Vec2::splat(POWER_UP_SIZE)),
            Transform::from_xyz(0.0, 200.0, 0.5),
            PowerUp {
                kind: PowerUpKind::SuperSizer,
                velocity: Vec2::ZERO,
                gravity: BASE_GRAVITY,
            },
            DespawnOnExit(AppState::InGame),
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bricks::REACTOR_BRICKS;
    use crate::test_support::*;
    use crate::{theme, BrickHealth};

    fn spawn_falling_power_up(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                Transform::from_xyz(0.0, 200.0, 0.5),
                PowerUp {
                    kind: PowerUpKind::SuperSizer,
                    velocity: Vec2::ZERO,
                    gravity: BASE_GRAVITY,
                },
                DespawnOnExit(AppState::InGame),
            ))
            .id()
    }

    fn effect_remaining(app: &App) -> Option<std::time::Duration> {
        app.world()
            .resource::<ActiveEffects>()
            .0
            .first()
            .map(|effect| effect.timer.remaining())
    }

    #[test]
    fn pausing_freezes_falling_power_ups_and_effect_timers() {
        let mut app = app();
        let power_up = spawn_falling_power_up(&mut app);
        app.world_mut()
            .resource_mut::<ActiveEffects>()
            .refresh_or_insert(PowerUpKind::SuperSizer, 5.0);

        tap(&mut app, KeyCode::KeyP);
        let y_paused = app
            .world()
            .get::<Transform>(power_up)
            .unwrap()
            .translation
            .y;
        let remaining_paused = effect_remaining(&app);
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(
            app.world()
                .get::<Transform>(power_up)
                .unwrap()
                .translation
                .y,
            y_paused
        );
        assert_eq!(effect_remaining(&app), remaining_paused);

        tap(&mut app, KeyCode::KeyP);
        app.update();
        assert!(
            app.world()
                .get::<Transform>(power_up)
                .unwrap()
                .translation
                .y
                < y_paused
        );
        assert!(effect_remaining(&app) < remaining_paused);
    }

    #[test]
    fn a_new_run_clears_power_ups_and_active_effects() {
        let mut app = app();
        tap(&mut app, KeyCode::Space);
        spawn_falling_power_up(&mut app);
        app.world_mut()
            .resource_mut::<ActiveEffects>()
            .refresh_or_insert(PowerUpKind::SuperSizer, 5.0);
        app.world_mut().resource_mut::<crate::Lives>().0 = 1;
        let mut ball = app
            .world_mut()
            .query_filtered::<&mut Transform, With<crate::Ball>>()
            .single_mut(app.world_mut())
            .unwrap();
        ball.translation.y = -WINDOW_HEIGHT;
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::GameOver);
        assert_eq!(count::<With<PowerUp>>(&mut app), 0);

        tap(&mut app, KeyCode::KeyR);
        assert_eq!(app_state(&app), AppState::InGame);
        assert!(app.world().resource::<ActiveEffects>().0.is_empty());
        assert_eq!(count::<With<PowerUp>>(&mut app), 0);
    }

    fn power_up_bricks(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<PowerUpBrick>>()
            .iter(app.world())
            .collect()
    }

    fn power_up_brick_positions(app: &mut App) -> Vec<(i32, i32)> {
        let mut positions: Vec<(i32, i32)> = app
            .world_mut()
            .query_filtered::<&Transform, With<PowerUpBrick>>()
            .iter(app.world())
            .map(|t| (t.translation.x as i32, t.translation.y as i32))
            .collect();
        positions.sort();
        positions
    }

    fn position(app: &App, entity: Entity) -> Vec2 {
        app.world()
            .get::<Transform>(entity)
            .unwrap()
            .translation
            .truncate()
    }

    /// Every falling power-up: (entity, position, kind, gravity).
    fn falling(app: &mut App) -> Vec<(Entity, Vec2, PowerUpKind, f32)> {
        app.world_mut()
            .query::<(Entity, &Transform, &PowerUp)>()
            .iter(app.world())
            .map(|(e, t, p)| (e, t.translation.truncate(), p.kind, p.gravity))
            .collect()
    }

    fn break_brick(app: &mut App, brick: Entity) {
        hit(app, brick);
        hit(app, brick);
        assert!(app.world().get_entity(brick).is_err());
    }

    #[test]
    fn every_reactor_brick_and_only_those_carry_a_power_up() {
        let mut app = app();
        assert_eq!(power_up_bricks(&mut app).len(), REACTOR_BRICKS);
        assert_eq!(
            bricks(&mut app).len(),
            crate::BRICK_ROWS * crate::BRICK_COLS
        );

        for brick in bricks(&mut app) {
            let entity = app.world().entity(brick);
            let health = entity.get::<BrickHealth>().unwrap().0;
            let class = *entity.get::<BrickClass>().unwrap();
            match entity.get::<PowerUpBrick>() {
                Some(power_up_brick) => {
                    assert_eq!(class, BrickClass::Reactor);
                    assert_eq!(health, 2);
                    assert_eq!(entity.get::<Sprite>().unwrap().color, theme::REACTOR);
                    assert!(power_up_brick.kind == PowerUpKind::SuperSizer);
                }
                None => {
                    assert_ne!(class, BrickClass::Reactor);
                    assert_eq!(health, class.max_hits());
                }
            }
        }
    }

    #[test]
    fn power_up_bricks_are_rechosen_every_run() {
        let mut app = app();
        let first = power_up_brick_positions(&mut app);

        tap(&mut app, KeyCode::Space);
        app.world_mut().resource_mut::<crate::Lives>().0 = 1;
        let ball = app
            .world_mut()
            .query_filtered::<Entity, With<crate::Ball>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<Transform>(ball)
            .unwrap()
            .translation
            .y = -WINDOW_HEIGHT;
        app.update();
        app.update();
        assert_eq!(app_state(&app), AppState::GameOver);
        tap(&mut app, KeyCode::KeyR);

        let second = power_up_brick_positions(&mut app);
        assert_eq!(second.len(), REACTOR_BRICKS);
        // Same six by chance: 1 in C(70, 6) ≈ 1.3e8.
        assert_ne!(first, second);
    }

    #[test]
    fn a_single_hit_only_cracks_a_power_up_brick() {
        let mut app = app();
        let brick = power_up_bricks(&mut app)[0];
        hit(&mut app, brick);
        for _ in 0..5 {
            app.update();
        }

        let entity = app.world().entity(brick);
        assert_eq!(entity.get::<BrickHealth>().unwrap().0, 1);
        assert_eq!(
            entity.get::<Sprite>().unwrap().color,
            theme::cracked(theme::REACTOR)
        );
        assert_eq!(app.world().resource::<crate::Score>().0, 10);
        assert_eq!(count::<With<PowerUp>>(&mut app), 0);
    }

    #[test]
    fn breaking_a_power_up_brick_drops_its_power_up_where_it_stood() {
        let mut app = app();
        let [first, second] = power_up_bricks(&mut app)[..2] else {
            unreachable!()
        };
        let at = position(&app, first);

        break_brick(&mut app, first);
        assert_eq!(app.world().resource::<crate::Score>().0, 20);
        let dropped = falling(&mut app);
        assert_eq!(dropped.len(), 1);
        let (_, pos, kind, gravity) = dropped[0];
        assert_eq!(pos, at);
        assert!(kind == PowerUpKind::SuperSizer);
        assert_eq!(gravity, BASE_GRAVITY);

        // The next drop this run is heavier.
        break_brick(&mut app, second);
        let gravities: Vec<f32> = falling(&mut app).iter().map(|f| f.3).collect();
        assert!(gravities.contains(&(BASE_GRAVITY + GRAVITY_STEP)));

        // A normal (ceramic, one-hit) brick drops nothing.
        let normal = brick_of(&mut app, BrickClass::Ceramic);
        hit(&mut app, normal);
        assert!(app.world().get_entity(normal).is_err());
        assert_eq!(count::<With<PowerUp>>(&mut app), 2);
    }

    #[test]
    fn catching_a_dropped_power_up_widens_the_paddle() {
        let mut app = app();
        let brick = power_up_bricks(&mut app)[0];
        break_brick(&mut app, brick);
        let (power_up, ..) = falling(&mut app)[0];
        let paddle = app
            .world_mut()
            .query_filtered::<Entity, With<Paddle>>()
            .single(app.world())
            .unwrap();
        let paddle_at = app.world().get::<Transform>(paddle).unwrap().translation;
        app.world_mut()
            .get_mut::<Transform>(power_up)
            .unwrap()
            .translation = paddle_at;
        app.update();
        app.update();

        assert_eq!(count::<With<PowerUp>>(&mut app), 0);
        assert!(app
            .world()
            .resource::<ActiveEffects>()
            .is_active(PowerUpKind::SuperSizer));
        assert!(app.world().get::<Paddle>(paddle).unwrap().width > crate::PADDLE_WIDTH);
    }

    #[test]
    fn no_power_ups_appear_without_breaking_a_power_up_brick() {
        let mut app = app();
        // 30 s at the test app's 100 ms step; the old timer fired every 7-10 s.
        for _ in 0..300 {
            app.update();
        }
        assert_eq!(app_state(&app), AppState::InGame);
        assert_eq!(count::<With<PowerUp>>(&mut app), 0);
    }

    #[test]
    fn abandoning_a_paused_run_for_the_main_menu_clears_power_ups() {
        let mut app = app();
        spawn_falling_power_up(&mut app);
        app.world_mut()
            .resource_mut::<ActiveEffects>()
            .refresh_or_insert(PowerUpKind::SuperSizer, 5.0);

        tap(&mut app, KeyCode::Escape);
        crate::menu::test_helpers::press(&mut app, "Main menu");
        assert_eq!(app_state(&app), AppState::MainMenu);
        assert_eq!(count::<With<PowerUp>>(&mut app), 0);

        crate::menu::test_helpers::press(&mut app, "Start");
        assert_eq!(app_state(&app), AppState::InGame);
        assert!(app.world().resource::<ActiveEffects>().0.is_empty());
        assert_eq!(count::<With<PowerUp>>(&mut app), 0);
    }
}
