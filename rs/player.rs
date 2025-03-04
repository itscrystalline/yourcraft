use std::{cmp::Ordering, net::SocketAddr, num::NonZeroU8};

use crate::{
    constants,
    network::{PacketTypes, ToNetwork},
    world::{is_solid, Block, BlockPos, World, WorldError},
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
    pub health: f32,
    pub hitbox_width: u32,
    pub hitbox_height: u32,
    pub velocity: Velocity,
    pub acceleration: Acceleration,
    pub do_jump: bool,
    pub inventory: [Option<ItemStack>; 9],
    pub selected_slot: u8,
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
macro_rules! map_surrounding_solid {
    ($surrounding:expr, [$($field:ident),*]) => {{
        let Surrounding { $($field),*, .. } = $surrounding;

        [$($field),*].map(|opt| match opt {
            None => false,
            Some((_, _, block)) => is_solid(block),
        })
    }};
}

#[derive(PartialEq)]
enum Shift {
    Left,
    None,
    Right,
}

impl Player {
    pub fn spawn_at(world: &World, x: u32) -> Result<Self, WorldError> {
        let (highest_x, highest_y) = world.get_highest_block_at(x)?;
        Ok(Player {
            x: highest_x as f32,
            y: (highest_y + 1) as f32,
            health: 5.0,
            hitbox_width: constants::HITBOX_WIDTH,
            hitbox_height: constants::HITBOX_HEIGHT,
            velocity: Velocity::default(),
            acceleration: Acceleration::default(),
            do_jump: false,
            inventory: [None; 9],
            selected_slot: 0,
        })
    }

    pub fn do_collision(mut self, surrounding: Surrounding) -> (Self, bool) {
        // bottom corner
        let (snap_x, snap_y) = (self.x.round(), self.y.round());

        let direction = match (self.x - snap_x).partial_cmp(&0.0) {
            Some(Ordering::Less) => Shift::Left,
            Some(Ordering::Greater) => Shift::Right,
            _ => Shift::None,
        };

        let [top_left, top_center, top_right, bottom_left, bottom_center, bottom_right, left_up, left_down, right_up, right_down, upper_body, lower_body] = map_surrounding_solid!(
            surrounding,
            [
                top_left,
                top_center,
                top_right,
                bottom_left,
                bottom_center,
                bottom_right,
                left_up,
                left_down,
                right_up,
                right_down,
                upper_body,
                lower_body
            ]
        );

        let (bottom, top) = match direction {
            Shift::Left => (bottom_left || bottom_center, top_left || top_center),
            Shift::None => (bottom_center, top_center),
            Shift::Right => (bottom_center || bottom_right, top_center || top_right),
        };

        let mut has_changed = false;

        if self.y != snap_y && ((bottom && self.velocity.y < 0.0) || (top && self.velocity.y > 0.0))
        {
            self.y = snap_y;
            self.velocity.y = self.velocity.y.min(0.0);
            self.acceleration.y = self.acceleration.y.min(0.0);
            has_changed = true;
        }

        if self.x != snap_x
            && (((right_up || right_down) && direction == Shift::Right)
                || ((left_up || left_down) && direction == Shift::Left))
        {
            self.x = snap_x;
            has_changed = true;
        }

        match (lower_body, upper_body) {
            (false, false) => (),
            (true, false) => {
                if !top_center {
                    self.y = snap_y + 1.0;
                    has_changed = true;
                }
            }
            (false, true) => {
                if !bottom_center {
                    self.y = snap_y - 1.0;
                    has_changed = true;
                }
            }
            (true, true) => {
                match (
                    top_left || left_up,
                    left_down || bottom_left,
                    top_right || right_up,
                    right_down || bottom_right,
                ) {
                    (true, _, _, _) => {
                        self.x = snap_x - 1.0;
                        self.y = snap_y + 1.0;
                        has_changed = true;
                    }
                    (_, true, _, _) => {
                        self.x = snap_x - 1.0;
                        self.y = snap_y - 1.0;
                        has_changed = true;
                    }
                    (_, _, true, _) => {
                        self.x = snap_x + 1.0;
                        self.y = snap_y + 1.0;
                        has_changed = true;
                    }
                    (_, _, _, true) => {
                        self.x = snap_x + 1.0;
                        self.y = snap_y - 1.0;
                        has_changed = true;
                    }
                    _ => (),
                }
            }
        }

        (self, has_changed)
    }

    pub fn get_current_itemstack(&self) -> Option<ItemStack> {
        self.inventory[self.selected_slot as usize]
    }

    pub fn get_current_breaking_power(&self) -> u8 {
        match self.get_current_itemstack() {
            Some(ItemStack { item, .. }) => item.breaking_power(),
            None => 0,
        }
    }

    pub fn get_current_damage(&self) -> f32 {
        match self.get_current_itemstack() {
            Some(ItemStack {
                item: Item::WoodSword,
                ..
            }) => 0.5,
            Some(ItemStack {
                item: Item::WoodAxe | Item::WoodPickaxe,
                ..
            }) => 0.35,
            Some(_) => 0.2,
            None => 0.1,
        }
    }

    pub fn consume_current(&mut self) {
        if let Some(current) = self.inventory[self.selected_slot as usize] {
            self.inventory[self.selected_slot as usize] =
                NonZeroU8::new(current.count.get() - 1).map(|new| current.with_count(new));
        }
    }

    pub fn insert(&mut self, itemstack: ItemStack) -> Result<(), u8> {
        let mut count_left = itemstack.count.get();
        for stack in self.inventory.iter_mut() {
            if count_left == 0 {
                return Ok(());
            }
            match stack {
                None => {
                    *stack = Some(ItemStack {
                        item: itemstack.item,
                        count: NonZeroU8::new(count_left).unwrap_or_else(|| unreachable!()),
                    });
                    count_left = 0;
                }
                Some(stack) => {
                    if stack.item == itemstack.item {
                        match stack.count.checked_add(count_left) {
                            Some(c) => {
                                stack.count = c;
                                count_left = 0;
                            }
                            None => {
                                count_left = stack.count.get().wrapping_add(count_left + 1);
                                stack.count =
                                    NonZeroU8::new(u8::MAX).unwrap_or_else(|| unreachable!());
                            }
                        }
                    }
                }
            }
        }
        Err(count_left)
    }

    pub fn notify_inventory_changed(&self, to_network: ToNetwork, addr: SocketAddr) {
        encode_and_send!(
            to_network,
            PacketTypes::ServerUpdateInventory {
                inv: self
                    .inventory
                    .map(|stack_maybe| stack_maybe.map(|s| s.into())),
            },
            addr
        );
    }

    fn is_grounded(x: f32, y: f32, surrounding: Surrounding) -> bool {
        let snap_x = x.round();
        let direction = match (x - snap_x).partial_cmp(&0.0) {
            Some(Ordering::Less) => Shift::Left,
            Some(Ordering::Greater) => Shift::Right,
            _ => Shift::None,
        };

        let [bottom_left_solid, bottom_center_solid, bottom_right_solid] =
            map_surrounding_solid!(surrounding, [bottom_left, bottom_center, bottom_right]);
        let considered_solid = match direction {
            Shift::Left => bottom_left_solid || bottom_center_solid,
            Shift::Right => bottom_right_solid || bottom_center_solid,
            Shift::None => bottom_center_solid,
        };
        considered_solid && y.round() == y
    }

    fn do_fall(mut self, surrounding: Surrounding) -> Self {
        match !Self::is_grounded(self.x, self.y, surrounding) {
            true => {
                self.velocity.y = self.velocity.y.max(-constants::TERMINAL_VELOCITY);
                self.y += self.velocity.y;
                self.acceleration.y -= constants::G - Self::get_resistance(surrounding);
            }
            false => {
                (self.velocity.y, self.acceleration.y) = (0.0, 0.0);
            }
        }
        self
    }

    fn get_resistance(surrounding: Surrounding) -> f32 {
        let Surrounding {
            upper_body,
            lower_body,
            ..
        } = surrounding;
        let in_water = [upper_body, lower_body]
            .into_iter()
            .flatten()
            .any(|(_, _, bl)| bl == Block::Water);
        match in_water {
            true => constants::WATER_RESISTANCE,
            false => constants::AIR_RESISTANCE,
        }
    }

    fn do_air_resistance(mut self, surrounding: Surrounding) -> Self {
        match self.acceleration.x.partial_cmp(&0.0) {
            Some(Ordering::Less) => {
                self.acceleration.x =
                    f32::min(self.acceleration.x + Self::get_resistance(surrounding), 0.0);
            }
            Some(Ordering::Greater) => {
                self.acceleration.x =
                    f32::max(self.acceleration.x - Self::get_resistance(surrounding), 0.0);
            }
            _ => self.acceleration.x = 0.0,
        };
        self
    }

    pub fn do_move(mut self, surrounding: Surrounding) -> (Self, bool) {
        // jump
        if self.do_jump && Self::is_grounded(self.x, self.y, surrounding) {
            self.acceleration.y = constants::INITIAL_JUMP_ACCEL - Self::get_resistance(surrounding);
            self.velocity.y = constants::INITIAL_JUMP_SPEED;
            self.y += self.velocity.y;
        }
        self.do_jump = false;

        self = self.do_fall(surrounding);
        self = self.do_air_resistance(surrounding);
        // void check
        if self.y <= constants::RESPAWN_THRESHOLD {
            self.y = -constants::RESPAWN_THRESHOLD;
            self.x = self.x.max(0.0);
        }
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemStack {
    pub item: Item,
    pub count: NonZeroU8,
}

impl From<Item> for ItemStack {
    fn from(item: Item) -> Self {
        Self {
            item,
            count: NonZeroU8::new(1).unwrap_or_else(|| unreachable!()),
        }
    }
}
impl ItemStack {
    pub fn with_count(mut self, count: NonZeroU8) -> Self {
        self.count = count;
        self
    }
}

macro_rules! define_items {
    ($($name:ident = ($id:expr, $block_match:expr, $breaking_power: expr)),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum Item {
            $($name = $id),*
        }

        impl From<u8> for Item {
            fn from(id: u8) -> Self {
                match id {
                    $($id => Item::$name),*,
                    _ => Item::Grass,
                }
            }
        }

        impl Item {
            fn breaking_power(&self) -> u8 {
                match self {
                    $(Item::$name => $breaking_power),*,
                }
            }
        }

        impl From<Item> for u8 {
            fn from(item: Item) -> u8 { item as u8 }
        }

        impl From<Item> for Option<Block> {
            fn from(item: Item) -> Self {
                match item {
                    $(Item::$name => $block_match),*
                }
            }
        }
    }
}

define_items! {
    Grass = (0, Some(Block::Grass), 0),
    Stone = (1, Some(Block::Stone), 0),
    Wood = (2, Some(Block::Wood), 0),
    Leaves = (3, Some(Block::Leaves), 0),
    Bucket = (4, None, 0),
    WaterBucket = (5, Some(Block::Water), 0),
    WoodPickaxe = (6, None, 1),
    WoodAxe = (7, None, 1),
    WoodSword = (8, None, 0),
    Ore = (9, Some(Block::Ore), 0),
    OrePickaxe = (10, None, 2),
    OreAxe = (11, None, 2),
    OreSword = (12, None, 0)
}
