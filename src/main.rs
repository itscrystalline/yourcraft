use tokio::{net::UdpSocket, sync::mpsc};
use std::{io, net::SocketAddr, sync::Arc};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_pickle::{from_iter, from_slice, DeOptions};
use crate::network::{HelloPacket, Packet, PacketTypes, PlayerCoordinates};

mod network;

macro_rules! unwrap_packet {
    ($packet: expr) => {from_slice(&$packet.data, DeOptions::new()).unwrap()};
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    
    let socket = UdpSocket::bind("0.0.0.0:8475").await?;
    info!("Listening on {}", socket.local_addr()?);
    let mut buf = [0; 1024];
    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        info!("{:?} bytes received from {:?}", len, addr);
        
        let packet: Packet = from_slice(&buf[..len], DeOptions::new()).unwrap();
        
        match packet.t.into() {
            PacketTypes::HelloPacket => {
                let hello_packet: HelloPacket = unwrap_packet!(packet);
                debug!("{:?}", hello_packet);
            },
            PacketTypes::PlayerCoordinates => {
                let player_coords: PlayerCoordinates = unwrap_packet!(packet);
                debug!("{:?}", player_coords);
            },
            PacketTypes::Invalid => error!("{:?} packet: {:?}", packet, packet),
        }
    }
}
