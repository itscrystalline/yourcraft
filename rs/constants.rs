pub const HITBOX_WIDTH: u32 = 1;
pub const HITBOX_HEIGHT: u32 = 2;
pub const TICKS_PER_SECOND: u64 = 50;
pub const CONSOLE_UPDATE_RATE_MS: u64 = 50;
pub const SECONDS_BETWEEN_HEARTBEATS: u64 = 10;
/// unit: units / tick^2
pub const G: f32 = 9.81 / (TICKS_PER_SECOND.pow(2) as f32);
/// unit: units / tick (20ms)
pub const TERMINAL_VELOCITY: f32 = 54.0 / (TICKS_PER_SECOND as f32);
pub const INITIAL_JUMP_SPEED: f32 = 25.0 / (TICKS_PER_SECOND as f32);
pub const INITIAL_JUMP_ACCEL: f32 = 50.0 / (TICKS_PER_SECOND.pow(2) as f32);
