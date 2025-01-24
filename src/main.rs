use tokio::{net::UdpSocket, sync::mpsc};
use std::{io, net::SocketAddr, sync::Arc};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_pickle::{from_iter, from_slice, to_vec, DeOptions, SerOptions};
use crate::network::{ClientConnection, ClientHello, Packet, PacketTypes, ServerSync};
use crate::world::{Block, World};

mod network;
mod world;

macro_rules! unwrap_packet {
    ($packet: expr) => {from_slice(&$packet.data, DeOptions::new()).unwrap()};
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let mut world = World::generate_empty(8, 8, 2).unwrap();
    world.set_block(3, 2, Block::Grass).unwrap();

    let mut players: Vec<ClientConnection> = Vec::new();
    
    let socket = UdpSocket::bind("0.0.0.0:8475").await?;
    info!("Listening on {}", socket.local_addr()?);
    let mut buf = [0u8; 1024];
    loop {
        network_handler(&socket, &mut buf, &mut world, &mut players).await?
    }
}

async fn network_handler(socket: &UdpSocket, buf: &mut [u8], world: &mut World, players: &mut Vec<ClientConnection>) -> io::Result<()> {
    let (len, client_addr) = socket.recv_from(buf).await?;
    info!("{:?} bytes received from {:?}", len, client_addr);

    let packet: Packet = from_slice(&buf[..len], DeOptions::new()).unwrap();
    process_packet(socket, packet, client_addr, world, players).await?;
    Ok(())
}
async fn process_packet(socket: &UdpSocket, packet: Packet, addr: SocketAddr, world: &mut World, players: &mut Vec<ClientConnection>) -> io::Result<()> {
    match packet.t.into() {
        PacketTypes::ClientHello => {
            let hello_packet: ClientHello = unwrap_packet!(packet);
            info!("{} joined the server!", hello_packet.name);
            let connection = ClientConnection::new(addr, hello_packet.name);
            
            let response = ServerSync {
                player_id: connection.id,
                world_width: world.width,
                world_height: world.height,
                chunk_size: world.chunk_size,
            };
            let encoded = Packet::encode(PacketTypes::ServerSync, response).unwrap();

            socket.send_to(&encoded, addr).await?;
            players.push(connection);
        }
        PacketTypes::Invalid => { 
            error!("unknown packet: {:?}", packet);
        },
        _ => todo!(),
    }
    
    Ok(())
}
