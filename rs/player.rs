use crate::{
    console::ToConsole,
    constants,
    world::{is_solid, BlockPos, World, WorldError},
};

#[derive(Debug, PartialEq, Clone)]
pub struct Player {
    pub x: f32,
    pub y: f32,
    pub hitbox_width: u32,
    pub hitbox_height: u32,
    pub velocity: f32,
    pub acceleration: f32,
}

impl Player {
    pub fn spawn_at(to_console: ToConsole, world: &World, x: u32) -> Result<Self, WorldError> {
        let highest = world.get_highest_block_at(to_console, x)?;
        Ok(Player {
            x: highest.0 as f32,
            y: highest.1 as f32,
            hitbox_width: constants::HITBOX_WIDTH,
            hitbox_height: constants::HITBOX_HEIGHT,
            velocity: 0.0,
            acceleration: 0.0,
        })
    }

    pub fn do_collision(mut self, surrounding: [BlockPos; 6]) -> (Self, bool) {
        // bottom corner
        let (snap_x, snap_y) = (self.x.round(), self.y.round());

        let [bottom, top, left_up, left_down, right_up, right_down] = surrounding;

        let mut has_changed = false;

        if is_solid(bottom.2) && self.y != snap_y {
            self.y = snap_y;
            has_changed = true;
        }

        if is_solid(top.2) && self.y != snap_y {
            self.y = snap_y;
            has_changed = true;
        }

        if (is_solid(left_up.2) || is_solid(left_down.2)) && self.x != snap_x {
            self.x = snap_x;
            has_changed = true;
        }

        if (is_solid(right_up.2) || is_solid(right_down.2)) && self.x != snap_x {
            self.x = snap_x;
            has_changed = true;
        }

        (self, has_changed)
    }

    fn is_grounded(y: f32, surrounding: &[BlockPos; 6]) -> bool {
        is_solid(surrounding[0].2) && y.round() == y
    }

    pub fn do_fall(mut self, surrounding: [BlockPos; 6]) -> (Self, bool) {
        if !Self::is_grounded(self.y, &surrounding) {
            self.velocity += self.acceleration;
            self.velocity = self.velocity.max(-constants::TERMINAL_VELOCITY);
            self.y += self.velocity;
            self.acceleration -= constants::G;
            (self, true)
        } else {
            (self.velocity, self.acceleration) = (0.0, 0.0);
            (self, false)
        }
    }

    pub fn do_jump(mut self, surrounding: [BlockPos; 6]) -> Self {
        if Self::is_grounded(self.y, &surrounding) {
            self.acceleration += constants::INITIAL_JUMP_ACCEL;
            self.velocity += constants::INITIAL_JUMP_SPEED;
            self.y += self.velocity;
        }
        self
    }
}
