use std::{collections::HashMap, fmt::Display, io, num::ParseIntError, str::FromStr};

use color_eyre::owo_colors::OwoColorize;
use crossterm::event::{Event, EventStream, KeyCode};
use futures::{FutureExt, StreamExt};
use log::{debug, error, info, warn, LevelFilter};
use ratatui::{
    layout::{Constraint, Layout},
    style::Stylize,
    text::{Line, Text},
    widgets::{self, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    DefaultTerminal, Frame,
};
use strum::ParseError;
use thiserror::Error;
use tokio::{
    net::UdpSocket,
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time,
};

use crate::{
    constants,
    world::{self, BlockPos, World},
};
use tokio::time::Duration;

pub type FromConsole = UnboundedReceiver<Command>;
pub type ToConsole = UnboundedSender<ToConsoleType>;

#[derive(PartialEq, Clone)]
pub enum Command {
    Shutdown,
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
            "exit" => Ok(Command::Shutdown),
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
                let block = world::Block::from_str(
                    tokens
                        .next()
                        .ok_or(CommandError::MissingArgument("block".to_string()))?,
                )?;

                Ok(Command::SetBlock { pos: (x, y, block) })
            }
            c => Err(CommandError::InvalidCommand(c.to_string())),
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
#[derive(Default)]
pub struct Stats {
    uptime: Duration,
    tps: u32,
    mspt: Duration,
    players: u32,
}
pub enum ToConsoleType {
    Log(Log),
    Stats(Stats),
}

#[macro_export]
macro_rules! c_info {
    ($to_console: expr, $($arg:tt)+) => {
        match $to_console.send($crate::console::ToConsoleType::Log($crate::console::Log($crate::console::LogLevel::Info, format!($($arg)+)))) {
            Ok(_) => (),
            Err(_) => log::info!($($arg)+)
        }
    };
}

#[macro_export]
macro_rules! c_debug {
    ($to_console: expr, $($arg:tt)+) => {
        match $to_console.send($crate::console::ToConsoleType::Log($crate::console::Log($crate::console::LogLevel::Debug, format!($($arg)+)))) {
            Ok(_) => (),
            Err(_) => log::debug!($($arg)+)
        }
    };
}

#[macro_export]
macro_rules! c_warn {
    ($to_console: expr, $($arg:tt)+) => {
        match $to_console.send($crate::console::ToConsoleType::Log($crate::console::Log($crate::console::LogLevel::Warn, format!($($arg)+)))) {
            Ok(_) => (),
            Err(_) => log::warn!($($arg)+)
        }
    };
}

#[macro_export]
macro_rules! c_error {
    ($to_console: expr, $($arg:tt)+) => {
        match $to_console.send($crate::console::ToConsoleType::Log($crate::console::Log($crate::console::LogLevel::Error, format!($($arg)+)))) {
            Ok(_) => (),
            Err(_) => log::error!($($arg)+)
        }
    };
}

pub fn init(console_enabled: bool, debug: bool) -> (FromConsole, ToConsole) {
    let (to_main, from_console) = mpsc::unbounded_channel::<Command>();
    // if console_enabled is false, simply keep the channel open but don't send messages
    let (to_console, from_main) = mpsc::unbounded_channel::<ToConsoleType>();
    tokio::spawn(async move {
        let (send, mut recv) = (to_main, from_main);
        if console_enabled {
            match RatatuiConsole::init(send, recv, debug).await {
                Ok(_) => (),
                Err(e) => error!("ratatui Console failed: {e}"),
            };
        } else {
            env_logger::Builder::new()
                .filter_level({
                    if debug {
                        LevelFilter::Debug
                    } else {
                        LevelFilter::Info
                    }
                })
                .init();

            debug!("console thread started");

            while let Some(message) = recv.recv().await {
                match message {
                    ToConsoleType::Log(log) => match log.0 {
                        LogLevel::Debug => debug!("{}", log.1),
                        LogLevel::Info => info!("{}", log.1),
                        LogLevel::Warn => warn!("{}", log.1),
                        LogLevel::Error => error!("{}", log.1),
                    },
                    ToConsoleType::Stats(_) => {
                        warn!("stats message received but console is not enabled!")
                    }
                }
            }
        }
    });
    (from_console, to_console)
}

struct RatatuiConsole<'a> {
    scroll_state: ScrollbarState,
    scroll: usize,
    to_main: UnboundedSender<Command>,
    from_main: UnboundedReceiver<ToConsoleType>,
    debug: bool,
    stats: Stats,
    logs: Text<'a>,
    command_input: String,
}

impl RatatuiConsole<'_> {
    async fn init(
        to_main: UnboundedSender<Command>,
        from_main: UnboundedReceiver<ToConsoleType>,
        debug: bool,
    ) -> color_eyre::Result<()> {
        color_eyre::install()?;
        let terminal = ratatui::init();
        let console = RatatuiConsole {
            scroll_state: Default::default(),
            scroll: Default::default(),
            to_main,
            from_main,
            debug,
            stats: Default::default(),
            logs: Default::default(),
            command_input: Default::default(),
        };
        let run = console.run(terminal).await;
        ratatui::restore();
        run
    }

    async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        let mut update_tick =
            time::interval(Duration::from_millis(constants::CONSOLE_UPDATE_RATE_MS));
        let mut input_events = EventStream::new();
        loop {
            let mut event = input_events.next().fuse();
            tokio::select! {
                msg = self.from_main.recv() => {
                    match msg {
                        Some(ToConsoleType::Log(log)) => match log.0 {
                            LogLevel::Debug => if self.debug {
                                self.logs.lines.push(Line::from(vec![
                                    "DEBUG".blue().bold(),
                                    " ".into(),
                                    log.1.into()
                                ]))
                            },
                            LogLevel::Info => self.logs.lines.push(Line::from(vec![
                                    "INFO".green().bold(),
                                    " ".into(),
                                    log.1.into()
                            ])),
                            LogLevel::Warn => self.logs.lines.push(Line::from(vec![
                                    "WARN".yellow().bold(),
                                    " ".into(),
                                    log.1.into()
                            ])),
                            LogLevel::Error => self.logs.lines.push(Line::from(vec![
                                    "ERROR".red().bold(),
                                    " ".into(),
                                    log.1.into()
                            ])),
                        },
                        Some(ToConsoleType::Stats(stats)) => self.stats = stats,
                        None => {
                            break Ok(());
                        }
                    }
                }
                _ = update_tick.tick() => {
                    terminal.draw(|frame| self.draw(frame))?;
                }
                maybe_event = event => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            if let Event::Key(key) = event {
                                match key.code {
                                    KeyCode::Char(c) => self.command_input.push(c),
                                    KeyCode::Backspace => {self.command_input.pop();},
                                    KeyCode::Enter => {
                                        self.logs.lines.push(Line::from(format!("<- {}", self.command_input.clone())));
                                        let command = match Command::from_str(&self.command_input) {
                                            Ok(c) => Some(c),
                                            Err(e) => {
                                                self.logs.lines.push(Line::from(vec![
                                                    "ERROR".red().bold(),
                                                    " ".into(),
                                                    format!("{e}").into()
                                                ]));
                                                None
                                            }
                                        };
                                        self.command_input.clear();
                                        if let Some(comm) = command {
                                            let _ = self.to_main.send(comm.clone());
                                            if comm == Command::Shutdown {
                                                break Ok(());
                                            }
                                        }
                                    },
                                    _ => {}
                                }
                            }
                        },
                        Some(Err(e)) => {

                        },
                        None => {
                            break Ok(());
                        }
                    }
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let layout =
            Layout::vertical([Constraint::Percentage(100), Constraint::Min(3)]).split(area);

        self.scroll_state = self.scroll_state.content_length(self.logs.lines.len());

        let log_paragraph = Paragraph::new(self.logs.clone())
            .gray()
            .wrap(Wrap { trim: true })
            .block(widgets::Block::bordered().gray().title("Logs"))
            .scroll((self.scroll as u16, 0));
        frame.render_widget(log_paragraph, layout[0]);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            layout[0],
            &mut self.scroll_state,
        );

        let input = Paragraph::new(self.command_input.clone())
            .gray()
            .block(widgets::Block::bordered().gray().title("Command Input"));
        frame.render_widget(input, layout[1]);
    }
}

pub async fn process_command(
    to_console: ToConsole,
    socket: &UdpSocket,
    world: &mut World,
    command: Command,
    tick_times_saved: [Duration; 8],
    last_tick_time: Duration,
) -> io::Result<bool> {
    match command {
        Command::Shutdown => {
            world.shutdown(to_console, socket).await?;
            return Ok(true);
        }
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
                    1000u128
                        / std::cmp::max(
                            $avg_tick_ms.as_millis(),
                            1000u128 / (constants::TICKS_PER_SECOND as u128),
                        )
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
    Ok(false)
}
