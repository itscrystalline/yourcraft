use crate::world::{Block, World};
use clap::{Parser, Subcommand};
use log::{error, info};
use std::io;
use std::process::exit;
use tokio::net::UdpSocket;
use tokio::time::{self, Duration};

#[macro_use]
mod network;
mod player;
mod world;

#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Settings {
    #[arg(short, long, default_value = "8475")]
    port: u16,
    #[arg(long, default_value = "1024")]
    world_width: u32,
    #[arg(long, default_value = "256")]
    world_height: u32,
    #[arg(short, long, default_value = "16")]
    chunk_size: u32,
    #[command(subcommand)]
    world_type: WorldType,
}

#[derive(Subcommand, Debug)]
enum WorldType {
    Empty,
    Flat {
        #[arg(short, long, default_value = "4")]
        grass_height: u32,
    },
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let settings = Settings::parse();
    info!("Starting up with {:?}", settings);

    let mut clock = time::interval(Duration::from_millis(20));

    let world_res = match settings.world_type {
        WorldType::Empty => World::generate_empty(
            settings.world_width,
            settings.world_height,
            settings.chunk_size,
        ),
        WorldType::Flat { grass_height } => World::generate_flat(
            settings.world_width,
            settings.world_height,
            settings.chunk_size,
            grass_height,
        ),
    };
    let mut world = match world_res {
        Ok(w) => w,
        Err(e) => {
            error!("Error creating world: {e}");
            exit(1)
        }
    };

    let socket = UdpSocket::bind(format!("0.0.0.0:{}", settings.port)).await?;
    let mut buf = [0u8; 1024];
    info!("Listening on {}", socket.local_addr()?);

    loop {
        clock.tick().await;

        network::incoming_packet_handler(&socket, &mut buf, &mut world).await?;
        world.tick(&socket).await?;
    }
}
