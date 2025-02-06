use log::{debug, error, info, warn};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::world::BlockPos;

pub type FromConsole = UnboundedReceiver<Command>;
pub type ToConsole = UnboundedSender<Log>;

pub enum Command {
    Mspt,
    Tps,
    PlayersOnline,
    SetBlock(BlockPos),
    GetBlock(u32, u32),
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
