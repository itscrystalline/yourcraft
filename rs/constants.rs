pub const HITBOX_WIDTH: u32 = 1;
pub const HITBOX_HEIGHT: u32 = 2;
pub const TICKS_PER_SECOND: u64 = 30;
/// How many times physics are calculated per second.
pub const PHYS_TICKS_PER_SECOND: u64 = 125;
/// How many movement updates are send per second.
pub const PACKET_UPDATES_PER_SECOND: u64 = 25;
pub const CONSOLE_UPDATE_RATE_MS: u64 = 50;
pub const SECONDS_BETWEEN_HEARTBEATS: u64 = 10;
/// unit: units / tick^2
pub const G: f32 = 9.81 / (PHYS_TICKS_PER_SECOND.pow(2) as f32);
pub const KNOCKBACK_POWER: f32 = 50.0 / (PHYS_TICKS_PER_SECOND.pow(2) as f32);
pub const AIR_RESISTANCE: f32 = 40.0 / (PHYS_TICKS_PER_SECOND.pow(2) as f32);
/// unit: units / tick (20ms)
pub const TERMINAL_VELOCITY: f32 = 54.0 / (PHYS_TICKS_PER_SECOND as f32);
pub const INITIAL_JUMP_SPEED: f32 = 25.0 / (PHYS_TICKS_PER_SECOND as f32);
pub const INITIAL_JUMP_ACCEL: f32 = 50.0 / (PHYS_TICKS_PER_SECOND.pow(2) as f32);
pub const RESPAWN_THRESHOLD: f32 = -256.0;
pub const MAX_INTERACT_RANGE: u32 = 10;
pub const PACKET_BATCH_THRESHOLD: usize = 5;
