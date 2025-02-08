use color_eyre::owo_colors::OwoColorize;
use crossterm::event::EventStream;
use futures::{FutureExt, StreamExt};
use log::{debug, error, info, warn, LevelFilter};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{self, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    DefaultTerminal, Frame,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{io, num::ParseIntError, str::FromStr};
use strum::ParseError;
use thiserror::Error;
use tokio::{
    net::UdpSocket,
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time,
};
use tui_textarea::{Input, Key, TextArea};

use crate::{
    constants,
    world::{self, BlockPos, World},
};
use tokio::time::Duration;

pub type FromConsole = UnboundedReceiver<Command>;
pub type ToConsole = UnboundedSender<ToConsoleType>;

#[derive(PartialEq, Clone)]
pub enum Command {
    Help,
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
            "help" | "h" | "?" => Ok(Command::Help),
            "exit" | "stop" => Ok(Command::Shutdown),
            "mspt" => Ok(Command::Mspt),
            "tps" => Ok(Command::Tps),
            "players" => Ok(Command::Players),
            "get" | "block_at" => {
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
    pub uptime: Duration,
    pub tps: u128,
    pub mspt: Duration,
    pub players: usize,
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
    scroll: u16,
    to_main: UnboundedSender<Command>,
    from_main: UnboundedReceiver<ToConsoleType>,
    debug: bool,
    stats: Stats,
    consle_rect: (u16, u16),
    logs: Text<'a>,
    command_input: TextArea<'a>,
}

macro_rules! log_console {
    ($self: expr, $header: expr, $msg: expr) => {
        $self.logs.lines.push(Line::from(vec![
            format!(
                "{}",
                humantime::format_rfc3339_seconds(std::time::SystemTime::now())
            )
            .into(),
            " ".into(),
            $header.bold(),
            " ".into(),
            $msg.into(),
        ]))
    };
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
            consle_rect: (0, 0),
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

        self.command_input.set_cursor_line_style(Style::default());
        self.command_input.set_block(
            widgets::Block::bordered()
                .gray()
                .title("Input Command (type `?` for commands)")
                .border_type(widgets::BorderType::Rounded),
        );

        loop {
            let event = input_events.next().fuse();
            tokio::select! {
                msg = self.from_main.recv() => {
                    if self.process_from_main(msg) {
                        break Ok(());
                    }
                }
                _ = update_tick.tick() => {
                    terminal.draw(|frame| self.draw(frame))?;
                }
                maybe_event = event => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            if self.process_terminal_events(event.into()) {
                                break Ok(());
                            }
                        },
                        Some(Err(_)) => {/*silently discard*/},
                        None => {
                            break Ok(());
                        }
                    }
                }
            }
        }
    }

    fn process_from_main(&mut self, msg: Option<ToConsoleType>) -> bool {
        match msg {
            Some(ToConsoleType::Log(log)) => match log.0 {
                LogLevel::Debug => {
                    if self.debug {
                        log_console!(self, "DEBUG".blue(), log.1);
                    }
                }
                LogLevel::Info => log_console!(self, "INFO".green(), log.1),
                LogLevel::Warn => log_console!(self, "WARN".yellow(), log.1),
                LogLevel::Error => log_console!(self, "ERROR".red(), log.1),
            },
            Some(ToConsoleType::Stats(stats)) => self.stats = stats,
            None => return true,
        }
        false
    }

    fn calculate_line_heights(&self) -> u16 {
        let (width, _) = self.consle_rect;
        self.logs
            .lines
            .par_iter()
            .map(|l| (l.width() as u16 / (width - 2)) + 1)
            .sum()
    }

    fn process_terminal_events(&mut self, input: Input) -> bool {
        match input {
            Input {
                key: Key::Up,
                shift,
                ..
            } => {
                let scroll_lines = if shift { 10 } else { 1 };
                self.scroll = self.scroll.saturating_sub(scroll_lines);
            }
            Input {
                key: Key::Down,
                shift,
                ..
            } => {
                let scroll_lines = if shift { 10 } else { 1 };
                self.scroll = self.scroll.saturating_add(scroll_lines);
            }
            Input {
                key: Key::Char('c'),
                ctrl: true,
                ..
            } => {
                let _ = self.to_main.send(Command::Shutdown);
                return true;
            }
            Input {
                key: Key::Enter, ..
            } => {
                if !self.command_input.is_empty() {
                    self.logs.lines.push(Line::from(vec![
                        "<- ".bold(),
                        self.command_input.lines()[0].clone().bold(),
                    ]));
                    self.scroll = self.calculate_line_heights().saturating_add(1);
                    let cmd = match Command::from_str(&self.command_input.lines()[0]) {
                        Ok(c) => Some(c),
                        Err(e) => {
                            log_console!(self, "ERROR".red(), format!("{e}"));
                            None
                        }
                    };
                    self.command_input
                        .move_cursor(tui_textarea::CursorMove::End);
                    self.command_input.delete_line_by_head();
                    if let Some(command) = cmd {
                        let exit = command == Command::Shutdown;
                        let _ = self.to_main.send(command);
                        return exit;
                    }
                }
            }
            input => {
                self.command_input.input(input);
            }
        }
        false
    }

    fn validate_command(&mut self) {
        if self.command_input.is_empty() {
            self.command_input.set_block(
                widgets::Block::bordered()
                    .gray()
                    .title("Input Command (type `?` for commands)")
                    .border_type(widgets::BorderType::Rounded),
            );
        } else {
            let comm = Command::from_str(&self.command_input.lines()[0]);
            if let Err(e) = comm {
                self.command_input.set_block(
                    widgets::Block::bordered()
                        .red()
                        .title(format!("{e} (type `?` for commands)"))
                        .border_type(widgets::BorderType::Rounded)
                        .border_style(Style::new().bold()),
                );
            } else {
                self.command_input.set_block(
                    widgets::Block::bordered()
                        .green()
                        .title("Input Command")
                        .border_type(widgets::BorderType::Rounded)
                        .border_style(Style::new().bold()),
                );
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let layout =
            Layout::vertical([Constraint::Percentage(100), Constraint::Min(3)]).split(area);

        self.consle_rect = (layout[0].width, layout[0].height);

        self.scroll_state = self.scroll_state.content_length(self.logs.lines.len());

        let actual_scroll_position = self.scroll.saturating_sub(self.consle_rect.1 - 2);
        self.scroll_state = self.scroll_state.position(actual_scroll_position.into());

        let log_paragraph = Paragraph::new(self.logs.clone())
            .gray()
            .wrap(Wrap { trim: true })
            .block(
                widgets::Block::bordered()
                    .gray()
                    .title("Logs".bold().into_left_aligned_line())
                    .title(
                        format!("Up for {}", humantime::format_duration(self.stats.uptime))
                            .bold()
                            .into_centered_line(),
                    )
                    .title(
                        Line::from(vec![
                            format!("{} TPS", self.stats.tps).bold(),
                            " ".into(),
                            format!("({:?}/t)", self.stats.mspt).into(),
                            " ".into(),
                            format!("{} Online", self.stats.players).bold(),
                            "─".into(),
                        ])
                        .right_aligned(),
                    )
                    .border_type(widgets::BorderType::Rounded),
            )
            .scroll((actual_scroll_position, 0));
        frame.render_widget(log_paragraph, layout[0]);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            layout[0],
            &mut self.scroll_state,
        );

        self.validate_command();
        frame.render_widget(&self.command_input, layout[1]);
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
        Command::Help => {
            c_info!(to_console, "Commands: help/h/?, exit/stop, tps, mspt, players, block_at/get (x, y), set (x, y, Block)");
        }
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
