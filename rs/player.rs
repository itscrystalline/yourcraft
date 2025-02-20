use std::convert::Infallible;

use itertools::sorted;

use crate::{
    constants,
    world::{is_solid, BlockPos, World, WorldError},
};

#[derive(Debug, PartialEq, Clone)]
pub struct Player {
    pub x: f32,
    pub next_x: Option<f32>,
    pub y: f32,
    pub hitbox_width: u32,
    pub hitbox_height: u32,
    pub velocity: f32,
    pub acceleration: f32,
    pub do_jump: bool,
}

pub struct Surrounding {
    pub top_left: BlockPos,
    pub top_center: BlockPos,
    pub top_right: BlockPos,
    pub left_up: BlockPos,
    pub upper_body: BlockPos,
    pub right_up: BlockPos,
    pub left_down: BlockPos,
    pub lower_body: BlockPos,
    pub right_down: BlockPos,
    pub bottom_left: BlockPos,
    pub bottom_center: BlockPos,
    pub bottom_right: BlockPos,
}
impl From<&[BlockPos]> for Surrounding {
    fn from(v: &[BlockPos]) -> Self {
        Self {
            top_left: v[0],
            top_center: v[1],
            top_right: v[2],
            left_up: v[3],
            upper_body: v[4],
            right_up: v[5],
            left_down: v[6],
            lower_body: v[7],
            right_down: v[8],
            bottom_left: v[9],
            bottom_center: v[10],
            bottom_right: v[11],
        }
    }
}

impl Player {
    pub fn spawn_at(world: &World, x: u32) -> Result<Self, WorldError> {
        let (highest_x, highest_y) = world.get_highest_block_at(x)?;
        Ok(Player {
            x: highest_x as f32,
            next_x: None,
            y: (highest_y + 1) as f32,
            hitbox_width: constants::HITBOX_WIDTH,
            hitbox_height: constants::HITBOX_HEIGHT,
            velocity: 0.0,
            acceleration: 0.0,
            do_jump: false,
        })
    }

    pub fn move_x(mut self) -> (Self, bool) {
        let has_changed = self.next_x.is_some();
        if let Some(next) = self.next_x {
            self.x = next;
            self.next_x = None;
        }
        (self, has_changed)
    }

    pub fn do_collision(mut self, surrounding: &Surrounding) -> (Self, bool) {
        // bottom corner
        let (snap_x, snap_y) = (self.x.round(), self.y.round());

        let (mut moving_left, mut moving_right) = (false, false);

        if self.x - snap_x > 0.0 {
            moving_right = true;
        } else if self.x - snap_x < 0.0 {
            moving_left = true;
        }

        let Surrounding {
            top_center,
            bottom_center,
            ..
        } = surrounding;

        let mut has_changed = false;

        if self.y != snap_y {
            if is_solid(bottom_center.2) && self.velocity < 0.0 {
                self.y = snap_y;
                has_changed = true;
            }

            if is_solid(top_center.2) && self.velocity > 0.0 {
                self.y = snap_y;
                has_changed = true;
            }
        }
        //if self.x != snap_x {
        //    if (is_solid(left_up.2) || is_solid(left_down.2)) && moving_left {
        //        self.x = snap_x;
        //        has_changed = true;
        //    }
        //
        //    if (is_solid(right_up.2) || is_solid(right_down.2)) && moving_right {
        //        self.x = snap_x;
        //        has_changed = true;
        //    }
        //}

        (self, has_changed)
    }

    fn is_grounded(y: f32, surrounding: &Surrounding) -> bool {
        let Surrounding {
            bottom_left,
            bottom_center,
            bottom_right,
            ..
        } = surrounding;
        (is_solid(bottom_left.2) || is_solid(bottom_center.2) || is_solid(bottom_right.2))
            && y.round() == y
    }

    pub fn do_fall(mut self, surrounding: &Surrounding) -> (Self, bool) {
        if !Self::is_grounded(self.y, surrounding) {
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

    pub fn do_jump(mut self, surrounding: &Surrounding) -> (Self, bool) {
        if Self::is_grounded(self.y, surrounding) && self.do_jump {
            self.acceleration += constants::INITIAL_JUMP_ACCEL;
            self.velocity += constants::INITIAL_JUMP_SPEED;
            self.y += self.velocity;
            self.do_jump = false;
            (self, true)
        } else {
            (self, false)
        }
    }
}
