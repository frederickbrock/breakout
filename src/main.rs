use macroquad::prelude::*;

// Game constants
const PADDLE_WIDTH: f32 = 120.0;
const PADDLE_HEIGHT: f32 = 20.0;
const PADDLE_SPEED: f32 = 500.0;
const BALL_SIZE: f32 = 15.0;
const BALL_SPEED: f32 = 300.0;
const BRICK_WIDTH: f32 = 80.0;
const BRICK_HEIGHT: f32 = 30.0;
const BRICK_ROWS: usize = 5;
const BRICK_COLS: usize = 10;

// Game state
struct GameState {
    paddle_x: f32,
    ball_x: f32,
    ball_y: f32,
    ball_velocity_x: f32,
    ball_velocity_y: f32,
    bricks: Vec<Brick>,
    score: i32,
    lives: i32,
    game_over: bool,
}

struct Brick {
    x: f32,
    y: f32,
    alive: bool,
    color: Color,
}

impl GameState {
    fn new() -> Self {
        let mut bricks = Vec::new();
        let colors = [RED, ORANGE, YELLOW, GREEN, BLUE];

        // Create brick grid
        for row in 0..BRICK_ROWS {
            for col in 0..BRICK_COLS {
                bricks.push(Brick {
                    x: col as f32 * (BRICK_WIDTH + 5.0) + 40.0,
                    y: row as f32 * (BRICK_HEIGHT + 5.0) + 50.0,
                    alive: true,
                    color: colors[row % colors.len()],
                });
            }
        }

        Self {
            paddle_x: screen_width() / 2.0 - PADDLE_WIDTH / 2.0,
            ball_x: screen_width() / 2.0,
            ball_y: screen_height() / 2.0,
            ball_velocity_x: BALL_SPEED,
            ball_velocity_y: -BALL_SPEED,
            bricks,
            score: 0,
            lives: 3,
            game_over: false,
        }
    }

    fn update(&mut self, delta_time: f32) {
        if self.game_over {
            return;
        }

        // Move paddle with arrow keys or mouse
        if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
            self.paddle_x -= PADDLE_SPEED * delta_time;
        }
        if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
            self.paddle_x += PADDLE_SPEED * delta_time;
        }

        // Clamp paddle to screen bounds
        self.paddle_x = self.paddle_x.max(0.0)
            .min(screen_width() - PADDLE_WIDTH);

        // Move ball
        self.ball_x += self.ball_velocity_x * delta_time;
        self.ball_y += self.ball_velocity_y * delta_time;

        // Ball collision with walls
        if self.ball_x <= 0.0 || self.ball_x >= screen_width() - BALL_SIZE {
            self.ball_velocity_x *= -1.0;
        }
        if self.ball_y <= 0.0 {
            self.ball_velocity_y *= -1.0;
        }

        // Ball collision with paddle
        if self.ball_y + BALL_SIZE >= screen_height() - PADDLE_HEIGHT - 10.0
            && self.ball_x >= self.paddle_x
            && self.ball_x <= self.paddle_x + PADDLE_WIDTH {
            self.ball_velocity_y *= -1.0;

            // Add spin based on where ball hits paddle
            let hit_pos = (self.ball_x - self.paddle_x) / PADDLE_WIDTH;
            self.ball_velocity_x = (hit_pos - 0.5) * BALL_SPEED * 2.0;
        }

        // Ball fell off bottom
        if self.ball_y > screen_height() {
            self.lives -= 1;
            if self.lives <= 0 {
                self.game_over = true;
            } else {
                self.reset_ball();
            }
        }

        // Ball collision with bricks
        for brick in &mut self.bricks {
            if !brick.alive {
                continue;
            }

            if self.ball_x + BALL_SIZE >= brick.x
                && self.ball_x <= brick.x + BRICK_WIDTH
                && self.ball_y + BALL_SIZE >= brick.y
                && self.ball_y <= brick.y + BRICK_HEIGHT {
                brick.alive = false;
                self.ball_velocity_y *= -1.0;
                self.score += 10;
                break;
            }
        }

        // Check win condition
        if self.bricks.iter().all(|b| !b.alive) {
            self.game_over = true;
        }
    }

    fn reset_ball(&mut self) {
        self.ball_x = screen_width() / 2.0;
        self.ball_y = screen_height() / 2.0;
        self.ball_velocity_x = BALL_SPEED;
        self.ball_velocity_y = -BALL_SPEED;
    }

    fn draw(&self) {
        clear_background(BLACK);

        // Draw paddle
        draw_rectangle(
            self.paddle_x,
            screen_height() - PADDLE_HEIGHT - 10.0,
            PADDLE_WIDTH,
            PADDLE_HEIGHT,
            WHITE,
        );

        // Draw ball
        draw_circle(self.ball_x, self.ball_y, BALL_SIZE / 2.0, WHITE);

        // Draw bricks
        for brick in &self.bricks {
            if brick.alive {
                draw_rectangle(
                    brick.x,
                    brick.y,
                    BRICK_WIDTH,
                    BRICK_HEIGHT,
                    brick.color,
                );
            }
        }

        // Draw UI
        draw_text(&format!("Score: {}", self.score), 20.0, 30.0, 30.0, WHITE);
        draw_text(&format!("Lives: {}", self.lives), 20.0, 60.0, 30.0, WHITE);

        if self.game_over {
            let text = if self.lives <= 0 {
                "GAME OVER - Press R to Restart"
            } else {
                "YOU WIN! - Press R to Restart"
            };
            let text_size = 40.0;
            let text_width = text.len() as f32 * text_size * 0.5;
            draw_text(
                text,
                screen_width() / 2.0 - text_width / 2.0,
                screen_height() / 2.0,
                text_size,
                YELLOW,
            );
        }
    }
}

#[macroquad::main("Breakout")]
async fn main() {
    let mut game = GameState::new();

    loop {
        let delta_time = get_frame_time();

        // Reset game on R key
        if is_key_pressed(KeyCode::R) {
            game = GameState::new();
        }

        game.update(delta_time);
        game.draw();

        next_frame().await;
    }
}
