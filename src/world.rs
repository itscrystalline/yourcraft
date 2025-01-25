use std::io::Error;
use crate::network::ClientConnection;
use log::{debug, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum WorldError {
    MismatchedChunkSize,
    OutOfBounds(u32, u32),
    PlaceOutOfLoadedChunk,
    ChunkAlreadyLoaded,
    ChunkAlreadyUnloaded,
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chunk {
    pub size: u32,
    pub chunk_x: u32,
    pub chunk_y: u32,
    pub blocks: Vec<Block>,
}

macro_rules! define_blocks {
    ($($name:ident = $id:expr),* $(,)?) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub enum Block {
            $($name = $id),*
        }

        impl Into<Block> for u8 {
            fn into(self) -> Block {
                match self {
                    $($id => Block::$name),*,
                    _ => Block::Air,
                }
            }
        }

        impl Into<u8> for Block {
            fn into(self) -> u8 {self as u8
            }
        }
    };
}

impl World {
    pub fn generate_empty(width: u32, height: u32, chunk_size: u32) -> Result<World, WorldError> {
        if width % chunk_size != 0 && height % chunk_size != 0 {
            Err(WorldError::MismatchedChunkSize)
        } else {
            let width_chunks = width / chunk_size;
            let height_chunks = height / chunk_size;
            let (chunks, player_loaded) = (0..width_chunks * height_chunks)
                .map(|idx| {
                    let chunk_x = idx % width_chunks;
                    let chunk_y = idx / width_chunks;
                    (Chunk::empty(chunk_size, chunk_x, chunk_y), vec![])
                })
                .collect();

            Ok(World {
                width,
                height,
                chunk_size,
                chunks,
                width_chunks,
                height_chunks,
                players: vec![],
                player_loaded,
            })
        }
    }

    fn check_out_of_bounds_chunk(&self, chunk_x: u32, chunk_y: u32) -> Result<(), WorldError> {
        if chunk_x > self.width / self.chunk_size || chunk_y > self.height / self.chunk_size {
            Err(WorldError::OutOfBounds(chunk_x, chunk_y))
        } else {
            Ok(())
        }
    }
    fn check_out_of_bounds_block(&self, x: u32, y: u32) -> Result<(), WorldError> {
        if x >= self.width && y >= self.height {
            Err(WorldError::OutOfBounds(x, y))
        } else {
            Ok(())
        }
    }

    pub fn get_chunk_mut(&mut self, chunk_x: u32, chunk_y: u32) -> Result<&mut Chunk, WorldError> {
        self.check_out_of_bounds_chunk(chunk_x, chunk_y)?;
        Ok(&mut self.chunks[(chunk_y * self.height_chunks + chunk_x) as usize])
    }

    pub fn get_chunk(&self, chunk_x: u32, chunk_y: u32) -> Result<&Chunk, WorldError> {
        self.check_out_of_bounds_chunk(chunk_x, chunk_y)?;
        Ok(&self.chunks[(chunk_y * self.height_chunks + chunk_x) as usize])
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
                if let Some(_) = self
                    .players
                    .iter()
                    .find(|&player| player.id == player_loading_id)
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
        for players_loading_chunk in self.player_loaded.iter_mut() {
            players_loading_chunk.retain(|&con| player_loading_id != con);
        }
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

    pub fn set_block(&mut self, pos_x: u32, pos_y: u32, block: Block) -> Result<(), WorldError> {
        self.check_out_of_bounds_block(pos_x, pos_y)?;

        let (chunk_x, chunk_y) = self.get_chunk_block_is_in(pos_x, pos_y)?;
        let pos_inside_chunk_x = pos_x - chunk_x * self.chunk_size;
        let pos_inside_chunk_y = pos_y - chunk_y * self.chunk_size;

        let chunk = self.get_chunk_mut(chunk_x, chunk_y)?;
        debug!("Found chunk: {:?}", chunk);
        chunk.set_block(pos_inside_chunk_x, pos_inside_chunk_y, block);
        Ok(())
    }

    pub fn get_chunk_block_is_in(&self, pos_x: u32, pos_y: u32) -> Result<(u32, u32), WorldError> {
        self.check_out_of_bounds_block(pos_x, pos_y)?;
        let chunk_x = pos_x / self.chunk_size;
        let chunk_y = pos_y / self.chunk_size;
        Ok((chunk_x, chunk_y))
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
}

define_blocks! {
    Air = 0,
    Grass = 1,
    Stone = 2,
    Log = 3,
    Leaves = 4,
    Water = 5,
    Wood = 6,
}
