use crate::world::{World, WorldError};

#[derive(Debug, PartialEq)]
pub struct Player {
    pub x: f32,
    pub y: f32,
    pub hitbox_width: f32,
    pub hitbox_height: f32,
}

impl Player {
    pub fn spawn_at_origin() -> Self {
        Player {
            x: 0.0,
            y: 0.0,
            hitbox_width: 1.0,
            hitbox_height: 2.0,
        }
    }

    pub fn spawn_at(world: &World, x: u32) -> Result<Self, WorldError> {
        let highest = world.get_highest_block_at(x)?;
        Ok(Player {
            x: highest.0 as f32,
            y: highest.1 as f32,
            hitbox_width: 1.0,
            hitbox_height: 2.0,
        })
    }
}
