pub const HITBOX_WIDTH: u32 = 1;
pub const HITBOX_HEIGHT: u32 = 2;
pub const TICKS_PER_SECOND: u64 = 50;
pub const SECONDS_BETWEEN_HEARTBEATS: u64 = 10;
/// unit: units / tick^2
pub const G: f32 = 9.81 / (TICKS_PER_SECOND.pow(2) as f32);
/// unit: units / tick (20ms)
pub const TERMINAL_VELOCITY: f32 = 54.0 / (TICKS_PER_SECOND as f32);
