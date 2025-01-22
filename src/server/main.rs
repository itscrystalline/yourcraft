use tokio::{net::UdpSocket, sync::mpsc};
use std::{io, net::SocketAddr, sync::Arc};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_pickle::{from_iter, from_slice, DeOptions};

#[derive(Serialize, Deserialize, Debug)]
#[repr(u8)]
enum PacketTypes {
    NoPacket,
    HelloPacket
}

impl Into<u8> for PacketTypes {
    fn into(self) -> u8 {
        self as u8
    }
}
impl Into<PacketTypes> for u8 {
    fn into(self) -> PacketTypes {
        match self {
            0 => PacketTypes::HelloPacket,
            _ => PacketTypes::NoPacket
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
struct Packet {
    t: u8,
    #[serde(with = "serde_bytes")]
    data: Vec<u8>
}
#[derive(Serialize, Deserialize, Debug)]
struct HelloPacket {
    timestamp: u64
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
                debug!("packet: {:?}", packet);
                let hello_packet: HelloPacket = from_slice(&packet.data, DeOptions::new()).unwrap();
                debug!("{:?}", hello_packet);
            },
            PacketTypes::NoPacket => error!("{:?} packet: {:?}", packet, packet)
        }
    }
}
