use crate::console::ToConsole;
use crate::player::Player;
use crate::world::{Chunk, World, WorldError};
use crate::{c_debug, c_error, c_info, c_warn};
use get_size::GetSize;
use rand::prelude::*;
use rayon::prelude::*;
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
    pub connection_alive: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkChunk {
    pub size: u32,
    pub chunk_x: u32,
    pub chunk_y: u32,
    pub blocks: Vec<u8>,
}

impl From<Chunk> for NetworkChunk {
    fn from(chunk: Chunk) -> Self {
        Self {
            size: chunk.size,
            chunk_x: chunk.chunk_x,
            chunk_y: chunk.chunk_y,
            blocks: chunk.blocks.par_iter().map(|&bl| bl.into()).collect(),
        }
    }
}

impl ClientConnection {
    pub fn with(old: &Self, new_player: Player) -> Self {
        ClientConnection {
            id: old.id,
            name: old.name.clone(),
            addr: old.addr,
            server_player: new_player,
            connection_alive: old.connection_alive,
        }
    }

    pub fn new_at(
        to_console: ToConsole,
        addr: SocketAddr,
        world: &World,
        x: u32,
        name: String,
    ) -> Result<ClientConnection, WorldError> {
        let mut rng = rand::rng();
        Ok(ClientConnection {
            addr,
            name,
            server_player: Player::spawn_at(to_console, world, x)?,
            id: rng.next_u32(),
            connection_alive: true,
        })
    }
}

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

        impl From<u8> for PacketTypes {
            fn from(id: u8) -> PacketTypes {
                match id {
                    $($value => PacketTypes::$name),*,
                    _ => PacketTypes::Invalid,
                }
            }
        }

        impl From<PacketTypes> for u8 {
            fn from(packet: PacketTypes) -> u8 {
                packet as u8
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
        chunk_size: u32,
        spawn_x: f32,
        spawn_y: f32,
    },
    ClientRequestChunk = 3 => ClientRequestChunk {
        chunk_coords_x: u32,
        chunk_coords_y: u32,
    },
    ServerChunkResponse = 4 => ServerChunkResponse {
        chunk: NetworkChunk,
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
        player_id: u32,
        pos_x: f32,
        pos_y: f32,
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
    },
    ServerKick = 16 => ServerKick {
        msg: String
    },
    ServerHeartbeat = 17 => ServerHeartbeat {},
    ClientHeartbeat = 18 => ClientHeartbeat {}
);

/// returns from the function early if packet fails to decode.
macro_rules! unwrap_packet_or_ignore {
    ($to_console: expr, $packet: expr) => {
        match from_slice(&$packet.data, DeOptions::new()) {
            Ok(packet) => packet,
            Err(err) => {
                c_error!($to_console, "Failed to deserialize packet: {}", err);
                c_error!($to_console, "Received differing packet content from what type of packet suggests ({:?})! ignoring.",
                                        PacketTypes::from($packet.t));
                return Ok(());
            }
        }
    };
}

#[macro_export]
macro_rules! encode_and_send {
    ($to_console: expr, $packet_type: expr, $packet: expr, $socket: expr, $addr: expr) => {
        let encoded = Packet::encode($packet_type, $packet).unwrap();
        c_debug!($to_console, "packet heap size: {}", encoded.get_heap_size());
        $socket.send_to(&encoded, $addr).await?;
    };
}

pub async fn incoming_packet_handler(
    to_console: ToConsole,
    socket: &UdpSocket,
    buf: &mut [u8],
    world: &mut World,
    recv: (usize, SocketAddr),
) -> io::Result<()> {
    let (len, client_addr) = recv;
    c_debug!(
        to_console,
        "{:?} bytes received from {:?}",
        len,
        client_addr
    );

    let packet: serde_pickle::Result<Packet> = from_slice(&buf[..len], DeOptions::new());
    match packet {
        Ok(packet) => {
            process_client_packet(to_console, socket, packet, client_addr, world).await?;
        }
        Err(e) => {
            c_warn!(
                to_console,
                "Recieved unknown packet from {}, ignoring! (Err: {:?})",
                client_addr,
                e
            );
        }
    }

    Ok(())
}

pub async fn heartbeat(
    to_console: ToConsole,
    socket: &UdpSocket,
    world: &mut World,
) -> io::Result<()> {
    // sends a heartbeat packet to all incoming players.
    let mut inactive: Vec<u32> = vec![];
    for player in world.players.iter_mut() {
        if player.connection_alive {
            encode_and_send!(
                to_console,
                PacketTypes::ServerHeartbeat,
                ServerHeartbeat {},
                socket,
                player.addr
            );
            player.connection_alive = false;
        } else {
            inactive.push(player.id);
        }
    }
    if !inactive.is_empty() {
        c_info!(
            to_console,
            "Kicking {} players due to inactivity.",
            inactive.len()
        );
        for id in inactive {
            world
                .kick(
                    to_console.clone(),
                    socket,
                    id,
                    Some("Kicked due to inactivity."),
                )
                .await?;
        }
    }
    Ok(())
}
async fn process_client_packet(
    to_console: ToConsole,
    socket: &UdpSocket,
    packet: Packet,
    addr: SocketAddr,
    world: &mut World,
) -> io::Result<()> {
    macro_rules! assert_player_exists {
        ($to_console: expr, $world:expr, $addr:expr, $iter:ident, $fn:ident, $player_var:ident, $block:block) => {
            match $world.players.$iter().$fn(|x| x.addr == $addr) {
                None => c_error!($to_console, "addr hasn't joined! ({})", $addr),
                Some($player_var) => $block,
            }
        };
    }
    macro_rules! unwrap_or_return_early {
        ($to_console: expr, $to_try: expr, $err_msg: expr) => {
            match $to_try {
                Ok(ok) => ok,
                Err(e) => {
                    c_error!($to_console, $err_msg, e);
                    return Ok(());
                }
            }
        };
    }
    match packet.t.into() {
        PacketTypes::ClientHello => {
            let hello_packet: ClientHello = unwrap_packet_or_ignore!(to_console, packet);
            c_info!(to_console, "{} joined the server!", hello_packet.name);
            let spawn_x = world.get_spawn();
            let connection = unwrap_or_return_early!(
                to_console,
                ClientConnection::new_at(
                    to_console.clone(),
                    addr,
                    world,
                    spawn_x,
                    hello_packet.name
                ),
                "cannot spawn player: {}"
            );
            let spawn_block_pos = (
                connection.server_player.x.round() as u32,
                connection.server_player.y.round() as u32,
            );

            let response = ServerSync {
                player_id: connection.id,
                world_width: world.width,
                world_height: world.height,
                chunk_size: world.chunk_size,
                spawn_x: connection.server_player.x,
                spawn_y: connection.server_player.y,
            };

            encode_and_send!(to_console, PacketTypes::ServerSync, response, socket, addr);

            // notify other players and the ones loading the chunk
            let spawn_chunk_pos = world
                .get_chunk_block_is_in(spawn_block_pos.0, spawn_block_pos.1)
                .unwrap();
            let players_loading_chunk = world
                .get_list_of_players_loading_chunk(spawn_chunk_pos.0, spawn_chunk_pos.1)
                .unwrap();

            let to_broadcast = ServerPlayerJoin {
                player_name: connection.name.clone(),
                player_id: connection.id,
            };
            let to_broadcast_chunk = ServerPlayerEnterLoaded {
                player_name: connection.name.clone(),
                player_id: connection.id,
                pos_x: connection.server_player.x,
                pos_y: connection.server_player.y,
            };

            for player in world.players.iter() {
                encode_and_send!(
                    to_console,
                    PacketTypes::ServerPlayerJoin,
                    to_broadcast.clone(),
                    socket,
                    player.addr
                );
                if players_loading_chunk.contains(&player) {
                    encode_and_send!(
                        to_console,
                        PacketTypes::ServerPlayerEnterLoaded,
                        to_broadcast_chunk.clone(),
                        socket,
                        player.addr
                    );
                }
            }

            world.players.push(connection);
        }
        PacketTypes::ClientGoodbye => {
            match world.players.par_iter().position_any(|x| x.addr == addr) {
                None => c_error!(
                    to_console,
                    "Goodbye packet from address that hasn't joined! ({})",
                    addr
                ),
                Some(idx) => {
                    let connection = world.players.swap_remove(idx);
                    world.unload_all_for(connection.id);
                    c_info!(
                        to_console,
                        "{} (addr: {}) left the server!",
                        connection.name,
                        connection.addr
                    );

                    let last_location = (
                        connection.server_player.x.round() as u32,
                        connection.server_player.y.round() as u32,
                    );
                    let last_location_chunk_pos = world
                        .get_chunk_block_is_in(last_location.0, last_location.1)
                        .unwrap();
                    let players_loading_chunk = world
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

                    for player in world.players.iter() {
                        encode_and_send!(
                            to_console,
                            PacketTypes::ServerPlayerLeave,
                            to_broadcast.clone(),
                            socket,
                            player.addr
                        );
                        if players_loading_chunk.contains(&player) {
                            encode_and_send!(
                                to_console,
                                PacketTypes::ServerPlayerLeaveLoaded,
                                to_broadcast_chunk.clone(),
                                socket,
                                player.addr
                            );
                        }
                    }
                }
            };
        }
        PacketTypes::ClientPlaceBlock => {
            assert_player_exists!(to_console, world, addr, par_iter, find_any, player_conn, {
                let place_block_packet: ClientPlaceBlock =
                    unwrap_packet_or_ignore!(to_console, packet);
                let (chunk_x, chunk_y) = unwrap_or_return_early!(
                    to_console,
                    world.get_chunk_block_is_in(place_block_packet.x, place_block_packet.y),
                    "error while placing block: {}"
                );
                let players_loading_chunk = unwrap_or_return_early!(
                    to_console,
                    world.get_list_of_players_loading_chunk(chunk_x, chunk_y),
                    "error while placing block: {}"
                );
                if !players_loading_chunk.contains(&player_conn) {
                    c_error!(to_console, "player {} (addr: {}) tried to place a block in a position they themselves have not loaded!", player_conn.name, player_conn.addr);
                    return Ok(());
                }
                match world
                    .set_block_and_notify(
                        to_console.clone(),
                        socket,
                        place_block_packet.x,
                        place_block_packet.y,
                        place_block_packet.block.into(),
                    )
                    .await
                {
                    Ok(_) => (),
                    Err(e) => match e {
                        WorldError::NetworkError(e) => {
                            c_error!(to_console, "error while notifying clients: {e}")
                        }
                        _ => c_error!(to_console, "error while placing block: {e}"),
                    },
                };
            })
        }
        PacketTypes::ClientPlayerJump => {
            assert_player_exists!(to_console, world, addr, par_iter, position_any, idx, {
                let surrounding = world.get_neighbours_of_player(&world.players[idx].server_player);
                world.players[idx].server_player = world.players[idx]
                    .server_player
                    .clone()
                    .do_jump(surrounding);
                let new_conn = &world.players[idx];
                let packet = ServerPlayerUpdatePos {
                    player_id: new_conn.id,
                    pos_x: new_conn.server_player.x,
                    pos_y: new_conn.server_player.y,
                };
                let (chunk_x, chunk_y) = world
                    .get_chunk_block_is_in(
                        new_conn.server_player.x.round() as u32,
                        new_conn.server_player.y.round() as u32,
                    )
                    .unwrap_or((0, 0));
                let players_loading_chunk = world
                    .get_list_of_players_loading_chunk(chunk_x, chunk_y)
                    .unwrap_or_default();
                for conn in players_loading_chunk {
                    encode_and_send!(
                        to_console,
                        PacketTypes::ServerPlayerUpdatePos,
                        packet.clone(),
                        socket,
                        conn.addr
                    );
                }
            });
        }
        PacketTypes::ClientPlayerMoveX => {
            assert_player_exists!(to_console, world, addr, par_iter_mut, position_any, idx, {
                let move_packet: ClientPlayerMoveX = unwrap_packet_or_ignore!(to_console, packet);

                let (old_chunk_x, old_chunk_y) = world
                    .get_chunk_block_is_in(
                        world.players[idx].server_player.x.round() as u32,
                        world.players[idx].server_player.y.round() as u32,
                    )
                    .unwrap_or((0, 0));

                world.players[idx].server_player.x = move_packet.pos_x;
                let new_player = &world.players[idx];
                let (chunk_x, chunk_y) = world
                    .get_chunk_block_is_in(
                        new_player.server_player.x.round() as u32,
                        new_player.server_player.y.round() as u32,
                    )
                    .unwrap_or((0, 0));

                let players_loading_old_chunk = world
                    .get_list_of_players_loading_chunk(old_chunk_x, old_chunk_y)
                    .unwrap_or_default();
                let players_loading_new_chunk = world
                    .get_list_of_players_loading_chunk(chunk_x, chunk_y)
                    .unwrap_or_default();

                let old_players: Vec<&ClientConnection> = players_loading_old_chunk
                    .clone()
                    .into_par_iter()
                    .filter(|conn| !players_loading_new_chunk.contains(conn))
                    .collect();
                let new_players: Vec<&ClientConnection> = players_loading_new_chunk
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
                for conn in players_loading_new_chunk {
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
                    let move_packet = ServerPlayerUpdatePos {
                        player_id: new_player.id,
                        pos_x: new_player.server_player.x,
                        pos_y: new_player.server_player.y,
                    };
                    encode_and_send!(
                        to_console,
                        PacketTypes::ServerPlayerUpdatePos,
                        move_packet,
                        socket,
                        conn.addr
                    );
                }
            });
        }
        PacketTypes::ClientRequestChunk => {
            assert_player_exists!(to_console, world, addr, par_iter, find_any, player_conn, {
                let request_packet: ClientRequestChunk =
                    unwrap_packet_or_ignore!(to_console, packet);
                match world.mark_chunk_loaded_by_id(
                    request_packet.chunk_coords_x,
                    request_packet.chunk_coords_y,
                    player_conn.id,
                ) {
                    Ok(chunk) => {
                        let response = ServerChunkResponse {
                            chunk: chunk.clone().into(),
                        };
                        encode_and_send!(
                            to_console,
                            PacketTypes::ServerChunkResponse,
                            response,
                            socket,
                            addr
                        );
                    }
                    Err(err) => match err {
                        WorldError::ChunkAlreadyLoaded => c_warn!(
                            to_console,
                            "player requested already loaded chunk ({}, {})!",
                            request_packet.chunk_coords_x,
                            request_packet.chunk_coords_y
                        ),
                        _ => c_warn!(to_console, "error marking chunk as loaded! {:?}", err),
                    },
                };
            })
        }
        PacketTypes::ClientUnloadChunk => {
            assert_player_exists!(to_console, world, addr, par_iter, find_any, player_conn, {
                let request_packet: ClientUnloadChunk =
                    unwrap_packet_or_ignore!(to_console, packet);
                match world.unmark_loaded_chunk_for(
                    request_packet.chunk_coords_x,
                    request_packet.chunk_coords_y,
                    player_conn.id,
                ) {
                    Ok(_) => (),
                    Err(err) => {
                        c_error!(to_console, "error marking chunk as unloaded! {:?}", err);
                    }
                };
            })
        }
        PacketTypes::ClientHeartbeat => {
            assert_player_exists!(
                to_console,
                world,
                addr,
                par_iter_mut,
                find_any,
                player_conn,
                {
                    player_conn.connection_alive = true;
                }
            )
        }

        _ => {
            c_error!(
                to_console,
                "server received unknown or client bound packet: {:?}",
                packet
            );
        }
    }

    Ok(())
}
