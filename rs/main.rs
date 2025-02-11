use crate::world::World;
use clap::{Parser, Subcommand};
use console::Stats;
use log::{error, info, LevelFilter};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator};
use std::cmp::max;
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
    #[arg(long)]
    spawn_point: Option<u32>,
    #[arg(long, default_value = "16")]
    spawn_range: NonZeroU32,
    #[arg(short, long, default_value = "false")]
    no_console: bool,
    #[arg(long, default_value = "false")]
    debug: bool,
    #[arg(long, default_value = "false")]
    no_heartbeat: bool,
    #[command(subcommand)]
    world_type: WorldType,
}

#[derive(Subcommand, Debug)]
pub enum WorldType {
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
        #[arg(short, long)]
        seed: Option<u64>,
        #[arg(short, long, default_value = "5")]
        noise_passes: usize,
    },
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let settings = Settings::parse();

    env_logger::Builder::new()
        .filter_level({
            if settings.debug {
                LevelFilter::Debug
            } else {
                LevelFilter::Info
            }
        })
        .init();

    let (console_thread, mut from_console, to_console) =
        console::init(!settings.no_console, settings.debug);

    c_info!(to_console, "Starting up with {:?}", settings);

    let mut world_tick = time::interval(Duration::from_millis(1000 / constants::TICKS_PER_SECOND));
    let mut heartbeat_tick =
        time::interval(Duration::from_secs(constants::SECONDS_BETWEEN_HEARTBEATS));
    let mut uptime_clock = time::interval(Duration::from_secs(1));

    let spawn_point = settings
        .spawn_point
        .unwrap_or(u32::from(settings.world_width) / 2);

    let world_res = World::generate(
        to_console.clone(),
        settings.world_width.into(),
        settings.world_height.into(),
        settings.chunk_size.into(),
        spawn_point,
        settings.spawn_range,
        settings.world_type,
    );
    let mut world = match world_res {
        Ok(w) => w,
        Err(e) => {
            let _ = to_console.send(console::ToConsoleType::Quit);
            console_thread.await.unwrap();
            error!("Error creating world: {e}");
            exit(1);
        }
    };

    // uptime, stats
    let mut uptime = Duration::default();
    let mut last_tick_time = Duration::default();
    // 1s, 5s, 10s, 30s, 1m, 2m, 5m, 10m
    let mut tick_times_saved: [Duration; 8] = [Duration::default(); 8];
    let mut tick_times_current: [Duration; 8] = [Duration::default(); 8];
    let mut tick_times_count: [u32; 8] = [0u32; 8];

    let socket = UdpSocket::bind(format!("0.0.0.0:{}", settings.port)).await?;
    let mut buf = [0u8; 1024];
    c_info!(to_console, "Listening on {}", socket.local_addr()?);
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                world.shutdown(to_console.clone(), &socket).await?;
                let _ = to_console.send(console::ToConsoleType::Quit);
                console_thread.await.unwrap();
                break;
            }
            packet = socket.recv_from(&mut buf) => {
                network::incoming_packet_handler(to_console.clone(), &socket, &mut buf, &mut world, packet?).await?
            }
            _ = heartbeat_tick.tick() => {
                if !settings.no_heartbeat {
                    network::heartbeat(to_console.clone(), &socket, &mut world).await?;
                }
            }
            _ = world_tick.tick() => {
                last_tick_time = world.tick(to_console.clone(), &socket).await?;
                tick_times_current.par_iter_mut().enumerate().for_each(|(idx, time)| {
                    *time = ((*time * tick_times_count[idx]) + last_tick_time) / (tick_times_count[idx] + 1);
                });
                tick_times_count.par_iter_mut().for_each(|count| *count += 1);
            }
            _ = uptime_clock.tick() => {
                uptime += Duration::from_secs(1);
                macro_rules! save_and_reset {
                    ($saved: expr, $current: expr, $idx: expr) => (
                        $saved[$idx] = $current[$idx];
                        $current[$idx] = Duration::default();
                    )
                }
                save_and_reset!(tick_times_saved, tick_times_current, 0);
                let secs = uptime.as_secs();
                if secs % 2 == 0 {
                    save_and_reset!(tick_times_saved, tick_times_current, 1);
                }
                if secs % 5 == 0 {
                    save_and_reset!(tick_times_saved, tick_times_current, 2);
                }
                if secs % 10 == 0 {
                    save_and_reset!(tick_times_saved, tick_times_current, 3);
                }
                if secs % 30 == 0 {
                    save_and_reset!(tick_times_saved, tick_times_current, 4);
                }
                if secs % 60 == 0 {
                    save_and_reset!(tick_times_saved, tick_times_current, 5);
                }
                if secs % 300 == 0 {
                    save_and_reset!(tick_times_saved, tick_times_current, 6);
                }
                if secs % 600 == 0 {
                    save_and_reset!(tick_times_saved, tick_times_current, 7);
                }
                if !settings.no_console {
                    let _ = to_console.send(console::ToConsoleType::Stats(Stats {
                        uptime,
                        tps: 1000u128 / max(tick_times_saved[0].as_millis(), 1000u128 / constants::TICKS_PER_SECOND as u128),
                        mspt: tick_times_saved[0],
                        players: world.players.len()
                    }));
                }
            }
            command_opt = from_console.recv() => {
                if let Some(command) = command_opt {
                    if console::process_command(to_console.clone(), &socket, &mut world, command, tick_times_saved, last_tick_time).await? {
                        let _ = to_console.send(console::ToConsoleType::Quit);
                        console_thread.await.unwrap();
                        break;
                    }
                }
            }
        }
    }

    info!("Server shutdown complete after being up for {uptime:?}.");
    Ok(())
}
