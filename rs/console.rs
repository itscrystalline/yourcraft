use crossterm::event::EventStream;
use futures::{FutureExt, StreamExt};
use log::{debug, error, info, warn};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{self, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    DefaultTerminal, Frame,
};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::{
    io,
    num::{NonZeroU32, ParseFloatError, ParseIntError},
    str::FromStr,
};
use thiserror::Error;
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
    time,
};
use tui_textarea::{Input, Key, TextArea};

use crate::{
    constants,
    network::ToNetwork,
    player::{Acceleration, Player, Velocity},
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
    Kick(u32, String),
    Teleport { id: u32, x: f32, y: f32 },
    Respawn(u32),
    SetBlock { pos: BlockPos },
    GetBlock { x: u32, y: u32 },
    SetSpawn(u32),
    SetSpawnRange(NonZeroU32),
    InventorySee(u32),
}

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Invalid Command `{0}`")]
    InvalidCommand(String),
    #[error("Missing Argument: {0}")]
    MissingArgument(String),
    #[error("Wrong type for argument {arg}: {err}")]
    ArgParseError {
        arg: String,
        #[source]
        err: ArgParseError,
    },
}

#[derive(Error, Debug)]
pub enum ArgParseError {
    #[error("Cannot parse int: `{0}`")]
    Int(#[from] ParseIntError),
    #[error("Value cannot be zero!")]
    ZeroInt,
    #[error("Cannot parse float: `{0}`")]
    Float(#[from] ParseFloatError),
    #[error("Cannot parse Block: `{0}`")]
    Block(#[from] strum::ParseError),
}

macro_rules! next_type_token_or_err {
    ($tokens: expr, $property_name: expr, $type: ty) => {
        next_token!($tokens, $property_name)
            .parse::<$type>()
            .map_err(|err| CommandError::ArgParseError {
                arg: $property_name.to_string(),
                err: err.into(),
            })?
    };
}

macro_rules! next_token {
    ($tokens: expr, $property_name: expr) => {
        $tokens
            .next()
            .ok_or(CommandError::MissingArgument($property_name.to_string()))?
    };
}

impl FromStr for Command {
    type Err = CommandError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens = s.trim().split(" ");
        match tokens
            .next()
            .ok_or(CommandError::InvalidCommand("".to_string()))?
        {
            "help" | "h" | "?" => Ok(Command::Help),
            "exit" | "stop" => Ok(Command::Shutdown),
            "mspt" => Ok(Command::Mspt),
            "tps" => Ok(Command::Tps),
            "players" | "p" => Ok(Command::Players),
            "kick" => {
                let player_id = next_type_token_or_err!(tokens, "player_id", u32);
                let reason = next_token!(tokens, "reason");
                Ok(Command::Kick(player_id, reason.to_string()))
            }
            "respawn" => {
                let id = next_type_token_or_err!(tokens, "player_id", u32);
                Ok(Command::Respawn(id))
            }
            "teleport" | "tp" => {
                let id = next_type_token_or_err!(tokens, "player_id", u32);
                let x = next_type_token_or_err!(tokens, "x", f32);
                let y = next_type_token_or_err!(tokens, "y", f32);
                Ok(Command::Teleport { id, x, y })
            }
            "get" | "block_at" => {
                let x = next_type_token_or_err!(tokens, "x", u32);
                let y = next_type_token_or_err!(tokens, "y", u32);
                Ok(Command::GetBlock { x, y })
            }
            "set" => {
                let x = next_type_token_or_err!(tokens, "x", u32);
                let y = next_type_token_or_err!(tokens, "y", u32);
                let block = world::Block::from_str(next_token!(tokens, "block")).map_err(|e| {
                    CommandError::ArgParseError {
                        arg: "block".to_string(),
                        err: ArgParseError::Block(e),
                    }
                })?;

                Ok(Command::SetBlock { pos: (x, y, block) })
            }
            "spawn" => {
                let x = next_type_token_or_err!(tokens, "x", u32);
                Ok(Command::SetSpawn(x))
            }
            "spawn_range" => {
                let range = next_type_token_or_err!(tokens, "range", u32);
                let nonzero_range = NonZeroU32::new(range).ok_or(CommandError::ArgParseError {
                    arg: "range".to_string(),
                    err: ArgParseError::ZeroInt,
                })?;
                Ok(Command::SetSpawnRange(nonzero_range))
            }
            "invsee" | "inventorysee" => {
                let id = next_type_token_or_err!(tokens, "player_id", u32);
                Ok(Command::InventorySee(id))
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
    Quit,
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

pub fn init(console_enabled: bool, debug: bool) -> (JoinHandle<()>, FromConsole, ToConsole) {
    let (to_main, from_console) = mpsc::unbounded_channel::<Command>();
    // if console_enabled is false, simply keep the channel open but don't send messages
    let (to_console, from_main) = mpsc::unbounded_channel::<ToConsoleType>();
    let console_thread = tokio::spawn(async move {
        let (send, mut recv) = (to_main, from_main);
        if console_enabled {
            match RatatuiConsole::init(send, recv, debug).await {
                Ok(logs) => {
                    logs.into_iter().for_each(|msg| println!("{msg}"));
                }
                Err(e) => error!("ratatui Console failed: {e}"),
            };
        } else {
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
                    ToConsoleType::Quit => break,
                }
            }
        }
    });
    (console_thread, from_console, to_console)
}

struct RatatuiConsole<'a> {
    scroll_state: ScrollbarState,
    scroll: u16,
    to_main: UnboundedSender<Command>,
    from_main: UnboundedReceiver<ToConsoleType>,
    debug: bool,
    autoscroll: bool,
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
    ) -> color_eyre::Result<Vec<String>> {
        color_eyre::install()?;
        let terminal = ratatui::init();
        let console = RatatuiConsole {
            scroll_state: Default::default(),
            scroll: Default::default(),
            to_main,
            from_main,
            debug,
            autoscroll: true,
            stats: Default::default(),
            consle_rect: (0, 0),
            logs: Default::default(),
            command_input: Default::default(),
        };
        let run = console.run(terminal).await;
        ratatui::restore();
        run
    }

    async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<Vec<String>> {
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
                        break Ok(self.get_logs_str());
                    }
                }
                _ = update_tick.tick() => {
                    terminal.draw(|frame| self.draw(frame))?;
                }
                maybe_event = event => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            if self.process_terminal_events(event.into()) {
                                 break Ok(self.get_logs_str());
                            }
                        },
                        Some(Err(_)) => {/*silently discard*/},
                        None => {
                             break Ok(self.get_logs_str());
                        }
                    }
                }
            }
        }
    }

    fn get_logs_str(self) -> Vec<String> {
        self.logs.into_iter().map(|line| line.to_string()).collect()
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
            Some(ToConsoleType::Quit) | None => return true,
        }
        if self.autoscroll {
            self.scroll = self.calculate_line_heights();
        }
        false
    }

    fn calculate_line_heights(&self) -> u16 {
        let (width, _) = self.consle_rect;
        if width > 2 {
            self.logs
                .lines
                .par_iter()
                .map(|l| (l.width() as u16 / (width - 2)) + 1)
                .sum()
        } else {
            self.logs.lines.len() as u16
        }
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
            Input { key: Key::Tab, .. } => {
                self.autoscroll = !self.autoscroll;
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
                } else {
                    self.scroll = self.calculate_line_heights()
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

        let autoscroll = if self.autoscroll {
            "ON".green()
        } else {
            "OFF".red()
        };

        let log_paragraph = Paragraph::new(self.logs.clone())
            .gray()
            .wrap(Wrap { trim: true })
            .block(
                widgets::Block::bordered()
                    .gray()
                    .title(" Logs ".bold().into_left_aligned_line())
                    .title(
                        format!(" Up for {} ", humantime::format_duration(self.stats.uptime))
                            .bold()
                            .into_centered_line(),
                    )
                    .title(
                        Line::from(vec![
                            format!(" {} TPS", self.stats.tps).bold(),
                            " ".into(),
                            format!("({:?}/t)", self.stats.mspt).into(),
                            " ".into(),
                            format!("{} Online", self.stats.players).bold(),
                            " ─".into(),
                        ])
                        .right_aligned(),
                    )
                    .title_bottom(
                        Line::from(vec!["Autoscroll (↹ ): ".into(), autoscroll]).centered(),
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
    to_network: ToNetwork,
    world: &mut World,
    command: Command,
    tick_times_saved: [Duration; 8],
    last_tick_time: Duration,
    phys_last_tick_time: Duration,
) -> io::Result<bool> {
    match command {
        Command::Help => {
            c_info!(to_console, "Commands: help/h/?, exit/stop, tps, mspt, players/p, respawn (player_id), kick (player_id), teleport/tp (player_id, x, y) block_at/get (x, y), set (x, y, Block), spawn (x), spawn_range (range), inventorysee/invsee (player_id)");
        }
        Command::Shutdown => {
            return Ok(true);
        }
        Command::Mspt => {
            c_info!(
                to_console,
                "Physics: {}ms ({:?}) last tick",
                phys_last_tick_time.as_millis(),
                phys_last_tick_time
            );
            c_info!(
                to_console,
                "World Tick Averages: {}ms ({:?}) last tick | {}ms ({:?}) 1s | {}ms ({:?}) 5s | {}ms ({:?}) 10s | {}ms ({:?}) 30s | {}ms ({:?}) 1m | {}ms ({:?}) 2m | {}ms ({:?}) 5m | {}ms ({:?}) 10m",
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
                "Physics: {} TPS last tick",
                1000u128
                    / std::cmp::max(
                        phys_last_tick_time.as_millis(),
                        1000u128 / constants::PHYS_TICKS_PER_SECOND as u128
                    ),
            );
            c_info!(
                to_console,
                "World TPS Averages: {} TPS last tick | {} TPS 1s | {} TPS 5s | {} TPS 10s | {} TPS 30s | {} TPS 1m | {} TPS 2m | {} TPS 5m | {} TPS 10m",
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
                    "  {}: {} (addr: {}) at ({}, {}) velocity: {:?} acceleration: {:?}",
                    player.id,
                    player.name.clone(),
                    player.addr,
                    player.server_player.x,
                    player.server_player.y,
                    player.server_player.velocity,
                    player.server_player.acceleration
                );
            });
        }
        Command::Kick(id, msg) => {
            world.kick(to_console, to_network, id, Some(&msg)).await?;
        }
        Command::Respawn(id) => {
            let idx_maybe = world.players.par_iter().position_any(|conn| conn.id == id);
            if let Some(idx) = idx_maybe {
                let spawn = world.get_spawn();
                let old_player = &world.players[idx];
                let (old_x, old_y) = (old_player.server_player.x, old_player.server_player.y);
                world.players[idx].server_player = match Player::spawn_at(world, spawn) {
                    Ok(new) => new,
                    Err(e) => {
                        c_error!(to_console, "error spawning new player: {e}");
                        return Ok(false);
                    }
                };
                world.notify_player_moved(to_network, &world.players[idx].clone(), old_x, old_y)?;
            } else {
                c_error!(to_console, "Player doesn't exist.")
            }
        }
        Command::Teleport { id, x, y } => {
            let idx_maybe = world.players.par_iter().position_any(|conn| conn.id == id);
            if let Some(idx) = idx_maybe {
                let (old_x, old_y) = (
                    world.players[idx].server_player.x,
                    world.players[idx].server_player.y,
                );

                world.players[idx].server_player.x = x;
                world.players[idx].server_player.y = y;
                world.players[idx].server_player.velocity = Velocity::default();
                world.players[idx].server_player.acceleration = Acceleration::default();
                world.notify_player_moved(to_network, &world.players[idx].clone(), old_x, old_y)?;

                c_info!(
                    to_console,
                    "Teleported {} (id {id}) to ({x}, {y})",
                    world.players[idx].name
                )
            } else {
                c_error!(to_console, "Player {id} does not exist!")
            }
        }
        Command::SetBlock { pos } => {
            let (x, y, block) = pos;
            match world.set_block_and_notify(to_network, x, y, block).await {
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
        Command::SetSpawn(x) => match world.set_spawn(x) {
            Ok(_) => c_info!(to_console, "World spawn set to x: {x}"),
            Err(e) => c_error!(to_console, "cannot set spawn to x: {x}: {e}"),
        },
        Command::SetSpawnRange(range) => match world.set_spawn_range(range) {
            Ok(_) => c_info!(
                to_console,
                "Spawn range set to {range} blocks around x: {}",
                world.spawn_point
            ),
            Err(e) => c_error!(to_console, "cannot set spawn range to {range}: {e}"),
        },
        Command::InventorySee(id) => {
            match world.players.par_iter().find_any(|conn| conn.id == id) {
                Some(player) => {
                    let player_inv = player.server_player.inventory;
                    c_info!(
                        to_console,
                        "Inventory of {} (id {}): {:?}",
                        player.name,
                        player.id,
                        player_inv
                    );
                }
                None => c_error!(to_console, "Player doesn't exist."),
            }
        }
    }
    Ok(false)
}
