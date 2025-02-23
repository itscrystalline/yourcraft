use crate::{
    constants,
    world::{is_solid, BlockPos, World, WorldError},
};
#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct Acceleration {
    pub x: f32,
    pub y: f32,
}

impl Velocity {
    fn accelerate(mut self, accel: Acceleration) -> Self {
        self.x += accel.x;
        self.y += accel.y;
        self
    }
    fn is_zero(&self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }
}
impl Acceleration {
    fn is_zero(&self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Player {
    pub x: f32,
    pub y: f32,
    pub hitbox_width: u32,
    pub hitbox_height: u32,
    pub velocity: Velocity,
    pub acceleration: Acceleration,
    pub do_jump: bool,
}

#[derive(Clone, Copy)]
pub struct Surrounding {
    pub top_left: Option<BlockPos>,
    pub top_center: Option<BlockPos>,
    pub top_right: Option<BlockPos>,
    pub left_up: Option<BlockPos>,
    pub upper_body: Option<BlockPos>,
    pub right_up: Option<BlockPos>,
    pub left_down: Option<BlockPos>,
    pub lower_body: Option<BlockPos>,
    pub right_down: Option<BlockPos>,
    pub bottom_left: Option<BlockPos>,
    pub bottom_center: Option<BlockPos>,
    pub bottom_right: Option<BlockPos>,
}
impl From<&[Option<BlockPos>]> for Surrounding {
    fn from(v: &[Option<BlockPos>]) -> Self {
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
            y: (highest_y + 1) as f32,
            hitbox_width: constants::HITBOX_WIDTH,
            hitbox_height: constants::HITBOX_HEIGHT,
            velocity: Velocity::default(),
            acceleration: Acceleration::default(),
            do_jump: false,
        })
    }

    pub fn do_collision(mut self, surrounding: Surrounding) -> (Self, bool) {
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
            left_up,
            left_down,
            right_up,
            right_down,
            ..
        } = surrounding;
        let [top_center_solid, bottom_center_solid, left_up_solid, left_down_solid, right_up_solid, right_down_solid] =
            [
                top_center,
                bottom_center,
                left_up,
                left_down,
                right_up,
                right_down,
            ]
            .map(|opt| match opt {
                None => false,
                Some((_, _, block)) => is_solid(block),
            });

        let mut has_changed = false;

        if self.y != snap_y {
            if bottom_center_solid && self.velocity.y < 0.0 {
                self.y = snap_y;
                has_changed = true;
            }

            if top_center_solid && self.velocity.y > 0.0 {
                self.y = snap_y;
                has_changed = true;
            }
        }
        if self.x != snap_x {
            if (left_up_solid || left_down_solid) && moving_left {
                self.x = snap_x;
                has_changed = true;
            }

            if (right_up_solid || right_down_solid) && moving_right {
                self.x = snap_x;
                has_changed = true;
            }
        }

        (self, has_changed)
    }

    fn is_grounded(y: f32, surrounding: Surrounding) -> bool {
        let Surrounding {
            bottom_left,
            bottom_center,
            bottom_right,
            ..
        } = surrounding;

        let [bottom_left_solid, bottom_center_solid, bottom_right_solid] =
            [bottom_left, bottom_center, bottom_right].map(|opt| match opt {
                None => false,
                Some((_, _, block)) => is_solid(block),
            });
        (bottom_left_solid || bottom_center_solid || bottom_right_solid) && y.round() == y
    }

    pub fn do_fall(mut self, surrounding: Surrounding) -> Self {
        match !Self::is_grounded(self.y, surrounding) {
            true => {
                self.velocity.y = self.velocity.y.max(-constants::TERMINAL_VELOCITY);
                self.y += self.velocity.y;
                self.acceleration.y -= constants::G;
            }
            false => {
                (self.velocity.y, self.acceleration.y) = (0.0, 0.0);
            }
        }
        self
    }

    pub fn do_move(mut self, surrounding: Surrounding) -> (Self, bool) {
        // jump
        if self.do_jump && Self::is_grounded(self.y, surrounding) {
            self.acceleration.y = constants::INITIAL_JUMP_ACCEL;
            self.velocity.y = constants::INITIAL_JUMP_SPEED;
            self.y += self.velocity.y;
        }
        self.do_jump = false;

        self = self.do_fall(surrounding);
        match self.velocity.is_zero() && self.acceleration.is_zero() {
            true => (self, false),
            false => {
                self.velocity = self.velocity.accelerate(self.acceleration);
                self.x += self.velocity.x;
                (self, true)
            }
        }
    }
}
