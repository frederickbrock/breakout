use super::{ActiveEffects, PowerUpCollected, PowerUpKind, PowerUpSpawner, TickActiveEffects};
use crate::{Paddle, PADDLE_HEIGHT, PADDLE_WIDTH};
use avian2d::prelude::*;
use bevy::prelude::*;

const WEIGHT: f32 = 1.0;
const WIDTH_MULTIPLIER: f32 = 1.25;
const DURATION: f32 = 7.0;

fn color() -> Color {
    let green: f32 = rand::random_range(0.0..1.0);
    let red: f32 = rand::random_range(0.0..1.0);
    let blue: f32 = rand::random_range(0.0..1.0);
    Color::srgb(red, green, blue)
}

pub struct SuperSizerPlugin;

impl Plugin for SuperSizerPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().resource_mut::<PowerUpSpawner>().register(
            PowerUpKind::SuperSizer,
            WEIGHT,
            color(),
        );

        app.add_observer(effect).add_systems(
            Update,
            update_paddle_width
                .after(TickActiveEffects)
                .before(crate::PaddleMovementSet),
        );
    }
}

fn effect(on: On<PowerUpCollected>, mut active: ResMut<ActiveEffects>) {
    if on.kind == PowerUpKind::SuperSizer {
        active.refresh_or_insert(PowerUpKind::SuperSizer, DURATION);
    }
}

fn update_paddle_width(
    active: Res<ActiveEffects>,
    mut paddle_query: Query<(&mut Paddle, &mut Sprite, &mut Collider)>,
) {
    let Ok((mut paddle, mut sprite, mut collider)) = paddle_query.single_mut() else {
        return;
    };
    let width = if active.is_active(PowerUpKind::SuperSizer) {
        PADDLE_WIDTH * WIDTH_MULTIPLIER
    } else {
        PADDLE_WIDTH
    };
    if paddle.width != width {
        paddle.width = width;
        sprite.custom_size = Some(Vec2::new(width, PADDLE_HEIGHT));
        *collider = Collider::rectangle(width, PADDLE_HEIGHT);
    }
}
