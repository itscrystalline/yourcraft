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
    /// The port to use for clients to connect to.
    #[arg(short, long, default_value = "8475")]
    port: u16,
    /// The world's horizontal size.
    #[arg(long, default_value = "1024")]
    world_width: NonZeroU32,
    /// The world's vertical size.
    #[arg(long, default_value = "256")]
    world_height: NonZeroU32,
    /// The size of each chunk the world subdivides into.
    #[arg(short, long, default_value = "16")]
    chunk_size: NonZeroU32,
    /// The x coordinate of the center of spawn point. Defaults to the center of the world. (e.g.
    /// world_width / 2)
    #[arg(long)]
    spawn_point: Option<u32>,
    /// The spawn range that players will spawn around spawn_point.
    #[arg(long, default_value = "16")]
    spawn_range: NonZeroU32,
    /// Disables the command console.
    #[arg(short, long, default_value = "false")]
    no_console: bool,
    /// Enables Debug Logging.
    #[arg(long, default_value = "false")]
    debug: bool,
    /// Disables sending heartbeat packets to connected clients.
    #[arg(long, default_value = "false")]
    no_heartbeat: bool,
    /// The amount of network errors that are allowed to happen before the server exits.
    #[arg(long, default_value = "3")]
    max_network_errors: u8,
    /// The world type to generate.
    #[command(subcommand)]
    world_type: WorldType,
}

#[derive(Subcommand, Debug)]
pub enum WorldType {
    /// An empty world with nothing in it.
    Empty,
    /// A flat grass world.
    Flat {
        /// The height the grass layer generates at.
        #[arg(short, long, default_value = "4")]
        grass_height: u32,
    },
    /// Perlin noise based terrain.
    Terrain {
        /// The minimum height terrain can generate.
        #[arg(short, long, default_value = "48")]
        base_height: u32,
        /// The maximum height terrain can generate.
        #[arg(short, long, default_value = "192")]
        upper_height: u32,
        /// The height water generates up to.
        #[arg(short, long, default_value = "64")]
        water_height: u32,
        /// The seed for the world generator, Defaults to a randomly selected u64.
        #[arg(short, long)]
        seed: Option<u64>,
        /// How many Perlin noise generators should be created.
        #[arg(short, long, default_value = "5")]
        noise_passes: usize,
        /// The power to raise the final noise value with. Higher means more flatlands and steeper
        /// mountains, less means mose hills and less flatland.
        #[arg(short, long, default_value = "3.0")]
        redistribution_factor: f64,
        /// The chance for a cave to generate in each world "slice", in percent.
        #[arg(long, default_value = "10", value_parser = clap::value_parser!(u32).range(0..=100))]
        cave_gen_chance_percent: u32,
        /// How many segments each cave will have.
        #[arg(long, default_value = "72")]
        cave_gen_steps: u8,
        /// Lower bound for cave size.
        #[arg(long, default_value = "1")]
        cave_gen_min_width: u8,
        /// Upper bound for cave size.
        #[arg(long, default_value = "3")]
        cave_gen_max_width: u8,
        /// Max turn angle for each step of the cave in degrees.
        #[arg(long, default_value = "60", value_parser = clap::value_parser!(u16).range(0..=360))]
        cave_gen_max_turn_angle: u16,
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

    c_debug!(to_console, "Starting up with {:?}", settings);

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
    let mut network_error_strikes = 0u8;
    c_info!(to_console, "Listening on {}", socket.local_addr()?);
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            packet_maybe = socket.recv_from(&mut buf) => {
                // hopefully will fix windows bullshit
                match packet_maybe {
                    Ok(packet) => network::incoming_packet_handler(to_console.clone(), &socket, &mut buf, &mut world, packet).await?,
                    Err(e) => {
                        c_error!(to_console, "Encountered a network error while trying to recieve a packet: {}", e);
                        network_error_strikes += 1;
                        if network_error_strikes > settings.max_network_errors {
                            c_error!(to_console, "max_network_errors reached! shutting down.");
                            break;
                        }
                    }
                }
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
                        break;
                    }
                }
            }
        }
    }

    world.shutdown(to_console.clone(), &socket).await?;
    let _ = to_console.send(console::ToConsoleType::Quit);
    console_thread.await.unwrap();

    info!("Server shutdown complete after being up for {uptime:?}.");
    Ok(())
}
