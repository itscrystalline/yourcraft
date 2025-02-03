use crate::world::{is_solid, BlockPos, World, WorldError};

#[derive(Debug, PartialEq, Clone)]
pub struct Player {
    pub x: f32,
    pub y: f32,
    pub hitbox_width: u32,
    pub hitbox_height: u32,
}

impl Player {
    pub fn spawn_at_origin() -> Self {
        Player {
            x: 0.0,
            y: 0.0,
            hitbox_width: 1,
            hitbox_height: 2,
        }
    }

    pub fn spawn_at(world: &World, x: u32) -> Result<Self, WorldError> {
        let highest = world.get_highest_block_at(x)?;
        Ok(Player {
            x: highest.0 as f32,
            y: highest.1 as f32,
            hitbox_width: 1,
            hitbox_height: 2,
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
}
