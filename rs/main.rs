use tokio::net::UdpSocket;
use std::io;
use log::info;
use crate::world::{Block, World};

mod network;
mod world;
mod player;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let mut world = World::generate_empty(8, 8, 2).unwrap();
    world.set_block(3, 2, Block::Grass).unwrap();
    
    let socket = UdpSocket::bind("0.0.0.0:8475").await?;
    info!("Listening on {}", socket.local_addr()?);
    let mut buf = [0u8; 1024];
    
    loop {
        network::network_handler(&socket, &mut buf, &mut world).await?
    }
}

