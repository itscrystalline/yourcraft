use crate::console::ToConsole;
use crate::network::{
    ClientConnection, PacketTypes, ServerKick, ServerPlayerEnterLoaded, ServerPlayerUpdatePos,
    ServerUpdateBlock,
};
use crate::network::{Packet, ServerPlayerLeave, ServerPlayerLeaveLoaded};
use crate::player::Player;
use crate::{c_debug, c_error, c_info, WorldType};
use get_size::GetSize;
use noise::{NoiseFn, OpenSimplex, Perlin};
use rand::rngs::SmallRng;
use rand::{Rng, RngCore, SeedableRng};
use ratatui::symbols::block;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;
use std::iter::zip;
use std::num::NonZeroU32;
use std::ops::{AddAssign, Neg, Range};
use std::time::{Duration, Instant};
use strum::EnumString;
use thiserror::Error;
use tokio::net::UdpSocket;

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
pub struct World {
    pub width: u32,
    pub height: u32,
    pub chunk_size: u32,
    pub chunks: Vec<Chunk>,
    width_chunks: u32,
    height_chunks: u32,
    pub players: Vec<ClientConnection>,
    player_loaded: Vec<Vec<u32>>,
    to_update: HashSet<(u32, u32, Block)>,
    pub spawn_point: u32,
    pub spawn_range: NonZeroU32,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub size: u32,
    pub chunk_x: u32,
    pub chunk_y: u32,
    pub blocks: Vec<Block>,
}

pub type BlockPos = (u32, u32, Block);

macro_rules! define_blocks {
    ($($name:ident = ($id:expr, $solid:expr)),* $(,)?) => {
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

        pub fn is_solid(block: Block) -> bool {
            match block {
                $(Block::$name => $solid),*
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
}

#[derive(Default, Debug)]
struct AngleDeg(f64);

impl AddAssign for AngleDeg {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
        self.0 %= 360.0;
    }
}
impl Neg for AngleDeg {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self(360.0 - self.0)
    }
}
impl From<f64> for AngleDeg {
    fn from(value: f64) -> Self {
        let mut value = value;
        if !(-360.0..360.0).contains(&value) {
            value %= 360.0;
        }
        if value < 0.0 {
            value += 360.0;
        }
        Self(value)
    }
}
impl From<AngleDeg> for f64 {
    fn from(value: AngleDeg) -> Self {
        value.0
    }
}
impl PartialEq for AngleDeg {
    fn eq(&self, other: &Self) -> bool {
        if (self.0 == 0.0 && other.0 == 360.0) || (self.0 == 360.0 && other.0 == 0.0) {
            true
        } else {
            self.0 == other.0
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

        let generators: Vec<(Perlin, f64, f64)> = (0..terrain_settings.noise_passes)
            .map(|pass| {
                (
                    Perlin::new(seed_generator.next_u32()),
                    1f64 / (2f64.powi(pass as i32)),
                    2f64.powi(pass as i32),
                )
            })
            .collect();
        let cave_generator = (OpenSimplex::new(seed_generator.next_u32()), 32.0);

        c_debug!(to_console, "generators: {:?}", generators);
        c_debug!(to_console, "cave generator: {:?}", cave_generator);

        let mut height_map = vec![terrain_settings.base_height; world.width as usize];
        height_map.iter_mut().enumerate().for_each(|(idx, height)| {
            let x = idx as f64 * 0.005;
            let mut multiplier = 0.0;
            let mut octaves = 0.0;
            for (generator, octave, freq) in &generators {
                let generated = (generator.get([x * freq, *height as f64]) / 2.0) + 0.5;
                // from [-1, 1] to [0, 1]
                multiplier += octave * generated;
                octaves += octave;
            }
            multiplier /= octaves;
            multiplier = multiplier.powf(terrain_settings.redistribution_factor);
            *height += (multiplier * height_range).round() as u32;
        });

        let (cave, freq) = cave_generator;
        for (x, &height) in height_map.iter().enumerate() {
            if height != 0 {
                for y in 0..height {
                    let block = {
                        let noise_here = cave
                            .get([x as f64 * 0.001 * freq, y as f64 * 0.001 * freq])
                            .abs();
                        if noise_here < cave_gen_size {
                            Block::Air
                        } else {
                            Block::Stone
                        }
                    };
                    world.set_block(x as u32, y, block)?;
                }
            }
            if height < terrain_settings.water_height {
                for y in height..terrain_settings.water_height {
                    world.set_block(x as u32, y, Block::Water)?;
                }
            }
        }

        for x in 0..world.width {
            let (_, top_block_y) = world.get_highest_block_at(x)?;
            let above_top_block = world.get_block(x, top_block_y + 1)?;
            let below_top_block = world.get_block(x, top_block_y.saturating_sub(1))?;
            if below_top_block == Block::Air {
                world.set_block(x, top_block_y, Block::Air)?;
            } else if above_top_block != Block::Water && top_block_y > terrain_settings.water_height
            {
                world.set_block(x, top_block_y, Block::Grass)?;
            }
        }

        c_info!(
            to_console,
            "Generation of terrain with seed {} took {:?}.",
            master_seed,
            start.elapsed()
        );
        Ok(world)
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

    fn get_water_neighbours(x: u32, y: u32) -> [(u32, u32); 3] {
        [
            (x, y.saturating_sub(1)),
            (x.saturating_sub(1), y),
            (x + 1, y),
        ]
    }

    pub fn set_block(&mut self, pos_x: u32, pos_y: u32, block: Block) -> Result<(), WorldError> {
        self.raw_set_block(pos_x, pos_y, block)?;
        // update block
        if block == Block::Water {
            let neighbours = World::get_water_neighbours(pos_x, pos_y);
            for (x, y) in neighbours {
                if let Ok(bl) = self.get_block(x, y) {
                    if !is_solid(bl) && bl != Block::Water {
                        self.to_update.insert((x, y, Block::Water));
                    }
                }
            }
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
        to_console: ToConsole,
        socket: &UdpSocket,
        pos_x: u32,
        pos_y: u32,
        block: Block,
    ) -> Result<(), WorldError> {
        self.set_block(pos_x, pos_y, block)?;
        let (chunk_x, chunk_y) = self.get_chunk_block_is_in(pos_x, pos_y)?;
        let players_loading = self.get_list_of_players_loading_chunk(chunk_x, chunk_y)?;
        let response = ServerUpdateBlock {
            block: block.into(),
            x: pos_x,
            y: pos_y,
        };

        for player in players_loading {
            encode_and_send!(
                to_console,
                PacketTypes::ServerUpdateBlock,
                response.clone(),
                socket,
                player.addr
            );
        }

        Ok(())
    }

    pub async fn shutdown(&mut self, to_console: ToConsole, socket: &UdpSocket) -> io::Result<()> {
        c_info!(to_console, "Shutting down Server!");
        let kick_msg = String::from("Server Shutting Down!");
        self.player_loaded
            .par_iter_mut()
            .for_each(|chunk| chunk.clear());

        let kick = ServerKick { msg: kick_msg };
        for player in &mut self.players {
            encode_and_send!(
                to_console,
                PacketTypes::ServerKick,
                kick.clone(),
                socket,
                player.addr
            );
        }
        self.players.clear();
        Ok(())
    }

    pub async fn kick(
        &mut self,
        to_console: ToConsole,
        socket: &UdpSocket,
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

                let to_broadcast = ServerPlayerLeave {
                    player_name: connection.name.clone(),
                    player_id: connection.id,
                };
                let to_broadcast_chunk = ServerPlayerLeaveLoaded {
                    player_name: connection.name.clone(),
                    player_id: connection.id,
                };

                let kick = ServerKick {
                    msg: kick_msg.into(),
                };
                encode_and_send!(
                    to_console,
                    PacketTypes::ServerKick,
                    kick,
                    socket,
                    connection.addr
                );

                for player in self.players.iter() {
                    if players_loading_chunk.contains(&player) {
                        encode_and_send!(
                            to_console,
                            PacketTypes::ServerPlayerLeaveLoaded,
                            to_broadcast_chunk.clone(),
                            socket,
                            player.addr
                        );
                    }
                    encode_and_send!(
                        to_console,
                        PacketTypes::ServerPlayerLeave,
                        to_broadcast.clone(),
                        socket,
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

    async fn tick_water(
        &mut self,
        to_console: ToConsole,
        socket: &UdpSocket,
    ) -> Result<(), WorldError> {
        let water_to_update: HashSet<&(u32, u32, Block)> = self
            .to_update
            .par_iter()
            .filter(|pos| pos.2 == Block::Water)
            .collect();

        let to_update: HashSet<(u32, u32)> = water_to_update
            .par_iter()
            .flat_map(|(x, y, _)| World::get_water_neighbours(*x, *y))
            .filter_map(|(bl_pos_x, bl_pos_y)| {
                if let Ok(bl) = self.get_block(bl_pos_x, bl_pos_y) {
                    if !is_solid(bl) && bl != Block::Water {
                        return Some((bl_pos_x, bl_pos_y));
                    }
                }
                None
            })
            .collect();
        self.to_update.retain(|pos| pos.2 != Block::Water);
        for (x, y) in to_update {
            self.set_block_and_notify(to_console.clone(), socket, x, y, Block::Water)
                .await?;
        }
        Ok(())
    }

    pub fn get_neighbours_of_player(&self, player: &Player) -> [BlockPos; 6] {
        macro_rules! get_or_air {
            ($world: expr, $x: expr, $y: expr) => {
                match $world.get_block($x, $y) {
                    Ok(bl) => bl,
                    Err(_) => Block::Air,
                }
            };
        }
        let (grid_x, grid_y) = (player.x.round() as u32, player.y.round() as u32);
        let (hitbox_width, hitbox_height) = (player.hitbox_width, player.hitbox_height);

        let positions = [
            (grid_x, grid_y.wrapping_sub(1)),
            (grid_x, grid_y + 1),
            (grid_x.wrapping_sub(1), grid_y + (hitbox_height / 2)),
            (grid_x.wrapping_sub(1), grid_y),
            (grid_x + hitbox_width, grid_y + (hitbox_height / 2)),
            (grid_x + hitbox_width, grid_y),
        ];

        let block_pos_vec: Vec<BlockPos> = positions
            .iter()
            .map(|&(x, y)| {
                let bl = get_or_air!(self, x, y);
                (x, y, bl)
            })
            .collect();
        block_pos_vec.try_into().unwrap()
    }

    pub async fn tick(
        &mut self,
        to_console: ToConsole,
        socket: &UdpSocket,
    ) -> io::Result<Duration> {
        let now = Instant::now();

        if let Err(e) = self.tick_water(to_console.clone(), socket).await {
            c_error!(to_console, "Error occurred while ticking water: {e}")
        };

        //collision
        {
            let surrounding: Vec<[BlockPos; 6]> = self
                .players
                .par_iter()
                .map(|conn| self.get_neighbours_of_player(&conn.server_player))
                .collect();
            let player_surrounding: Vec<(&ClientConnection, [BlockPos; 6])> =
                zip(&self.players, surrounding).collect();

            let res: Vec<(ClientConnection, bool, (f32, f32))> = player_surrounding
                .par_iter()
                .map(|&(conn, surr)| {
                    let mut new_player = conn.server_player.clone();
                    let old_pos = (new_player.x, new_player.y);
                    let (has_changed_fall, has_changed_collision);
                    (new_player, has_changed_fall) = new_player.do_fall(surr);
                    (new_player, has_changed_collision) = new_player.do_collision(surr);
                    (
                        ClientConnection::with(conn, new_player),
                        has_changed_collision | has_changed_fall,
                        old_pos,
                    )
                })
                .collect();

            let mut new_players = vec![];
            for (new_player, update_pos, (old_x, old_y)) in res {
                if update_pos {
                    let (old_chunk_x, old_chunk_y) = self
                        .get_chunk_block_is_in(old_x.round() as u32, old_y.round() as u32)
                        .unwrap_or((0, 0));
                    let (chunk_x, chunk_y) = self
                        .get_chunk_block_is_in(
                            new_player.server_player.x.round() as u32,
                            new_player.server_player.y.round() as u32,
                        )
                        .unwrap_or((0, 0));
                    let players_loading_old_chunk = self
                        .get_list_of_players_loading_chunk(old_chunk_x, old_chunk_y)
                        .unwrap_or_default();
                    let players_loading_chunk = self
                        .get_list_of_players_loading_chunk(chunk_x, chunk_y)
                        .unwrap_or_default();

                    let old_players: Vec<&ClientConnection> = players_loading_old_chunk
                        .clone()
                        .into_par_iter()
                        .filter(|conn| !players_loading_chunk.contains(conn))
                        .collect();
                    let new_players: Vec<&ClientConnection> = players_loading_chunk
                        .clone()
                        .into_par_iter()
                        .filter(|conn| !players_loading_old_chunk.contains(conn))
                        .collect();

                    for conn in old_players {
                        let leave_packet = ServerPlayerLeaveLoaded {
                            player_id: new_player.id,
                            player_name: new_player.name.clone(),
                        };
                        encode_and_send!(
                            to_console,
                            PacketTypes::ServerPlayerLeaveLoaded,
                            leave_packet,
                            socket,
                            conn.addr
                        );
                    }
                    let move_packet = ServerPlayerUpdatePos {
                        player_id: new_player.id,
                        pos_x: new_player.server_player.x,
                        pos_y: new_player.server_player.y,
                    };
                    for conn in players_loading_chunk {
                        if new_players.contains(&conn) {
                            let enter_packet = ServerPlayerEnterLoaded {
                                player_id: new_player.id,
                                player_name: new_player.name.clone(),
                                pos_x: new_player.server_player.x,
                                pos_y: new_player.server_player.y,
                            };
                            encode_and_send!(
                                to_console,
                                PacketTypes::ServerPlayerEnterLoaded,
                                enter_packet,
                                socket,
                                conn.addr
                            );
                        }
                        encode_and_send!(
                            to_console,
                            PacketTypes::ServerPlayerUpdatePos,
                            move_packet.clone(),
                            socket,
                            conn.addr
                        );
                    }
                    encode_and_send!(
                        to_console,
                        PacketTypes::ServerPlayerUpdatePos,
                        move_packet,
                        socket,
                        new_player.addr
                    );
                }
                new_players.push(new_player);
            }
            self.players = new_players;
        }

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
        //debug!(
        //    "[Chunk at ({}, {})] Set block index {} to {:?}",
        //    self.chunk_x, self.chunk_y, idx, block
        //);
        self
    }

    fn get_block(&self, chunk_pos_x: u32, chunk_pos_y: u32) -> Block {
        self.blocks[(chunk_pos_y * self.size + chunk_pos_x) as usize]
    }
}

define_blocks! {
    Air = (0, false),
    Grass = (1, true),
    Stone = (2, true),
    Log = (3, true),
    Leaves = (4, true),
    Water = (5, false),
    Wood = (6, true)
}
