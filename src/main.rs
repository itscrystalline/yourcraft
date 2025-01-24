use tokio::{net::UdpSocket, sync::mpsc};
use std::{io, net::SocketAddr, sync::Arc};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_pickle::{from_iter, from_slice, DeOptions};
use crate::network::{ClientHello, Packet, PacketTypes, ServerSync};
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
    
    let socket = UdpSocket::bind("0.0.0.0:8475").await?;
    info!("Listening on {}", socket.local_addr()?);
    let mut buf = [0; 1024];
    loop {
        let (len, client_addr) = socket.recv_from(&mut buf).await?;
        info!("{:?} bytes received from {:?}", len, client_addr);
        
        let packet: Packet = from_slice(&buf[..len], DeOptions::new()).unwrap();
        
        match packet.t.into() {
            PacketTypes::ClientHello => {
                let hello_packet: ClientHello = unwrap_packet!(packet);
                debug!("{:?}", hello_packet);
            }
            PacketTypes::ServerSync => {
                let player_coords: ServerSync = unwrap_packet!(packet);
                debug!("{:?}", player_coords);
            }
            PacketTypes::Invalid => error!("unknown packet: {:?}", packet),
        }
    }
}
