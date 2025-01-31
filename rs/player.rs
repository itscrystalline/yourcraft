use crate::world::World;

#[derive(Debug, PartialEq)]
pub struct Player {
    pub x: f32,
    pub y: f32,
}

impl Player {
    pub fn spawn_at_origin() -> Self {
        Player { x: 0.0, y: 0.0 }
    }

    pub fn spawn_at(world: &World, x: u32) -> Self {
        let highest = world.get_highest_block_at(x).unwrap();
        Player { x: highest.0 as f32, y: highest.1 as f32}
    }
}
