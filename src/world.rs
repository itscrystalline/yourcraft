#[derive(Debug, Clone)]
pub enum WorldError {
    MismatchedChunkSize,
    GroundLevelOverWorldHeight,
    OutOfBounds(u32, u32),
}

#[derive(Debug)]
pub struct World {
    pub width: u32,
    pub height: u32,
    pub chunk_size: u32,
    width_chunks: u32,
    height_chunks: u32,
    pub chunks: Vec<Chunk>
}

#[derive(Debug)]
pub struct Chunk {
    pub size: u32,
    pub chunk_x: u32,
    pub chunk_y: u32,
    pub blocks: Vec<Block>
}

macro_rules! define_blocks {
    ($($name:ident = $id:expr),* $(,)?) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
            fn into(self) -> u8 {
                self as u8
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
            let chunks = (0..width_chunks * height_chunks).map(|idx| {
                let chunk_x = idx % width_chunks;
                let chunk_y = idx / width_chunks;
                Chunk::empty(chunk_size, chunk_x, chunk_y)
            }).collect();

            Ok(World {
                width,
                height,
                chunk_size,
                width_chunks,
                height_chunks,
                chunks
            })
        }
    }

    pub fn get_chunk_mut(&mut self, chunk_x: u32, chunk_y: u32) -> Result<&mut Chunk, WorldError> {
        if (chunk_x > self.width / self.chunk_size || chunk_y > self.height / self.chunk_size) {
            return Err(WorldError::OutOfBounds(chunk_x, chunk_y))
        }
        Ok(&mut self.chunks[(chunk_y * self.height_chunks + chunk_x) as usize])
    }

    pub fn set_block(&mut self, pos_x: u32, pos_y: u32, block: Block) -> Result<(), WorldError> {
        let chunk_x = pos_x / self.chunk_size;
        let chunk_y = pos_y / self.chunk_size;
        let pos_inside_chunk_x = pos_x - chunk_x * self.chunk_size;
        let pos_inside_chunk_y = pos_y - chunk_y * self.chunk_size;

        let chunk = self.get_chunk_mut(chunk_x, chunk_y)?;
        chunk.set_block(pos_inside_chunk_x, pos_inside_chunk_y, block);
        Ok(())
    }
}

impl Chunk {
    fn empty(size: u32, chunk_x: u32, chunk_y: u32) -> Chunk {
        Chunk {
            size, chunk_x, chunk_y,
            blocks: (0..size.pow(2)).map(|_| Block::Air).collect(),
        }
    }

    fn set_block(&mut self, chunk_pos_x: u32, chunk_pos_y: u32, block: Block) -> &mut Self {
        self.blocks[(chunk_pos_y * self.size + chunk_pos_x) as usize] = block;
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