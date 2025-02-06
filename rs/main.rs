use crate::world::World;
use clap::{Parser, Subcommand};
use log::{error, info};
use std::io;
use std::num::NonZeroU32;
use std::process::exit;
use tokio::net::UdpSocket;
use tokio::time::{self, Duration};

#[macro_use]
mod network;
mod console;
mod constants;
mod player;
mod world;

#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Settings {
    #[arg(short, long, default_value = "8475")]
    port: u16,
    #[arg(long, default_value = "1024")]
    world_width: NonZeroU32,
    #[arg(long, default_value = "256")]
    world_height: NonZeroU32,
    #[arg(short, long, default_value = "16")]
    chunk_size: NonZeroU32,
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
    Terrain {
        #[arg(short, long, default_value = "4")]
        base_height: u32,
        #[arg(short, long, default_value = "128")]
        upper_height: u32,
        #[arg(short, long, default_value = "6")]
        passes: NonZeroU32,
    },
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let (from_console, to_console) = console::init();

    let settings = Settings::parse();
    c_info!(to_console, "Starting up with {:?}", settings);

    let mut world_tick = time::interval(Duration::from_millis(1000 / constants::TICKS_PER_SECOND));
    let mut heartbeat_tick =
        time::interval(Duration::from_secs(constants::SECONDS_BETWEEN_HEARTBEATS));

    let world_res = match settings.world_type {
        WorldType::Empty => World::generate_empty(
            to_console.clone(),
            settings.world_width.into(),
            settings.world_height.into(),
            settings.chunk_size.into(),
        ),
        WorldType::Flat { grass_height } => World::generate_flat(
            to_console.clone(),
            settings.world_width.into(),
            settings.world_height.into(),
            settings.chunk_size.into(),
            grass_height,
        ),
        WorldType::Terrain {
            base_height,
            upper_height,
            passes,
        } => World::generate_terrain(
            to_console.clone(),
            settings.world_width.into(),
            settings.world_height.into(),
            settings.chunk_size.into(),
            base_height,
            upper_height,
            passes.into(),
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
    c_info!(to_console, "Listening on {}", socket.local_addr()?);
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                world.shutdown(to_console.clone(), &socket).await?;
                c_info!(to_console, "Server shutdown complete.");
                break;
            }
            packet = socket.recv_from(&mut buf) => {
                network::incoming_packet_handler(to_console.clone(), &socket, &mut buf, &mut world, packet?).await?
            }
            _ = heartbeat_tick.tick() => {
                network::heartbeat(to_console.clone(), &socket, &mut world).await?;
            }
            _ = world_tick.tick() => {
                world.tick(to_console.clone(), &socket).await?;
            }
        }
    }

    Ok(())
}
