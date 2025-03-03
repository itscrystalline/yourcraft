use crate::console::ToConsole;
use crate::player::{Item, ItemStack, Player};
use crate::world::{is_solid, Block, Chunk, World, WorldError};
use crate::{c_debug, c_error, c_info, c_warn, constants};
use rand::prelude::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_pickle::{from_slice, to_vec, DeOptions, SerOptions};
use std::io;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

impl PacketTypes {
    pub fn to_bytes(&self) -> serde_pickle::Result<Vec<u8>> {
        to_vec(self, SerOptions::new())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientConnection {
    pub addr: SocketAddr,
    pub name: String,
    pub id: u32,
    pub server_player: Player,
    pub connection_alive: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq)]
pub struct NetworkChunk {
    size: u32,
    chunk_x: u32,
    chunk_y: u32,
    blocks: Vec<u8>,
}

impl From<Chunk> for NetworkChunk {
    fn from(chunk: Chunk) -> Self {
        Self {
            size: chunk.size,
            chunk_x: chunk.chunk_x,
            chunk_y: chunk.chunk_y,
            blocks: chunk.blocks.into_par_iter().map(|bl| bl.into()).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct NetworkItemStack {
    item: u8,
    count: u8,
}

impl From<ItemStack> for NetworkItemStack {
    fn from(stack: ItemStack) -> Self {
        Self {
            item: stack.item.into(),
            count: stack.count.get(),
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
        addr: SocketAddr,
        world: &World,
        x: u32,
        name: String,
    ) -> Result<ClientConnection, WorldError> {
        Ok(ClientConnection {
            addr,
            name,
            server_player: Player::spawn_at(world, x)?,
            id: rand::rng().next_u32(),
            connection_alive: true,
        })
    }
}

// Use the macro to define packets
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum PacketTypes {
    ClientHello {
        name: String,
    },
    ServerSync {
        player_id: u32,
        world_width: u32,
        world_height: u32,
        chunk_size: u32,
        spawn_x: f32,
        spawn_y: f32,
    },
    ClientRequestChunk {
        chunk_coords_x: u32,
        chunk_coords_y: u32,
    },
    ServerChunkResponse {
        chunk: NetworkChunk,
    },
    ClientUnloadChunk {
        chunk_coords_x: u32,
        chunk_coords_y: u32,
    },
    ServerPlayerJoin {
        player_name: String,
        player_id: u32,
    },
    ServerPlayerEnterLoaded {
        player_name: String,
        player_id: u32,
        pos_x: f32,
        pos_y: f32,
    },
    ServerPlayerLeaveLoaded {
        player_name: String,
        player_id: u32,
    },
    ServerPlayerLeave {
        player_name: String,
        player_id: u32,
    },
    ClientGoodbye {},
    ClientPlaceBlock {
        x: u32,
        y: u32,
    },
    ServerUpdateBlock {
        block: u8,
        x: u32,
        y: u32,
    },
    ServerBatchUpdateBlock {
        block: u8,
        batch: Vec<(u32, u32)>,
    },
    ClientPlayerXVelocity {
        vel_x: f32,
    },
    ClientPlayerJump {},
    ClientPlayerRespawn {},
    ServerPlayerUpdatePos {
        player_id: u32,
        pos_x: f32,
        pos_y: f32,
    },
    ServerKick {
        msg: String,
    },
    ServerHeartbeat {},
    ClientHeartbeat {},
    ClientSendMessage {
        msg: String,
    },
    ServerSendMessage {
        player_name: String,
        player_id: u32,
        msg: String,
    },
    ClientBreakBlock {
        x: u32,
        y: u32,
    },
    ClientTryAttack {
        player_id: u32,
    },
    ClientChangeSlot {
        slot: u8,
    },
    ServerUpdateHealth {
        health: f32,
    },
    ServerUpdateInventory {
        inv: [Option<NetworkItemStack>; 9],
    },
    ClientRequestCraft {
        item: u8,
    },
}

#[macro_export]
macro_rules! encode_and_send {
    ($to_network: expr, $packet: expr, $addr: expr) => {
        let encoded = $packet.to_bytes().unwrap();
        let _ = $to_network.send($crate::network::NetworkThreadMessage::Packet(
            $addr, encoded,
        ));
    };
}

pub enum NetworkThreadMessage {
    Shutdown,
    Packet(SocketAddr, Vec<u8>),
}

pub type ToNetwork = UnboundedSender<NetworkThreadMessage>;
pub type FromNetwork = UnboundedReceiver<(SocketAddr, PacketTypes)>;
type ToMain = UnboundedSender<(SocketAddr, PacketTypes)>;

pub fn init(
    socket: UdpSocket,
    to_console: ToConsole,
    max_network_errors: u8,
) -> (JoinHandle<()>, FromNetwork, ToNetwork) {
    let (to_main, from_network) = mpsc::unbounded_channel::<(SocketAddr, PacketTypes)>();
    let (to_network, from_main) = mpsc::unbounded_channel::<NetworkThreadMessage>();
    let network_thread = tokio::spawn(async move {
        let (to_main, mut from_main) = (to_main, from_main);
        let mut buf = [0u8; 1024];
        let mut network_error_strikes = 0u8;
        c_info!(
            to_console,
            "Listening on {}",
            socket.local_addr().expect("cannot get socket address")
        );
        loop {
            tokio::select! {
                maybe_packet_incoming = socket.recv_from(&mut buf) => {
                    match maybe_packet_incoming {
                        Ok((len, addr)) => {
                            if let Err(e) = incoming_packet_handler(to_console.clone(), to_main.clone(), len, addr, &mut buf).await {
                                c_error!(to_console, "error while handling packet: {e}");
                            }
                        },
                        Err(e) => {
                            c_error!(to_console, "Encountered a network error while trying to recieve a packet: {}", e);
                            network_error_strikes += 1;
                            if network_error_strikes > max_network_errors {
                                c_error!(to_console, "max_network_errors reached! shutting down.");
                                break;
                            }
                        }
                    }
                }
                outgoing_message = from_main.recv() => {
                    if let Some(message) = outgoing_message {
                        match message {
                            NetworkThreadMessage::Shutdown => break,
                            NetworkThreadMessage::Packet(addr, packet) => {
                                let _ = socket.send_to(&packet, addr).await;
                            }
                        }
                    }
                }
            }
        }
    });
    (network_thread, from_network, to_network)
}

pub async fn incoming_packet_handler(
    to_console: ToConsole,
    to_main: ToMain,
    len: usize,
    addr: SocketAddr,
    buf: &mut [u8],
) -> io::Result<()> {
    //c_debug!(to_console, "{:?} bytes received from {:?}", len, addr);

    let packet: serde_pickle::Result<PacketTypes> = from_slice(&buf[..len], DeOptions::new());
    match packet {
        Ok(packet) => {
            let _ = to_main.send((addr, packet));
        }
        Err(e) => {
            c_warn!(
                to_console,
                "Recieved unknown packet from {}, ignoring! (Err: {:?})",
                addr,
                e
            );
        }
    }

    Ok(())
}

pub async fn heartbeat(
    to_console: ToConsole,
    to_network: ToNetwork,
    world: &mut World,
) -> io::Result<()> {
    // sends a heartbeat packet to all incoming players.
    let mut inactive: Vec<u32> = vec![];
    for player in world.players.iter_mut() {
        if player.connection_alive {
            encode_and_send!(to_network, PacketTypes::ServerHeartbeat {}, player.addr);
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
                    to_network.clone(),
                    id,
                    Some("Kicked due to inactivity."),
                )
                .await?;
        }
    }
    Ok(())
}
pub async fn process_client_packet(
    to_console: ToConsole,
    to_network: ToNetwork,
    packet: PacketTypes,
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
    macro_rules! result_unwrap_or_return_early {
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
    macro_rules! option_unwrap_or_return_early {
        ($to_console: expr, $to_try: expr, $err_msg: expr) => {
            match $to_try {
                Some(ok) => ok,
                None => {
                    c_error!($to_console, $err_msg);
                    return Ok(());
                }
            }
        };
    }
    match packet {
        PacketTypes::ClientHello { name } => {
            c_info!(to_console, "{} joined the server!", name);
            let spawn_x = world.get_spawn();
            let connection = result_unwrap_or_return_early!(
                to_console,
                ClientConnection::new_at(addr, world, spawn_x, name),
                "cannot spawn player: {}"
            );
            let spawn_block_pos = (
                connection.server_player.x.round() as u32,
                connection.server_player.y.round() as u32,
            );

            encode_and_send!(
                to_network,
                PacketTypes::ServerSync {
                    player_id: connection.id,
                    world_width: world.width,
                    world_height: world.height,
                    chunk_size: world.chunk_size,
                    spawn_x: connection.server_player.x,
                    spawn_y: connection.server_player.y,
                },
                addr
            );

            // notify other players and the ones loading the chunk
            let spawn_chunk_pos = world
                .get_chunk_block_is_in(spawn_block_pos.0, spawn_block_pos.1)
                .unwrap_or_else(|_| unreachable!());
            let players_loading_chunk = world
                .get_list_of_players_loading_chunk(spawn_chunk_pos.0, spawn_chunk_pos.1)
                .unwrap_or_else(|_| unreachable!());

            for player in world.players.iter() {
                encode_and_send!(
                    to_network,
                    PacketTypes::ServerPlayerJoin {
                        player_name: connection.name.clone(),
                        player_id: connection.id,
                    },
                    player.addr
                );
                if players_loading_chunk.contains(&player) {
                    encode_and_send!(
                        to_network,
                        PacketTypes::ServerPlayerEnterLoaded {
                            player_name: connection.name.clone(),
                            player_id: connection.id,
                            pos_x: connection.server_player.x,
                            pos_y: connection.server_player.y,
                        },
                        player.addr
                    );
                    encode_and_send!(
                        to_network,
                        PacketTypes::ServerPlayerEnterLoaded {
                            player_name: player.name.clone(),
                            player_id: player.id,
                            pos_x: player.server_player.x,
                            pos_y: player.server_player.y
                        },
                        connection.addr
                    );
                }
            }

            world.players.push(connection);
        }
        PacketTypes::ClientGoodbye {} => {
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
                    let last_location_chunk_pos = result_unwrap_or_return_early!(
                        to_console,
                        world.get_chunk_block_is_in(last_location.0, last_location.1),
                        "cannot get chunk: {}"
                    );
                    let players_loading_chunk = result_unwrap_or_return_early!(
                        to_console,
                        world.get_list_of_players_loading_chunk(
                            last_location_chunk_pos.0,
                            last_location_chunk_pos.1,
                        ),
                        "cannot get players loading chunk: {}"
                    );

                    for player in world.players.iter() {
                        encode_and_send!(
                            to_network,
                            PacketTypes::ServerPlayerLeave {
                                player_name: connection.name.clone(),
                                player_id: connection.id,
                            },
                            player.addr
                        );
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
                    }
                }
            };
        }
        PacketTypes::ClientPlaceBlock { x, y } | PacketTypes::ClientBreakBlock { x, y } => {
            assert_player_exists!(to_console, world, addr, par_iter, position_any, idx, {
                let (chunk_x, chunk_y) = result_unwrap_or_return_early!(
                    to_console,
                    world.get_chunk_block_is_in(x, y),
                    "error while placing block: {}"
                );
                let players_loading_chunk = result_unwrap_or_return_early!(
                    to_console,
                    world.get_list_of_players_loading_chunk(chunk_x, chunk_y),
                    "error while placing block: {}"
                );
                if !players_loading_chunk.contains(&&world.players[idx]) {
                    c_error!(to_console, "player {} (addr: {}) tried to place a block in a position they themselves have not loaded!", world.players[idx].name, world.players[idx].addr);
                    return Ok(());
                }
                let (player_x, player_y, x_i, y_i) = (
                    world.players[idx].server_player.x.round() as i32,
                    world.players[idx].server_player.y.round() as i32,
                    x as i32,
                    y as i32,
                );

                if ((x_i - player_x).pow(2) + (y_i - player_y).pow(2))
                    > constants::MAX_INTERACT_RANGE.pow(2) as i32
                {
                    c_warn!(
                        to_console,
                        "player {} tried to interact outside of their max range!",
                        world.players[idx].name
                    );
                    return Ok(());
                }

                match packet {
                    PacketTypes::ClientBreakBlock { x, y } => {
                        let block = result_unwrap_or_return_early!(
                            to_console,
                            world.get_block(x, y),
                            "cannot get block: {}"
                        );
                        if is_solid(block) {
                            let item: Option<Item> = block.into();
                            if let Some(item) = item {
                                let stack: ItemStack = item.into();
                                let _ = world.players[idx].server_player.insert(stack);
                                world.players[idx].server_player.notify_inventory_changed(
                                    to_network.clone(),
                                    world.players[idx].addr,
                                );
                                if let Err(e) = world
                                    .set_block_and_notify(to_network.clone(), x, y, Block::Air)
                                    .await
                                {
                                    match e {
                                        WorldError::NetworkError(e) => {
                                            c_error!(
                                                to_console,
                                                "error while notifying clients: {e}"
                                            )
                                        }
                                        _ => c_error!(to_console, "error while placing block: {e}"),
                                    }
                                }
                            }
                        }
                    }
                    PacketTypes::ClientPlaceBlock { x, y } => {
                        let block_there = result_unwrap_or_return_early!(
                            to_console,
                            world.get_block(x, y),
                            "cannot get block: {}"
                        );
                        if !is_solid(block_there) && world.check_block_placment(x, y) {
                            if let Some(item) =
                                world.players[idx].server_player.get_current_itemstack()
                            {
                                if let Some(block) = item.item.into() {
                                    let place_result = world
                                        .set_block_and_notify(to_network.clone(), x, y, block)
                                        .await;

                                    match place_result {
                                        Ok(_) => {
                                            world.players[idx].server_player.consume_current();
                                            world.players[idx]
                                                .server_player
                                                .notify_inventory_changed(
                                                    to_network,
                                                    world.players[idx].addr,
                                                );
                                        }
                                        Err(e) => match e {
                                            WorldError::NetworkError(e) => {
                                                c_error!(
                                                    to_console,
                                                    "error while notifying clients: {e}"
                                                )
                                            }
                                            _ => c_error!(
                                                to_console,
                                                "error while placing block: {e}"
                                            ),
                                        },
                                    }
                                }
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            })
        }
        PacketTypes::ClientPlayerJump {} => {
            assert_player_exists!(to_console, world, addr, par_iter, position_any, idx, {
                world.players[idx].server_player.do_jump = true;
                c_debug!(to_console, "jumping!");
            });
        }
        PacketTypes::ClientPlayerXVelocity { vel_x } => {
            assert_player_exists!(to_console, world, addr, par_iter_mut, position_any, idx, {
                c_debug!(to_console, "get vel x: {vel_x}");
                world.players[idx].server_player.velocity.x = vel_x;
            });
        }
        PacketTypes::ClientRequestChunk {
            chunk_coords_x,
            chunk_coords_y,
        } => {
            assert_player_exists!(to_console, world, addr, par_iter, find_any, player_conn, {
                match world.mark_chunk_loaded_by_id(chunk_coords_x, chunk_coords_y, player_conn.id)
                {
                    Ok(chunk) => {
                        encode_and_send!(
                            to_network,
                            PacketTypes::ServerChunkResponse {
                                chunk: chunk.clone().into(),
                            },
                            addr
                        );
                    }
                    Err(err) => match err {
                        WorldError::ChunkAlreadyLoaded => c_warn!(
                            to_console,
                            "player requested already loaded chunk ({}, {})!",
                            chunk_coords_x,
                            chunk_coords_y
                        ),
                        _ => c_warn!(to_console, "error marking chunk as loaded! {:?}", err),
                    },
                };
            })
        }
        PacketTypes::ClientUnloadChunk {
            chunk_coords_x,
            chunk_coords_y,
        } => {
            assert_player_exists!(to_console, world, addr, par_iter, find_any, player_conn, {
                match world.unmark_loaded_chunk_for(chunk_coords_x, chunk_coords_y, player_conn.id)
                {
                    Ok(_) => (),
                    Err(err) => {
                        c_error!(to_console, "error marking chunk as unloaded! {:?}", err);
                    }
                };
            })
        }
        PacketTypes::ClientPlayerRespawn {} => {
            assert_player_exists!(to_console, world, addr, par_iter, position_any, idx, {
                let spawn = world.get_spawn();
                let old_player_conn = &world.players[idx];
                let (old_x, old_y) = (
                    old_player_conn.server_player.x,
                    old_player_conn.server_player.y,
                );
                world.players[idx].server_player = result_unwrap_or_return_early!(
                    to_console,
                    Player::spawn_at(world, spawn),
                    "cannot spawn player: {}"
                );
                world.notify_player_moved(to_network, world.players[idx].id, old_x, old_y)?;
            });
        }
        PacketTypes::ClientHeartbeat {} => {
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
        PacketTypes::ClientSendMessage { msg } => {
            assert_player_exists!(to_console, world, addr, par_iter, find_any, player_conn, {
                c_info!(to_console, "[CHAT] <{}> {}", player_conn.name, msg);
                world.players.iter().for_each(|player| {
                    encode_and_send!(
                        to_network,
                        PacketTypes::ServerSendMessage {
                            player_name: player_conn.name.clone(),
                            player_id: player_conn.id,
                            msg: msg.clone()
                        },
                        player.addr
                    );
                });
            })
        }
        PacketTypes::ClientTryAttack { player_id } => {
            let attacker_idx = option_unwrap_or_return_early!(
                to_console,
                world.players.iter().position(|conn| conn.addr == addr),
                "cannot find attacker"
            );
            let attacked_idx = option_unwrap_or_return_early!(
                to_console,
                world.players.iter().position(|conn| conn.id == player_id),
                "cannot find attackee"
            );
            let attacker = &world.players[attacker_idx];
            let attacked = &world.players[attacked_idx];
            let (dist_x, dist_y) = (
                attacker.server_player.x - attacked.server_player.x,
                attacker.server_player.y - attacked.server_player.y,
            );
            if dist_x.powi(2) + dist_y.powi(2) <= constants::MAX_INTERACT_RANGE.pow(2) as f32 {
                let mut attacked = attacked.clone();
                let damage = attacker.server_player.get_current_damage();
                attacked.server_player.health -= damage;
                encode_and_send!(
                    to_network,
                    PacketTypes::ServerUpdateHealth {
                        health: attacked.server_player.health
                    },
                    attacked.addr
                );

                let magnitude = (dist_x.powi(2) + dist_y.powi(2)).sqrt();
                let (norm_x, norm_y) = (dist_x / magnitude, dist_y / magnitude);
                attacked.server_player.acceleration.x = norm_x * constants::KNOCKBACK_POWER;
                attacked.server_player.acceleration.y = norm_y * constants::KNOCKBACK_POWER;

                world.players[attacked_idx] = attacked;
            }
        }
        PacketTypes::ClientChangeSlot { slot } => {
            assert_player_exists!(to_console, world, addr, par_iter, position_any, idx, {
                world.players[idx].server_player.selected_slot = slot;
            })
        }
        PacketTypes::ClientRequestCraft { item } => {}

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
