use std::{collections::HashMap, io, num::ParseIntError, str::FromStr};

use log::{debug, error, info, warn};
use strum::ParseError;
use thiserror::Error;
use tokio::{
    net::UdpSocket,
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};

use crate::{
    constants,
    world::{Block, BlockPos, World},
};
use tokio::time::Duration;

pub type FromConsole = UnboundedReceiver<Command>;
pub type ToConsole = UnboundedSender<Log>;

pub enum Command {
    Mspt,
    Tps,
    Players,
    SetBlock { pos: BlockPos },
    GetBlock { x: u32, y: u32 },
}

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Invalid Command `{0}`")]
    InvalidCommand(String),
    #[error("Missing Argument: {0}")]
    MissingArgument(String),
    #[error("Wrong type for argument {arg}: {err:?}")]
    ArgParseErrorInt {
        arg: String,
        #[source]
        err: ParseIntError,
    },
    #[error("Unknown Block Type")]
    ArgParseErrorBlock(#[from] ParseError),
}

impl FromStr for Command {
    type Err = CommandError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens = s.split(" ");
        match tokens
            .next()
            .ok_or(CommandError::InvalidCommand("".to_string()))?
        {
            "mspt" => Ok(Command::Mspt),
            "tps" => Ok(Command::Tps),
            "players" => Ok(Command::Players),
            "block_at" => {
                let x = tokens
                    .next()
                    .ok_or(CommandError::MissingArgument("x".to_string()))?
                    .parse::<u32>()
                    .map_err(|err| CommandError::ArgParseErrorInt {
                        arg: "x".to_string(),
                        err,
                    })?;
                let y = tokens
                    .next()
                    .ok_or(CommandError::MissingArgument("y".to_string()))?
                    .parse::<u32>()
                    .map_err(|err| CommandError::ArgParseErrorInt {
                        arg: "y".to_string(),
                        err,
                    })?;
                Ok(Command::GetBlock { x, y })
            }
            "set" => {
                let x = tokens
                    .next()
                    .ok_or(CommandError::MissingArgument("x".to_string()))?
                    .parse::<u32>()
                    .map_err(|err| CommandError::ArgParseErrorInt {
                        arg: "x".to_string(),
                        err,
                    })?;
                let y = tokens
                    .next()
                    .ok_or(CommandError::MissingArgument("x".to_string()))?
                    .parse::<u32>()
                    .map_err(|err| CommandError::ArgParseErrorInt {
                        arg: "y".to_string(),
                        err,
                    })?;
                let block = Block::from_str(
                    tokens
                        .next()
                        .ok_or(CommandError::MissingArgument("block".to_string()))?,
                )?;

                Ok(Command::SetBlock { pos: (x, y, block) })
            }
            c => return Err(CommandError::InvalidCommand(c.to_string())),
        }
    }
}

pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}
pub struct Log(pub LogLevel, pub String);

#[macro_export]
macro_rules! c_info {
    ($to_console: expr, $($arg:tt)+) => {
        match $to_console.send($crate::console::Log($crate::console::LogLevel::Info, format!($($arg)+))) {
            Ok(_) => (),
            Err(_) => log::info!($($arg)+)
        }
    };
}

#[macro_export]
macro_rules! c_debug {
    ($to_console: expr, $($arg:tt)+) => {
        match $to_console.send($crate::console::Log($crate::console::LogLevel::Debug, format!($($arg)+))) {
            Ok(_) => (),
            Err(_) => log::debug!($($arg)+)
        }
    };
}

#[macro_export]
macro_rules! c_warn {
    ($to_console: expr, $($arg:tt)+) => {
        match $to_console.send($crate::console::Log($crate::console::LogLevel::Warn, format!($($arg)+))) {
            Ok(_) => (),
            Err(_) => log::warn!($($arg)+)
        }
    };
}

#[macro_export]
macro_rules! c_error {
    ($to_console: expr, $($arg:tt)+) => {
        match $to_console.send($crate::console::Log($crate::console::LogLevel::Error, format!($($arg)+))) {
            Ok(_) => (),
            Err(_) => log::error!($($arg)+)
        }
    };
}

pub fn init(console_enabled: bool) -> (FromConsole, ToConsole) {
    let (to_main, from_console) = mpsc::unbounded_channel::<Command>();
    // if console_enabled is false, simply keep the channel open but don't send messages
    let (to_console, from_main) = mpsc::unbounded_channel::<Log>();
    tokio::spawn(async move {
        let (send, mut recv) = (to_main, from_main);
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

        debug!("console thread started");

        while let Some(msg) = recv.recv().await {
            match msg.0 {
                LogLevel::Debug => debug!("{}", msg.1),
                LogLevel::Info => info!("{}", msg.1),
                LogLevel::Warn => warn!("{}", msg.1),
                LogLevel::Error => error!("{}", msg.1),
            }
        }
    });
    (from_console, to_console)
}

pub async fn process_command(
    to_console: ToConsole,
    socket: &UdpSocket,
    world: &mut World,
    command: Command,
    tick_times_saved: [Duration; 8],
    last_tick_time: Duration
) -> io::Result<()> {
    match command {
        Command::Mspt => {
            c_info!(
                to_console,
                "Tick Averages: {}ms ({:?}) last tick | {}ms ({:?}) 1s | {}ms ({:?}) 5s | {}ms ({:?}) 10s | {}ms ({:?}) 30s | {}ms ({:?}) 1m | {}ms ({:?}) 2m | {}ms ({:?}) 5m | {}ms ({:?}) 10m",
                last_tick_time.as_millis(), last_tick_time,
                tick_times_saved[0].as_millis(), tick_times_saved[0],
                tick_times_saved[1].as_millis(), tick_times_saved[1],
                tick_times_saved[2].as_millis(), tick_times_saved[2],
                tick_times_saved[3].as_millis(), tick_times_saved[3],
                tick_times_saved[4].as_millis(), tick_times_saved[4],
                tick_times_saved[5].as_millis(), tick_times_saved[5],
                tick_times_saved[6].as_millis(), tick_times_saved[6],
                tick_times_saved[7].as_millis(), tick_times_saved[7]
            );
        }
        Command::Tps => {
            macro_rules! tps {
                ($avg_tick_ms: expr) => {
                    1000u128 / std::cmp::max($avg_tick_ms.as_millis(), 1000u128 / (constants::TICKS_PER_SECOND as u128))
                };
            }
            c_info!(
                to_console,
                "Ticks Per Second Averages: {} TPS last tick | {} TPS 1s | {} TPS 5s | {} TPS 10s | {} TPS 30s | {} TPS 1m | {} TPS 2m | {} TPS 5m | {} TPS 10m",
                tps!(last_tick_time),
                tps!(tick_times_saved[0]),
               tps!(tick_times_saved[1]), 
               tps!(tick_times_saved[2]), 
               tps!(tick_times_saved[3]), 
               tps!(tick_times_saved[4]), 
               tps!(tick_times_saved[5]), 
               tps!(tick_times_saved[6]), 
               tps!(tick_times_saved[7]), 
            );
        }
        Command::Players => {
            c_info!(
                to_console,
                "There are {} players online:",
                world.players.len()
            );
            world.players.iter().for_each(|player| {
                c_info!(
                    to_console,
                    "  {}: {} (addr: {}) at ({}, {})",
                    player.id,
                    player.name.clone(),
                    player.addr,
                    player.server_player.x,
                    player.server_player.y
                );
            });
        }
        Command::SetBlock { pos } => {
            let (x, y, block) = pos;
            match world
                .set_block_and_notify(to_console.clone(), socket, x, y, block)
                .await
            {
                Ok(_) => c_info!(to_console, "Set block at ({x}, {y}) to {block:?}"),
                Err(e) => c_error!(to_console, "Cannot set block at ({x}, {y}): {e}"),
            };
        }
        Command::GetBlock { x, y } => {
            match world.get_block(x, y) {
                Ok(bl) => c_info!(to_console, "{bl:?} at ({x}, {y})"),
                Err(e) => c_error!(to_console, "Cannot get block at ({x}, {y}): {e}"),
            };
        }
    }
    Ok(())
}
