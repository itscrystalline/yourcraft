use crate::player::Player;
use crate::world::{Chunk, World, WorldError};
use log::{error, info, warn};
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use serde_pickle::{from_slice, to_vec, DeOptions, SerOptions};
use std::io;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

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

#[derive(Debug, PartialEq)]
pub struct ClientConnection {
    pub addr: SocketAddr,
    pub name: String,
    pub id: u32,
    pub server_player: Player,
}

impl ClientConnection {
    pub fn new(addr: SocketAddr, name: String) -> ClientConnection {
        let mut rng = rand::rng();
        ClientConnection {
            addr,
            name,
            server_player: Player::spawn_at_origin(),
            id: rng.next_u32(),
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
            #[derive(Serialize, Deserialize, Debug, Clone)]
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
        block: u8,
        x: u32,
        y: u32
    },
    ServerUpdateBlock = 12 => ServerUpdateBlock {
        block: u8,
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

macro_rules! unwrap_packet_or_ignore {
    ($packet: expr) => {
        match from_slice(&$packet.data, DeOptions::new()) {
            Ok(packet) => packet,
            Err(_) => {
                error!("Received differing packet content from what type of packet suggests ({})! ignoring.", $packet.t);
                return Ok(());
            }
        }
    };
}

#[macro_export]
macro_rules! encode_and_send {
    ($packet_type: expr, $packet: expr, $socket: expr, $addr: expr) => {
        let encoded = Packet::encode($packet_type, $packet).unwrap();
        $socket.send_to(&encoded, $addr).await?;
    };
}

pub async fn incoming_packet_handler(
    socket: &UdpSocket,
    buf: &mut [u8],
    world: &mut World,
) -> io::Result<()> {
    let (len, client_addr) = socket.recv_from(buf).await?;
    info!("{:?} bytes received from {:?}", len, client_addr);

    let packet: serde_pickle::Result<Packet> = from_slice(&buf[..len], DeOptions::new());
    match packet {
        Ok(packet) => {
            process_client_packet(socket, packet, client_addr, world).await?;
        }
        Err(e) => {
            warn!(
                "Recieved unknown packet from {}, ignoring! (Err: {:?})",
                client_addr, e
            );
        }
    }

    Ok(())
}
async fn process_client_packet(
    socket: &UdpSocket,
    packet: Packet,
    addr: SocketAddr,
    world: &mut World,
) -> io::Result<()> {
    match packet.t.into() {
        PacketTypes::ClientHello => {
            let hello_packet: ClientHello = unwrap_packet_or_ignore!(packet);
            info!("{} joined the server!", hello_packet.name);
            let connection = ClientConnection::new(addr, hello_packet.name);

            let response = ServerSync {
                player_id: connection.id,
                world_width: world.width,
                world_height: world.height,
                chunk_size: world.chunk_size,
            };

            encode_and_send!(PacketTypes::ServerSync, response, socket, addr);
            world.players.push(connection);
        }
        PacketTypes::ClientGoodbye => {
            match world.players.iter().position(|x| x.addr == addr) {
                None => error!("Goodbye packet from address that hasn't joined! ({})", addr),
                Some(idx) => {
                    let connection = world.players.swap_remove(idx);
                    world.unload_all_for(connection.id);
                    info!(
                        "{} (addr: {}) left the server!",
                        connection.name, connection.addr
                    );
                }
            };
        }
        PacketTypes::ClientPlaceBlock => {
            let place_block_packet: ClientPlaceBlock = unwrap_packet_or_ignore!(packet);
            match world
                .set_block_and_notify(
                    socket,
                    place_block_packet.x,
                    place_block_packet.y,
                    place_block_packet.block.into(),
                )
                .await
            {
                Ok(_) => (),
                Err(e) => match e {
                    WorldError::NetworkError(e) => error!("error while notifying clients: {e}"),
                    _ => error!("error while placing block: {e}"),
                },
            };
        }
        PacketTypes::ClientPlayerJump => {
            todo!()
        }
        PacketTypes::ClientPlayerMoveX => {
            todo!()
        }
        PacketTypes::ClientRequestChunk => match world.players.iter().find(|x| x.addr == addr) {
            None => error!("addr hasn't joined! ({})", addr),
            Some(player_conn) => {
                let request_packet: ClientRequestChunk = unwrap_packet_or_ignore!(packet);
                match world.mark_chunk_loaded_by_id(
                    request_packet.chunk_coords_x,
                    request_packet.chunk_coords_y,
                    player_conn.id,
                ) {
                    Ok(chunk) => {
                        let response = ServerChunkResponse {
                            chunk: chunk.clone(),
                        };
                        encode_and_send!(PacketTypes::ServerChunkResponse, response, socket, addr);
                    }
                    Err(err) => {
                        error!("error marking chunk as loaded! {:?}", err);
                    }
                };
            }
        },
        PacketTypes::ClientUnloadChunk => match world.players.iter().find(|x| x.addr == addr) {
            None => error!("addr hasn't joined! ({})", addr),
            Some(player_conn) => {
                let request_packet: ClientUnloadChunk = unwrap_packet_or_ignore!(packet);
                match world.unmark_loaded_chunk_for(
                    request_packet.chunk_coords_x,
                    request_packet.chunk_coords_y,
                    player_conn.id,
                ) {
                    Ok(_) => (),
                    Err(err) => {
                        error!("error marking chunk as unloaded! {:?}", err);
                    }
                };
            }
        },
        _ => {
            error!(
                "server received unknown or client bound packet: {:?}",
                packet
            );
        }
    }

    Ok(())
}
