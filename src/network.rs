use serde::{Deserialize, Serialize};
use crate::world::{Block, Chunk};

#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    pub t: u8,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

// https://chatgpt.com/share/67910f66-8c24-8006-bf28-7bc00ff905ed
macro_rules! define_packets {
    (
        $(
            $name:ident = $value:expr => $struct:ident {
                $($field_name:ident: $field_type:ty),* $(,)?
            }
        ),* $(,)?
    ) => {
        #[derive(Serialize, Deserialize, Debug)]
        #[repr(u8)]
        pub enum PacketTypes {
            Invalid = 0,
            $($name = $value),*
        }

        impl Into<u8> for PacketTypes {
            fn into(self) -> u8 {
                self as u8
            }
        }

        impl Into<PacketTypes> for u8 {
            fn into(self) -> PacketTypes {
                match self {
                    $($value => PacketTypes::$name),*,
                    _ => PacketTypes::Invalid,
                }
            }
        }

        $(
            #[derive(Serialize, Deserialize, Debug)]
            pub struct $struct {
                $($field_name: $field_type),*
            }
        )*
    };
}

// Use the macro to define packets
define_packets!(
    ClientHello = 1 => ClientHello {
        name: String
    },
    ServerSync = 2 => ServerSync {
        player_id: i32,
        world_width: i32,
        world_height: i32,
        chunk_size: i32
    },
    ClientRequestChunk = 3 => ClientRequestChunk {
        chunk_coords_x: i32,
        chunk_coords_y: i32,
    },
    ServerChunkResponse = 4 => ServerChunkResponse {
        chunk: Chunk,
    },
    ClientUnloadChunk = 5 => ClientUnloadChunk {
        chunk_coords_x: i32,
        chunk_coords_y: i32,
    },
    ServerPlayerJoin = 6 => ServerPlayerJoin {
        player_name: String,
        player_id: i32
    },
    ServerPlayerEnterLoaded = 7 => ServerPlayerEnterLoaded {
        player_name: String,
        player_id: i32
    },
    ServerPlayerLeaveLoaded = 8 => ServerPlayerLeaveLoaded {
        player_name: String,
        player_id: i32
    },
    ServerPlayerLeave = 9 => ServerPlayerLeave {
        player_name: String,
        player_id: i32
    },
    ClientGoodbye = 10 => ClientGoodbye {},
    ClientPlaceBlock = 11 => ClientPlaceBlock {
        block: Block,
        x: u32,
        y: u32
    },
    ServerUpdateBlock = 12 => ServerUpdateBlock {
        block: Block,
        x: u32,
        y: u32
    },
    ClientPlayerMoveX = 13 => ClientPlayerMoveX {
        pos_x: f32
    },
    ClientPlayerJump = 14 => ClientPlayerJump {},
    ServerPlayerUpdatePos = 15 => ServerPlayerUpdatePos {
        player_id: i32,
        pos_x: f32,
        pos_y: f32
    }
);
