use crate::console::ToConsole;
use crate::network::{ClientConnection, PacketTypes, ToNetwork};
use crate::player::{Item, Player, Surrounding};
use crate::{c_debug, c_error, c_info, WorldType};
use fast_poisson::Poisson;
use itertools::Itertools;
use noise::{NoiseFn, OpenSimplex, Perlin};
use rand::rngs::SmallRng;
use rand::{Rng, RngCore, SeedableRng};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io;
use std::iter::zip;
use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::ops::Range;
use std::time::{Duration, Instant};
use strum::EnumString;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorldError {
    #[error("World width and height can only be a multiple of chunk_size!")]
    MismatchedChunkSize,
    #[error("block position ({0}, {1}) out of world bounds")]
    OutOfBoundsBlock(u32, u32),
    #[error("chunk position ({0}, {1}) out of world bounds")]
    OutOfBoundsChunk(u32, u32),
    //#[error("player interaction outside loaded chunk")]
    //OutOfLoadedChunk,
    #[error("chunk is already loaded")]
    ChunkAlreadyLoaded,
    #[error("invalid generation heights, requested to generate terrain from y 0 - {0} but world's size range is 0 - {1}")]
    InvalidGenerationRange(u32, u32),
    #[error("upper_height cannot be less than or equal to base_height")]
    GenerationTooThin,
    #[error("spawn range too large (bigger than world width / 2)")]
    SpawnRangeTooLarge,
    #[error("error propagating changes to clients: {0}")]
    NetworkError(#[from] io::Error),
}

#[derive(Debug)]
pub struct PositionUpdate {
    pub pos_x: f32,
    pub pos_y: f32,
    pub recievers: Vec<SocketAddr>,
}

#[derive(Debug)]
pub struct World {
    pub width: u32,
    pub height: u32,
    pub chunk_size: u32,
    pub chunks: Vec<Chunk>,
    width_chunks: u32,
    height_chunks: u32,
    pub players: Vec<ClientConnection>,
    player_loaded: Vec<Vec<u32>>,
    pub physics_update_queue: HashMap<u32, PositionUpdate>,
    to_update: HashSet<(u32, u32, Block)>,
    pub spawn_point: u32,
    pub spawn_range: NonZeroU32,
}

struct SurroundingBlocks {
    top: Option<BlockPos>,
    bottom: Option<BlockPos>,
    left: Option<BlockPos>,
    right: Option<BlockPos>,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub size: u32,
    pub chunk_x: u32,
    pub chunk_y: u32,
    pub blocks: Vec<Block>,
}

pub type BlockPos = (u32, u32, Block);

pub struct BlockProperties {
    solid: bool,
    item: Option<Item>,
    hardness: u8,
}

macro_rules! define_blocks {
    ($($name: ident = $id: expr => { $($prop_name:ident : $prop_value:expr),* $(,)? }),* $(,)?) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, EnumString)]
        pub enum Block {
            $($name = $id),*
        }

        impl From<u8> for Block {
            fn from(id: u8) -> Self {
                match id {
                    $($id => Block::$name),*,
                    _ => Block::Air,
                }
            }
        }

        impl From<Block> for u8 {
            fn from(block: Block) -> u8 { block as u8 }
        }

        impl Block {
            fn properties(self) -> BlockProperties {
                match self {
                    $(Block::$name => BlockProperties { $($prop_name: $prop_value),* }),*
                }
            }
        }

        pub fn is_solid(block: Block) -> bool {
            block.properties().solid
        }

        impl From<Block> for Option<Item> {
            fn from(block: Block) -> Self {
                block.properties().item
            }
        }
    };
}

#[derive(Debug)]
struct TerrainSettings {
    base_height: u32,
    upper_height: u32,
    water_height: u32,
    seed: Option<u64>,
    noise_passes: usize,
    redistribution_factor: f64,
    cave_gen_size: f64,
    tree_spawn_radius: f64,
}

enum TreeTypes {
    Basic,
}
macro_rules! map_to_trunk {
    ($trunk_x: expr, $trunk_y: expr, $trunk_offset: expr, $spaces: expr) => {
        $spaces
            .into_iter()
            .map(|(x, y, block)| {
                (
                    (x + $trunk_x as i32) as u32,
                    (y + $trunk_y as i32) as u32,
                    block,
                )
            })
            .collect()
    };
}
impl TreeTypes {
    pub fn get_required_blocks(tree: TreeTypes, trunk_x: u32, trunk_y: u32) -> Vec<BlockPos> {
        match tree {
            TreeTypes::Basic => {
                let layout = vec![
                    (0, 5, Block::Leaves),
                    (-1, 4, Block::Leaves),
                    (0, 4, Block::Leaves),
                    (1, 4, Block::Leaves),
                    (-2, 3, Block::Leaves),
                    (-1, 3, Block::Leaves),
                    (0, 3, Block::Wood),
                    (1, 3, Block::Leaves),
                    (2, 3, Block::Leaves),
                    (0, 2, Block::Wood),
                    (0, 1, Block::Wood),
                    (0, 0, Block::Wood),
                ];
                map_to_trunk!(trunk_x, trunk_y, 2, layout)
            }
        }
    }
}

impl World {
    pub fn generate(
        to_console: ToConsole,
        width: u32,
        height: u32,
        chunk_size: u32,
        spawn_point: u32,
        spawn_range: NonZeroU32,
        type_settings: WorldType,
    ) -> Result<World, WorldError> {
        let base = World::generate_empty(
            to_console.clone(),
            width,
            height,
            chunk_size,
            spawn_point,
            spawn_range,
        )?;

        Ok(match type_settings {
            WorldType::Empty => base,
            WorldType::Flat { grass_height } => {
                World::generate_flat(to_console, base, grass_height)?
            }
            WorldType::Terrain {
                base_height,
                upper_height,
                seed,
                noise_passes,
                redistribution_factor,
                water_height,
                cave_gen_size,
                tree_spawn_radius,
            } => World::generate_terrain(
                to_console,
                base,
                TerrainSettings {
                    base_height,
                    upper_height,
                    water_height,
                    seed,
                    noise_passes,
                    redistribution_factor,
                    cave_gen_size,
                    tree_spawn_radius,
                },
            )?,
        })
    }
    fn generate_empty(
        to_console: ToConsole,
        width: u32,
        height: u32,
        chunk_size: u32,
        spawn_point: u32,
        spawn_range: NonZeroU32,
    ) -> Result<World, WorldError> {
        if width % chunk_size != 0 || height % chunk_size != 0 {
            Err(WorldError::MismatchedChunkSize)
        } else {
            let start = Instant::now();
            let width_chunks = width / chunk_size;
            let height_chunks = height / chunk_size;
            let (chunks, player_loaded) = (0..width_chunks * height_chunks)
                .into_par_iter()
                .map(|idx| {
                    let chunk_x = idx % width_chunks;
                    let chunk_y = idx / width_chunks;
                    (Chunk::empty(chunk_size, chunk_x, chunk_y), vec![])
                })
                .collect();

            c_info!(
                to_console,
                "Generated {} empty chunks in {:?}",
                width_chunks * height_chunks,
                start.elapsed()
            );
            Ok(World {
                width,
                height,
                chunk_size,
                chunks,
                width_chunks,
                height_chunks,
                players: vec![],
                player_loaded,
                to_update: HashSet::new(),
                physics_update_queue: HashMap::new(),
                spawn_point,
                spawn_range,
            })
        }
    }

    fn generate_flat(
        to_console: ToConsole,
        mut world: World,
        grass_level: u32,
    ) -> Result<World, WorldError> {
        let start = Instant::now();

        if grass_level != 0 {
            for idx in 0..world.width * grass_level {
                let x = idx % world.width;
                let y = idx / world.width;
                world.set_block(x, y, Block::Stone)?
            }
        }
        for x in 0..world.width {
            world.set_block(x, grass_level, Block::Grass)?
        }

        c_info!(
            to_console,
            "filled {} * {} area with grass and stone in {:?}",
            world.width,
            grass_level,
            start.elapsed()
        );
        Ok(world)
    }

    fn generate_terrain(
        to_console: ToConsole,
        mut world: World,
        terrain_settings: TerrainSettings,
    ) -> Result<World, WorldError> {
        type TerrainGenerator = Box<dyn FnMut(f64, f64, f64) -> (f64, f64)>;

        let start = Instant::now();

        if terrain_settings.upper_height > world.height {
            return Err(WorldError::InvalidGenerationRange(
                terrain_settings.upper_height,
                world.height,
            ));
        }
        if terrain_settings.base_height >= terrain_settings.upper_height {
            return Err(WorldError::GenerationTooThin);
        }
        let cave_gen_size = terrain_settings.cave_gen_size.clamp(0.0, 1.0);

        let master_seed = terrain_settings.seed.unwrap_or(rand::rng().next_u64());
        let mut seed_generator = SmallRng::seed_from_u64(master_seed);
        let height_range = (terrain_settings.upper_height - terrain_settings.base_height) as f64;

        let mut generators: Vec<TerrainGenerator> = (0..terrain_settings.noise_passes)
            .map(|pass| {
                let seed = seed_generator.next_u32();
                Box::new(move |x_f, multiplier, octaves| {
                    let perlin = Perlin::new(seed);
                    let pass_2n = 2f64.powi(pass as i32);
                    let noise = perlin.get([x_f * pass_2n]) / 2.0 + 0.5;
                    let octave = 1f64 / pass_2n;
                    (multiplier + (octave * noise), octaves + octave)
                }) as TerrainGenerator
            })
            .collect();
        let cave_generator = {
            let seed = seed_generator.next_u32();
            move |x, y| {
                let simplex = OpenSimplex::new(seed);
                simplex.get([x * 0.001 * 32.0, y * 0.001 * 32.0]).abs()
            }
        };
        let mut trees = Poisson::<1>::new()
            .with_seed(seed_generator.next_u64())
            .with_dimensions([world.width as f64], terrain_settings.tree_spawn_radius)
            .into_iter()
            .map(|pos| pos[0].round() as u32)
            .unique()
            .sorted();

        c_debug!(to_console, "trees: {trees:?}");

        let mut next_tree = trees.next();
        for x in 0..world.width {
            let x_f = x as f64 * 0.005;
            let mut multiplier = 0.0;
            let mut octaves = 0.0;
            generators.iter_mut().for_each(|generator| {
                (multiplier, octaves) = generator(x_f, multiplier, octaves);
            });
            multiplier /= octaves;
            multiplier = multiplier.powf(terrain_settings.redistribution_factor);
            let height = terrain_settings.base_height + (multiplier * height_range).round() as u32;

            let (mut top_y, mut prev_top_y) = (0u32, 0u32);
            for y in 0..=u32::max(height, terrain_settings.water_height) {
                let block = {
                    let noise_here = cave_generator(x as f64, y as f64);
                    if noise_here < cave_gen_size {
                        Block::Air
                    } else {
                        prev_top_y = top_y;
                        top_y = y;
                        Block::Stone
                    }
                };
                world.set_block(x, y, block)?;

                if y > height {
                    world.set_block(x, y, Block::Water)?;
                }
            }

            let should_place_grass = top_y > terrain_settings.water_height;
            if top_y - prev_top_y != 1 {
                if !is_solid(world.get_block(x, top_y)?) {
                    world.set_block(x, top_y, Block::Air)?;
                }
            } else if should_place_grass {
                world.set_block(x, top_y, Block::Grass)?;
            }

            if let Some(tree) = next_tree {
                if x == tree {
                    if should_place_grass {
                        let _ = world.generate_tree_at(x, top_y + 1);
                    }
                    next_tree = trees.next();
                }
            }
        }

        c_info!(
            to_console,
            "Generation of terrain with seed {} took {:?}.",
            master_seed,
            start.elapsed()
        );

        let start = Instant::now();
        while !world.to_update.is_empty() {
            world.init_flow_water()?;
        }
        c_info!(to_console, "Flowing water took {:?}.", start.elapsed());

        Ok(world)
    }

    fn generate_tree_at(&mut self, trunk_x: u32, trunk_y: u32) -> Result<(), WorldError> {
        let space = TreeTypes::get_required_blocks(TreeTypes::Basic, trunk_x, trunk_y);
        space.into_iter().try_for_each(|(x, y, block)| {
            self.raw_set_block(x, y, block)?;
            Ok(())
        })
    }

    fn check_out_of_bounds_chunk(&self, chunk_x: u32, chunk_y: u32) -> Result<(), WorldError> {
        if chunk_x >= self.width_chunks || chunk_y >= self.height_chunks {
            Err(WorldError::OutOfBoundsChunk(chunk_x, chunk_y))
        } else {
            Ok(())
        }
    }
    fn check_out_of_bounds_block(&self, x: u32, y: u32) -> Result<(), WorldError> {
        if x >= self.width || y >= self.height {
            Err(WorldError::OutOfBoundsBlock(x, y))
        } else {
            Ok(())
        }
    }

    pub fn get_spawn(&self) -> u32 {
        let spawn_range = Range {
            start: self.spawn_point.saturating_sub(self.spawn_range.into()),
            end: std::cmp::min(
                self.spawn_point.saturating_add(self.spawn_range.into()),
                self.width,
            ),
        };
        rand::rng().random_range(spawn_range)
    }

    pub fn set_spawn(&mut self, x: u32) -> Result<(), WorldError> {
        self.check_out_of_bounds_block(x, 0)?;
        self.spawn_point = x;
        Ok(())
    }

    pub fn set_spawn_range(&mut self, range: NonZeroU32) -> Result<(), WorldError> {
        if range.get() > (self.width / 2) {
            Err(WorldError::SpawnRangeTooLarge)
        } else {
            self.spawn_range = range;
            Ok(())
        }
    }

    pub fn get_chunk_mut(&mut self, chunk_x: u32, chunk_y: u32) -> Result<&mut Chunk, WorldError> {
        self.check_out_of_bounds_chunk(chunk_x, chunk_y)?;
        Ok(&mut self.chunks[(chunk_y * self.width_chunks + chunk_x) as usize])
    }

    pub fn get_chunk(&self, chunk_x: u32, chunk_y: u32) -> Result<&Chunk, WorldError> {
        self.check_out_of_bounds_chunk(chunk_x, chunk_y)?;
        Ok(&self.chunks[(chunk_y * self.width_chunks + chunk_x) as usize])
    }

    pub fn mark_chunk_loaded_by_id(
        &mut self,
        chunk_x: u32,
        chunk_y: u32,
        player_loading_id: u32,
    ) -> Result<&Chunk, WorldError> {
        self.check_out_of_bounds_chunk(chunk_x, chunk_y)?;
        let players_loading_chunk =
            &mut self.player_loaded[(chunk_y * self.width_chunks + chunk_x) as usize];
        match players_loading_chunk
            .iter()
            .any(|&loading| loading == player_loading_id)
        {
            true => Err(WorldError::ChunkAlreadyLoaded),
            false => {
                if self
                    .players
                    .par_iter()
                    .any(|player| player.id == player_loading_id)
                {
                    players_loading_chunk.push(player_loading_id);
                }
                Ok(self.get_chunk(chunk_x, chunk_y)?)
            }
        }
    }

    pub fn unmark_loaded_chunk_for(
        &mut self,
        chunk_x: u32,
        chunk_y: u32,
        player_loading_id: u32,
    ) -> Result<(), WorldError> {
        self.check_out_of_bounds_chunk(chunk_x, chunk_y)?;
        let players_loading_chunk =
            &mut self.player_loaded[(chunk_y * self.width_chunks + chunk_x) as usize];
        players_loading_chunk.retain(|&con| player_loading_id != con);
        Ok(())
    }

    pub fn unload_all_for(&mut self, player_loading_id: u32) {
        self.player_loaded
            .par_iter_mut()
            .for_each(|players_loading_chunk| {
                players_loading_chunk.retain(|&con| player_loading_id != con);
            });
    }

    pub fn get_list_of_players_loading_chunk(
        &self,
        chunk_x: u32,
        chunk_y: u32,
    ) -> Result<Vec<&ClientConnection>, WorldError> {
        self.get_chunk(chunk_x, chunk_y)?; // to perform the oob check
        let players_loading_ids =
            &self.player_loaded[(chunk_y * self.width_chunks + chunk_x) as usize];
        let players_loading = players_loading_ids
            .iter()
            .map(|&id| self.players.iter().find(|&conn| conn.id == id).unwrap())
            .collect();
        Ok(players_loading)
    }

    pub fn raw_set_block(
        &mut self,
        pos_x: u32,
        pos_y: u32,
        block: Block,
    ) -> Result<(), WorldError> {
        self.check_out_of_bounds_block(pos_x, pos_y)?;

        let (chunk_x, chunk_y) = self.get_chunk_block_is_in(pos_x, pos_y)?;
        let pos_inside_chunk_x = pos_x - chunk_x * self.chunk_size;
        let pos_inside_chunk_y = pos_y - chunk_y * self.chunk_size;

        let chunk = self.get_chunk_mut(chunk_x, chunk_y)?;

        chunk.set_block(pos_inside_chunk_x, pos_inside_chunk_y, block);
        Ok(())
    }

    fn get_neighbours(&self, x: u32, y: u32) -> SurroundingBlocks {
        let (x_i, y_i) = (x as i32, y as i32);
        let [top, bottom, left, right] = [
            (x_i, y_i + 1),
            (x_i, y_i - 1),
            (x_i - 1, y_i),
            (x_i + 1, y_i),
        ]
        .map(|(bl_x, bl_y)| {
            if bl_x < 0 || bl_y < 0 {
                None
            } else {
                match self.get_block(bl_x as u32, bl_y as u32) {
                    Ok(bl) => Some((bl_x as u32, bl_y as u32, bl)),
                    Err(_) => None,
                }
            }
        });
        SurroundingBlocks {
            top,
            bottom,
            left,
            right,
        }
    }

    pub fn set_block(&mut self, pos_x: u32, pos_y: u32, block: Block) -> Result<(), WorldError> {
        self.raw_set_block(pos_x, pos_y, block)?;
        // update block
        if block == Block::Water {
            let SurroundingBlocks {
                bottom,
                left,
                right,
                ..
            } = self.get_neighbours(pos_x, pos_y);
            [bottom, left, right]
                .into_iter()
                .flatten()
                .for_each(|(x, y, bl)| {
                    if !is_solid(bl) && bl != Block::Water {
                        self.to_update.insert((x, y, Block::Water));
                    }
                });
        }
        Ok(())
    }

    pub fn get_block(&self, pos_x: u32, pos_y: u32) -> Result<Block, WorldError> {
        self.check_out_of_bounds_block(pos_x, pos_y)?;

        let (chunk_x, chunk_y) = self.get_chunk_block_is_in(pos_x, pos_y)?;
        let inside_x = pos_x - chunk_x * self.chunk_size;
        let inside_y = pos_y - chunk_y * self.chunk_size;

        let chunk = self.get_chunk(chunk_x, chunk_y)?;

        Ok(chunk.get_block(inside_x, inside_y))
    }

    pub async fn set_block_and_notify(
        &mut self,
        to_network: ToNetwork,
        pos_x: u32,
        pos_y: u32,
        block: Block,
    ) -> Result<(), WorldError> {
        self.set_block(pos_x, pos_y, block)?;
        let (chunk_x, chunk_y) = self.get_chunk_block_is_in(pos_x, pos_y)?;
        let players_loading = self.get_list_of_players_loading_chunk(chunk_x, chunk_y)?;

        players_loading.into_iter().for_each(|player| {
            encode_and_send!(
                to_network,
                PacketTypes::ServerUpdateBlock {
                    block: block.into(),
                    x: pos_x,
                    y: pos_y,
                },
                player.addr
            );
        });

        Ok(())
    }

    pub async fn shutdown(
        &mut self,
        to_console: ToConsole,
        to_network: ToNetwork,
    ) -> io::Result<()> {
        c_info!(to_console, "Shutting down Server!");
        let kick_msg = String::from("Server Shutting Down!");
        self.player_loaded
            .par_iter_mut()
            .for_each(|chunk| chunk.clear());

        self.players.iter_mut().for_each(|player| {
            encode_and_send!(
                to_network,
                PacketTypes::ServerKick {
                    msg: kick_msg.clone()
                },
                player.addr
            );
        });
        self.players.clear();
        Ok(())
    }

    pub async fn kick(
        &mut self,
        to_console: ToConsole,
        to_network: ToNetwork,
        id: u32,
        msg: Option<&str>,
    ) -> io::Result<()> {
        match self.players.iter().position(|x| x.id == id) {
            None => c_error!(to_console, "Player {} does not exist!", id),
            Some(idx) => {
                let connection = self.players.swap_remove(idx);
                let kick_msg = msg.unwrap_or("No kick message provided");
                self.unload_all_for(connection.id);
                c_info!(
                    to_console,
                    "{} (addr: {}) kicked from sever! ({})",
                    connection.name,
                    connection.addr,
                    kick_msg
                );

                let last_location = (
                    connection.server_player.x.round() as u32,
                    connection.server_player.y.round() as u32,
                );
                let last_location_chunk_pos = self
                    .get_chunk_block_is_in(last_location.0, last_location.1)
                    .unwrap();
                let players_loading_chunk = self
                    .get_list_of_players_loading_chunk(
                        last_location_chunk_pos.0,
                        last_location_chunk_pos.1,
                    )
                    .unwrap();

                encode_and_send!(
                    to_network,
                    PacketTypes::ServerKick {
                        msg: kick_msg.into(),
                    },
                    connection.addr
                );

                for player in self.players.iter() {
                    if players_loading_chunk.contains(&player) {
                        encode_and_send!(
                            to_network,
                            PacketTypes::ServerPlayerLeaveLoaded {
                                player_name: connection.name.clone(),
                                player_id: connection.id,
                            },
                            player.addr
                        );
                    }
                    encode_and_send!(
                        to_network,
                        PacketTypes::ServerPlayerLeave {
                            player_name: connection.name.clone(),
                            player_id: connection.id,
                        },
                        player.addr
                    );
                }
            }
        };
        Ok(())
    }

    pub fn get_chunk_block_is_in(&self, pos_x: u32, pos_y: u32) -> Result<(u32, u32), WorldError> {
        self.check_out_of_bounds_block(pos_x, pos_y)?;
        let chunk_x = pos_x / self.chunk_size;
        let chunk_y = pos_y / self.chunk_size;
        Ok((chunk_x, chunk_y))
    }

    pub fn get_highest_block_at(&self, x: u32) -> Result<(u32, u32), WorldError> {
        self.check_out_of_bounds_block(x, 0)?;

        let y: Vec<u32> = (0..self.height).collect();
        let top_block_window = y.par_windows(2).find_last(|window| {
            let block_next = self.get_block(x, window[1]);
            let block_prev = self.get_block(x, window[0]);

            if let (Ok(block_next), Ok(block_prev)) = (block_next, block_prev) {
                !is_solid(block_next) && is_solid(block_prev)
            } else {
                false
            }
        });
        Ok(match top_block_window {
            Some(window) => (x, window[0]),
            None => (x, 0),
        })
    }

    fn init_flow_water(&mut self) -> Result<(), WorldError> {
        let water_to_update: HashSet<&(u32, u32, Block)> = self
            .to_update
            .par_iter()
            .filter(|pos| pos.2 == Block::Water)
            .collect();

        let to_update: HashSet<(u32, u32)> = water_to_update
            .par_iter()
            .flat_map(|&&(x, y, bl)| {
                let SurroundingBlocks {
                    bottom,
                    left,
                    right,
                    ..
                } = self.get_neighbours(x, y);
                [bottom, left, right, Some((x, y, bl))]
            })
            .filter_map(|maybe_block| {
                if let Some((bl_x, bl_y, bl)) = maybe_block {
                    if !is_solid(bl) {
                        return Some((bl_x, bl_y));
                    }
                }
                None
            })
            .collect();
        self.to_update.retain(|pos| pos.2 != Block::Water);
        for (x, y) in to_update {
            self.set_block(x, y, Block::Water)?;
        }
        Ok(())
    }

    async fn tick_water(&mut self, to_network: ToNetwork) -> Result<(), WorldError> {
        let water_to_update: HashSet<&(u32, u32, Block)> = self
            .to_update
            .par_iter()
            .filter(|pos| pos.2 == Block::Water)
            .collect();

        let to_update: HashSet<(u32, u32)> = water_to_update
            .par_iter()
            .flat_map(|&&(x, y, bl)| {
                let SurroundingBlocks {
                    bottom,
                    left,
                    right,
                    ..
                } = self.get_neighbours(x, y);
                [bottom, left, right, Some((x, y, bl))]
            })
            .filter_map(|maybe_block| {
                if let Some((bl_x, bl_y, bl)) = maybe_block {
                    if !is_solid(bl) {
                        return Some((bl_x, bl_y));
                    }
                }
                None
            })
            .collect();
        self.to_update.retain(|pos| pos.2 != Block::Water);
        for (x, y) in to_update {
            self.set_block_and_notify(to_network.clone(), x, y, Block::Water)
                .await?;
        }
        Ok(())
    }

    pub fn get_neighbours_of_player(&self, player: &Player) -> Surrounding {
        let (grid_x, grid_y) = (player.x.round() as i32, player.y.round() as i32);
        let (hitbox_width, hitbox_height) =
            (player.hitbox_width as i32, player.hitbox_height as i32);

        let positions = [
            (grid_x - 1, grid_y + hitbox_height),
            (grid_x, grid_y + hitbox_height),
            (grid_x + hitbox_width, grid_y + hitbox_height),
            (grid_x - 1, grid_y + (hitbox_height / 2)),
            (grid_x, grid_y + (hitbox_height / 2)),
            (grid_x + hitbox_width, grid_y + (hitbox_height / 2)),
            (grid_x - 1, grid_y),
            (grid_x, grid_y),
            (grid_x + hitbox_width, grid_y),
            (grid_x - 1, grid_y - 1),
            (grid_x, grid_y - 1),
            (grid_x + hitbox_width, grid_y - 1),
        ];

        let block_pos_vec = positions.map(|(x, y)| {
            if x < 0 || y < 0 {
                None
            } else {
                match self.get_block(x as u32, y as u32) {
                    Ok(bl) => Some((x as u32, y as u32, bl)),
                    Err(_) => None,
                }
            }
        });
        Surrounding::from(block_pos_vec.as_slice())
    }

    pub fn notify_player_moved(
        &mut self,
        to_network: ToNetwork,
        new_player: &ClientConnection,
        old_x: f32,
        old_y: f32,
    ) -> io::Result<()> {
        let (old_chunk_x, old_chunk_y) = self
            .get_chunk_block_is_in(old_x.round() as u32, old_y.round() as u32)
            .unwrap_or((0, 0));
        let (chunk_x, chunk_y) = self
            .get_chunk_block_is_in(
                new_player.server_player.x.round() as u32,
                new_player.server_player.y.round() as u32,
            )
            .unwrap_or((0, 0));

        let players_loading_chunk: Vec<&ClientConnection> = self
            .get_list_of_players_loading_chunk(chunk_x, chunk_y)
            .unwrap_or_default()
            .into_iter()
            .filter(|conn| conn.id != new_player.id)
            .collect();

        let mut update_queue: Vec<SocketAddr> = Vec::new();

        if (old_chunk_x, old_chunk_y) == (chunk_x, chunk_y) {
            players_loading_chunk.into_iter().for_each(|conn| {
                update_queue.push(conn.addr);
            });
        } else {
            let players_loading_old_chunk: Vec<&ClientConnection> = self
                .get_list_of_players_loading_chunk(old_chunk_x, old_chunk_y)
                .unwrap_or_default()
                .into_iter()
                .filter(|conn| conn.id != new_player.id)
                .collect();
            let old_players: Vec<&ClientConnection> = players_loading_old_chunk
                .clone()
                .into_iter()
                .filter(|conn| !players_loading_chunk.contains(conn))
                .collect();
            let new_players: Vec<&ClientConnection> = players_loading_chunk
                .clone()
                .into_iter()
                .filter(|conn| !players_loading_old_chunk.contains(conn))
                .collect();
            old_players.into_iter().for_each(|conn| {
                encode_and_send!(
                    to_network,
                    PacketTypes::ServerPlayerLeaveLoaded {
                        player_id: conn.id,
                        player_name: conn.name.clone()
                    },
                    new_player.addr
                );
                encode_and_send!(
                    to_network,
                    PacketTypes::ServerPlayerLeaveLoaded {
                        player_id: new_player.id,
                        player_name: new_player.name.clone(),
                    },
                    conn.addr
                );
            });
            players_loading_chunk.into_iter().for_each(|conn| {
                if new_players.contains(&conn) {
                    encode_and_send!(
                        to_network,
                        PacketTypes::ServerPlayerEnterLoaded {
                            player_id: conn.id,
                            player_name: conn.name.clone(),
                            pos_x: conn.server_player.x,
                            pos_y: conn.server_player.y,
                        },
                        new_player.addr
                    );
                    encode_and_send!(
                        to_network,
                        PacketTypes::ServerPlayerEnterLoaded {
                            player_id: new_player.id,
                            player_name: new_player.name.clone(),
                            pos_x: new_player.server_player.x,
                            pos_y: new_player.server_player.y,
                        },
                        conn.addr
                    );
                }
                update_queue.push(conn.addr);
            });
        }

        update_queue.push(new_player.addr);
        self.physics_update_queue.insert(
            new_player.id,
            PositionUpdate {
                pos_x: new_player.server_player.x,
                pos_y: new_player.server_player.y,
                recievers: update_queue,
            },
        );
        Ok(())
    }

    pub async fn physics_tick(&mut self, to_network: ToNetwork) -> io::Result<Duration> {
        let now = Instant::now();

        let surrounding: Vec<Surrounding> = self
            .players
            .par_iter()
            .map(|conn| self.get_neighbours_of_player(&conn.server_player))
            .collect();
        let player_surrounding: Vec<(&ClientConnection, Surrounding)> =
            zip(&self.players, surrounding).collect();

        let res: Vec<(ClientConnection, bool, (f32, f32))> = player_surrounding
            .into_par_iter()
            .map(|(conn, surr)| {
                let mut new_player = conn.server_player.clone();
                let old_pos = (new_player.x, new_player.y);
                let (has_changed_collision, has_jumped);
                (new_player, has_jumped) = new_player.do_move(surr);
                (new_player, has_changed_collision) = new_player.do_collision(surr);
                (
                    ClientConnection::with(conn, new_player),
                    has_jumped | has_changed_collision,
                    old_pos,
                )
            })
            .collect();

        let mut new_players = vec![];
        for (new_player, update_pos, (old_x, old_y)) in res {
            if update_pos {
                self.notify_player_moved(to_network.clone(), &new_player, old_x, old_y)?;
            }
            new_players.push(new_player);
        }
        self.players = new_players;
        Ok(now.elapsed())
    }

    pub async fn flush_physics_queue(&mut self, to_network: ToNetwork) -> io::Result<()> {
        self.physics_update_queue.drain().for_each(
            |(
                player_id,
                PositionUpdate {
                    pos_x,
                    pos_y,
                    recievers,
                },
            )| {
                recievers.iter().for_each(|&reciever| {
                    encode_and_send!(
                        to_network,
                        PacketTypes::ServerPlayerUpdatePos {
                            player_id,
                            pos_x,
                            pos_y,
                        },
                        reciever
                    );
                });
            },
        );
        Ok(())
    }

    pub async fn world_tick(
        &mut self,
        to_console: ToConsole,
        to_network: ToNetwork,
    ) -> io::Result<Duration> {
        let now = Instant::now();

        if let Err(e) = self.tick_water(to_network).await {
            c_error!(to_console, "Error occurred while ticking water: {e}")
        };

        let time = now.elapsed();
        Ok(time)
    }
}

impl Chunk {
    fn empty(size: u32, chunk_x: u32, chunk_y: u32) -> Chunk {
        Chunk {
            size,
            chunk_x,
            chunk_y,
            blocks: (0..size.pow(2))
                .into_par_iter()
                .map(|_| Block::Air)
                .collect(),
        }
    }

    fn set_block(&mut self, chunk_pos_x: u32, chunk_pos_y: u32, block: Block) -> &mut Self {
        let idx = (chunk_pos_y * self.size + chunk_pos_x) as usize;
        self.blocks[idx] = block;
        self
    }

    fn get_block(&self, chunk_pos_x: u32, chunk_pos_y: u32) -> Block {
        self.blocks[(chunk_pos_y * self.size + chunk_pos_x) as usize]
    }
}

define_blocks! {
    Air = 0 => { solid: false, item: None, hardness: 0 },
    Grass = 1 => { solid: true, item: Some(Item::Grass), hardness: 0 },
    Stone = 2 => { solid: true, item: Some(Item::Stone), hardness: 1 },
    Wood = 3 => { solid: true, item: Some(Item::Wood), hardness: 0 },
    Leaves = 4 => { solid: true, item: Some(Item::Leaves), hardness: 0},
    Water = 5 => { solid: false, item: Some(Item::WaterBucket), hardness: 0},
}
