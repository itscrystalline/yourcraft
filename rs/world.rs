use crate::network::{
    ClientConnection, PacketTypes, ServerKick, ServerPlayerUpdatePos, ServerUpdateBlock,
};
use crate::network::{Packet, ServerPlayerLeave, ServerPlayerLeaveLoaded};
use crate::player::Player;
use get_size::GetSize;
use log::{debug, error, info};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;
use std::iter::zip;
use std::time::Instant;
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
    #[error("player interaction outside loaded chunk")]
    PlaceOutOfLoadedChunk,
    #[error("chunk is already loaded")]
    ChunkAlreadyLoaded,
    #[error("chunk is already unloaded")]
    ChunkAlreadyUnloaded,
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
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
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

impl World {
    pub fn generate_empty(width: u32, height: u32, chunk_size: u32) -> Result<World, WorldError> {
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

            info!(
                "Generated {} chunks in {:?}",
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
            })
        }
    }

    pub fn generate_flat(
        width: u32,
        height: u32,
        chunk_size: u32,
        grass_level: u32,
    ) -> Result<World, WorldError> {
        let mut empty_world = World::generate_empty(width, height, chunk_size)?;

        let start = Instant::now();

        if grass_level != 0 {
            for idx in 0..width * grass_level {
                let x = idx % width;
                let y = idx / width;
                empty_world.set_block(x, y, Block::Stone)?
            }
        }
        for x in 0..width {
            empty_world.set_block(x, grass_level, Block::Grass)?
        }

        info!(
            "filled {} * {} area with grass and stone in {:?}",
            width,
            grass_level,
            start.elapsed()
        );
        Ok(empty_world)
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
            &mut self.player_loaded[(chunk_y * self.height_chunks + chunk_x) as usize];
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
            &mut self.player_loaded[(chunk_y * self.height_chunks + chunk_x) as usize];
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
            &self.player_loaded[(chunk_y * self.height_chunks + chunk_x) as usize];
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
        debug!("Found chunk at {}, {}", chunk_x, chunk_y);
        chunk.set_block(pos_inside_chunk_x, pos_inside_chunk_y, block);
        Ok(())
    }

    fn get_water_neighbours(x: u32, y: u32) -> [(u32, u32); 3] {
        [(x, y.wrapping_sub(1)), (x.wrapping_sub(1), y), (x + 1, y)]
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
                PacketTypes::ServerUpdateBlock,
                response.clone(),
                socket,
                player.addr
            );
        }

        Ok(())
    }

    pub async fn shutdown(&mut self, socket: &UdpSocket) -> io::Result<()> {
        info!("Shutting down Server!");
        let kick_msg = String::from("Server Shutting Down!");
        self.player_loaded
            .par_iter_mut()
            .for_each(|chunk| chunk.clear());

        let kick = ServerKick { msg: kick_msg };
        for player in &mut self.players {
            encode_and_send!(PacketTypes::ServerKick, kick.clone(), socket, player.addr);
        }
        self.players.clear();
        Ok(())
    }

    pub async fn kick(&mut self, socket: &UdpSocket, id: u32, msg: Option<&str>) -> io::Result<()> {
        match self.players.iter().position(|x| x.id == id) {
            None => error!("Kicking player that hasn't joined! ({})", id),
            Some(idx) => {
                let connection = self.players.swap_remove(idx);
                let kick_msg = msg.unwrap_or("No kick message provided");
                self.unload_all_for(connection.id);
                info!(
                    "{} (addr: {}) kicked from sever! ({})",
                    connection.name, connection.addr, kick_msg
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
                encode_and_send!(PacketTypes::ServerKick, kick, socket, connection.addr);

                for player in self.players.iter() {
                    encode_and_send!(
                        PacketTypes::ServerPlayerLeave,
                        to_broadcast.clone(),
                        socket,
                        player.addr
                    );
                    if players_loading_chunk.contains(&player) {
                        encode_and_send!(
                            PacketTypes::ServerPlayerLeaveLoaded,
                            to_broadcast_chunk.clone(),
                            socket,
                            player.addr
                        );
                    }
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
        let slice: Result<Vec<Block>, WorldError> =
            y.par_iter().map(|y| self.get_block(x, *y)).collect();
        debug!("world slice at x: {x}, {slice:#?}");
        let top_block_window = y.par_windows(2).find_last(|window| {
            let block_next = self.get_block(x, window[1]);
            let block_prev = self.get_block(x, window[0]);

            if let (Ok(block_next), Ok(block_prev)) = (block_next, block_prev) {
                block_next == Block::Air && block_prev != Block::Air
            } else {
                false
            }
        });
        Ok(match top_block_window {
            Some(window) => (x, window[0]),
            None => (x, 0),
        })
    }

    fn get_all_blockpos_of_type(&self, block_type: Block) -> Vec<(u32, u32)> {
        self.chunks
            .par_iter()
            .filter_map(|chunk| {
                let hits: Vec<(u32, u32)> = chunk
                    .blocks
                    .par_iter()
                    .enumerate()
                    .filter_map(|(idx, bl)| {
                        if *bl == block_type {
                            let (local_x, local_y) = chunk.pos_from_index(idx);
                            let global_pos = (
                                chunk.chunk_x * self.chunk_size + local_x,
                                chunk.chunk_y * self.chunk_size + local_y,
                            );
                            Some(global_pos)
                        } else {
                            None
                        }
                    })
                    .collect();
                Some(hits)
            })
            .flatten()
            .collect()
    }

    async fn tick_water(&mut self, socket: &UdpSocket) -> Result<(), WorldError> {
        let water_to_update: HashSet<&(u32, u32, Block)> = self
            .to_update
            .iter()
            .filter(|pos| pos.2 == Block::Water)
            .collect();
        let mut to_update: HashSet<(u32, u32)> = HashSet::new();
        for (x, y, _) in water_to_update.iter() {
            let neighbours = World::get_water_neighbours(*x, *y);
            for (bl_pos_x, bl_pos_y) in neighbours {
                if let Ok(bl) = self.get_block(bl_pos_x, bl_pos_y) {
                    if !is_solid(bl) && bl != Block::Water {
                        to_update.insert((bl_pos_x, bl_pos_y));
                    }
                }
            }
        }
        self.to_update.retain(|pos| pos.2 != Block::Water);
        for (x, y) in to_update {
            self.set_block_and_notify(socket, x, y, Block::Water)
                .await?;
        }
        Ok(())
    }

    fn get_neighbours_of_player(&self, player: &Player) -> [(u32, u32, Block); 6] {
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

        let block_pos_vec: Vec<(u32, u32, Block)> = positions
            .iter()
            .map(|&(x, y)| {
                let bl = get_or_air!(self, x, y);
                (x, y, bl)
            })
            .collect();
        block_pos_vec.try_into().unwrap()
    }

    pub async fn tick(&mut self, socket: &UdpSocket) -> io::Result<()> {
        let now = Instant::now();

        if let Err(e) = self.tick_water(socket).await {
            error!("Error occurred while ticking water: {e}")
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

            let res: Vec<(ClientConnection, bool)> = player_surrounding
                .par_iter()
                .map(|&(conn, surr)| {
                    let mut new_player = conn.server_player.clone();
                    let (has_changed_fall, has_changed_collision);
                    (new_player, has_changed_fall) = new_player.do_fall(surr);
                    (new_player, has_changed_collision) = new_player.do_collision(surr);
                    (
                        ClientConnection::with(conn, new_player),
                        has_changed_collision | has_changed_fall,
                    )
                })
                .collect();

            let mut new_players = vec![];
            for (conn, update_pos) in res {
                if update_pos {
                    let packet = ServerPlayerUpdatePos {
                        player_id: conn.id,
                        pos_x: conn.server_player.x,
                        pos_y: conn.server_player.y,
                    };
                    let (chunk_x, chunk_y) = self
                        .get_chunk_block_is_in(
                            conn.server_player.x.round() as u32,
                            conn.server_player.y.round() as u32,
                        )
                        .unwrap_or((0, 0));
                    let players_loading_chunk = self
                        .get_list_of_players_loading_chunk(chunk_x, chunk_y)
                        .unwrap_or_default();
                    for conn in players_loading_chunk {
                        encode_and_send!(
                            PacketTypes::ServerPlayerUpdatePos,
                            packet.clone(),
                            socket,
                            conn.addr
                        );
                    }
                }
                new_players.push(conn);
            }
            self.players = new_players;
        }

        debug!("tick took {:?}.", now.elapsed());
        Ok(())
    }
}

impl Chunk {
    fn empty(size: u32, chunk_x: u32, chunk_y: u32) -> Chunk {
        Chunk {
            size,
            chunk_x,
            chunk_y,
            blocks: (0..size.pow(2)).map(|_| Block::Air).collect(),
        }
    }

    fn set_block(&mut self, chunk_pos_x: u32, chunk_pos_y: u32, block: Block) -> &mut Self {
        let idx = (chunk_pos_y * self.size + chunk_pos_x) as usize;
        self.blocks[idx] = block;
        debug!(
            "[Chunk at ({}, {})] Set block index {} to {:?}",
            self.chunk_x, self.chunk_y, idx, block
        );
        self
    }

    fn get_block(&self, chunk_pos_x: u32, chunk_pos_y: u32) -> Block {
        self.blocks[(chunk_pos_y * self.size + chunk_pos_x) as usize]
    }

    fn pos_from_index(&self, idx: usize) -> (u32, u32) {
        let idx_u32 = idx as u32;
        (idx_u32 % self.size, idx as u32 / self.size)
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
