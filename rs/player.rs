use crate::{
    constants,
    world::{is_solid, BlockPos, World, WorldError},
};

#[derive(Debug, PartialEq, Clone)]
pub struct Player {
    pub x: f32,
    pub y: f32,
    pub hitbox_width: u32,
    pub hitbox_height: u32,
    velocity: f32,
    acceleration: f32,
}

impl Player {
    pub fn spawn_at_origin() -> Self {
        Player {
            x: 0.0,
            y: 0.0,
            hitbox_width: constants::HITBOX_WIDTH,
            hitbox_height: constants::HITBOX_HEIGHT,
            velocity: 0.0,
            acceleration: 0.0,
        }
    }

    pub fn spawn_at(world: &World, x: u32) -> Result<Self, WorldError> {
        let highest = world.get_highest_block_at(x)?;
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
        let (snap_x, snap_y) = (self.x.round() as u32, self.y.round() as u32);

        let [bottom, top, left_up, left_down, right_up, right_down] = surrounding;

        let mut has_changed = false;

        if is_solid(bottom.2) || is_solid(top.2) {
            self.x = snap_x as f32;
            has_changed = true;
        }

        (self, has_changed)
    }

    fn is_grounded(surrounding: &[BlockPos; 6]) -> bool {
        is_solid(surrounding[0].2)
    }

    pub fn do_fall(mut self, surrounding: [BlockPos; 6]) -> (Self, bool) {
        if !Self::is_grounded(&surrounding) {
            self.velocity = f32::min(
                self.velocity + self.acceleration,
                constants::TERMINAL_VELOCITY,
            );
            self.y -= self.velocity;
            self.acceleration += constants::G;
            (self, true)
        } else {
            (self, false)
        }
    }
}
