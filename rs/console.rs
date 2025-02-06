use std::{collections::HashMap, num::ParseIntError, str::FromStr};

use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::world::{Block, BlockPos};

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
    #[error("Wrong type for argument {0}: {1:?}")]
    ArgParseError {
        arg: String,
        #[source]
        err: ParseIntError,
    },
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
                    .map_err(|err| CommandError::ArgParseError {
                        arg: "x".to_string(),
                        err,
                    })?;
                let y = tokens
                    .next()
                    .ok_or(CommandError::MissingArgument("y".to_string()))?
                    .parse::<u32>()
                    .map_err(|err| CommandError::ArgParseError {
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
                    .map_err(|err| CommandError::ArgParseError {
                        arg: "x".to_string(),
                        err,
                    })?;
                let y = tokens
                    .next()
                    .ok_or(CommandError::MissingArgument("x".to_string()))?
                    .parse::<u32>()
                    .map_err(|err| CommandError::ArgParseError {
                        arg: "y".to_string(),
                        err,
                    })?;
                let block = tokens
                    .next()
                    .ok_or(CommandError::MissingArgument("block"))?
                    .parse::<Block>();
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

pub fn init() -> (FromConsole, ToConsole) {
    let (to_main, from_console) = mpsc::unbounded_channel::<Command>();
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
