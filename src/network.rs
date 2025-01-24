use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use rand::prelude::*;
use serde_pickle::{to_vec, SerOptions};
use crate::world::{Block, Chunk};

#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    pub t: u8,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

impl Packet {
    pub fn encode<T: Serialize>(t: PacketTypes, packet: T) -> serde_pickle::Result<Vec<u8>> {
        let packet = Packet {
            t: t.into(),
            data: to_vec(&packet, SerOptions::new())?,
        };
        to_vec(&packet, SerOptions::new())
    }
}

pub struct ClientConnection {
    pub addr: SocketAddr,
    pub name: String,
    pub id: u32,
    pub loaded_chunks: Vec<(u32, u32)>,
}

impl ClientConnection {
    pub fn new(addr: SocketAddr, name: String) -> ClientConnection {
        let mut rng = rand::rng();
        ClientConnection {
            addr,
            name,
            id: rng.next_u32(),
            loaded_chunks: Vec::new(),
        }
    }
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
                $(pub $field_name: $field_type),*
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
        player_id: u32,
        world_width: u32,
        world_height: u32,
        chunk_size: u32
    },
    ClientRequestChunk = 3 => ClientRequestChunk {
        chunk_coords_x: u32,
        chunk_coords_y: u32,
    },
    ServerChunkResponse = 4 => ServerChunkResponse {
        chunk: Chunk,
    },
    ClientUnloadChunk = 5 => ClientUnloadChunk {
        chunk_coords_x: u32,
        chunk_coords_y: u32,
    },
    ServerPlayerJoin = 6 => ServerPlayerJoin {
        player_name: String,
        player_id: u32
    },
    ServerPlayerEnterLoaded = 7 => ServerPlayerEnterLoaded {
        player_name: String,
        player_id: u32
    },
    ServerPlayerLeaveLoaded = 8 => ServerPlayerLeaveLoaded {
        player_name: String,
        player_id: u32
    },
    ServerPlayerLeave = 9 => ServerPlayerLeave {
        player_name: String,
        player_id: u32
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
        player_id: u32,
        pos_x: f32,
        pos_y: f32
    }
);
