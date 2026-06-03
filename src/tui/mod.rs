use std::io::{self, Read, Write};
use std::sync::mpsc::{self, Sender};
use std::{fs, thread};

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{
        Block, Borders, Gauge, List, ListItem, ListState, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};

use crate::des_ssh::{self, create_ssh_session};

enum DesEvent {
    Input(crossterm::event::KeyEvent),
    Progress(RomDownload),
    DownloadComplete,
}

fn handle_input_events(tx: mpsc::Sender<DesEvent>) {
    loop {
        if let Some(key_event) = event::read().unwrap().as_key_press_event() {
            tx.send(DesEvent::Input(key_event)).unwrap();
        }
    }
}

#[derive(Clone, Debug, Default)]
struct BaseRom {
    title: String,
    console: String,
}

impl BaseRom {
    fn new(title: String, console: String) -> BaseRom {
        BaseRom { title, console }
    }
}

#[derive(Debug, Default, Clone)]
struct RomItem {
    rom: BaseRom,
    is_selected: bool,
}

impl RomItem {
    fn new(title: String, console: String, is_selected: Option<bool>) -> RomItem {
        RomItem {
            rom: BaseRom::new(title, console),
            is_selected: is_selected.unwrap_or(false),
        }
    }
}

#[derive(Clone)]
struct RomDownload {
    rom: BaseRom,
    download_percent: f64,
    total_size: u64,
    total_received: u64,
}

impl RomDownload {
    fn new(
        title: String,
        console: String,
        download_percent: Option<f64>,
        total_size: Option<u64>,
    ) -> RomDownload {
        RomDownload {
            rom: BaseRom::new(title, console),
            download_percent: download_percent.unwrap_or(0.0),
            total_size: total_size.unwrap_or(0),
            total_received: 0,
        }
    }
}

#[derive(Debug, PartialEq)]
enum AppMode {
    SelectConsole,
    SelectRom,
    SelectSelection,
    Downloading,
}

struct AppState<'a> {
    sess: &'a ssh2::Session,
    consoles_list: Vec<String>,
    roms_list: Vec<RomItem>,
    highlighted_console: ListState,
    highlighted_rom: ListState,
    selected_roms: Vec<RomDownload>,
    mode: AppMode,
    current_console: String,
    exit: bool,
    event_tx: Sender<DesEvent>,
    download_scroll: usize,
}

impl<'a> AppState<'a> {
    fn new(
        sess: &'_ ssh2::Session,
        consoles_list: Vec<String>,
        event_tx: Sender<DesEvent>,
    ) -> AppState<'_> {
        AppState {
            sess,
            consoles_list,
            roms_list: vec![],
            highlighted_console: ListState::default(),
            highlighted_rom: ListState::default(),
            selected_roms: vec![],
            mode: AppMode::SelectConsole,
            current_console: String::new(),
            exit: false,
            event_tx,
            download_scroll: 0,
        }
    }

    fn is_selected_rom(&self, rom_title: &str, console: &str) -> bool {
        self.selected_roms
            .iter()
            .any(|r| r.rom.console == console && r.rom.title == rom_title)
    }

    fn toggle_rom_selected(&mut self) {
        let rom_index = self.highlighted_rom.selected().unwrap();
        if self.roms_list[rom_index].is_selected {
            // Unselect
            let rom_title = &self.roms_list[rom_index].rom.title;
            let console = &self.roms_list[rom_index].rom.console;
            if let Some(selected_rom_index) = self
                .selected_roms
                .iter()
                .position(|r| r.rom.title == *rom_title && r.rom.console == *console)
            {
                self.selected_roms.remove(selected_rom_index);
            }
            self.roms_list[rom_index].is_selected = false;
            return;
        }
        // Select
        self.roms_list[rom_index].is_selected = true;
        let rom_item = self.roms_list[rom_index].clone();

        let remote_root_path =
            dotenv::var("REMOTE_ROM_ROOT_PATH").expect("REMOTE_ROM_ROOT_PATH not set in .env");
        let rom_path = format!("/{}/{}", rom_item.rom.console, rom_item.rom.title);
        let remote_rom_path = format!("{}/{}", remote_root_path, rom_path);

        let (_remote_file, stat) = self
            .sess
            .scp_recv(std::path::Path::new(&remote_rom_path))
            .unwrap();

        // Insert ROM into selection list
        let new_rom_download = RomDownload::new(
            rom_item.rom.title.to_owned(),
            rom_item.rom.console.to_owned(),
            None,
            Some(stat.size()),
        );
        self.selected_roms.push(new_rom_download);
    }

    fn download(&mut self) {
        self.set_mode(AppMode::Downloading);
        let tx = self.event_tx.clone();
        let roms = self.selected_roms.clone();

        let thread_sess = create_ssh_session();
        thread::spawn(move || {
            let local_root_path =
                dotenv::var("LOCAL_ROM_ROOT_PATH").expect("LOCAL_ROM_ROOT_PATH not set in .env");
            let remote_root_path =
                dotenv::var("REMOTE_ROM_ROOT_PATH").expect("REMOTE_ROM_ROOT_PATH not set in .env");

            for rom_item in roms {
                let (rom, console) = (&rom_item.rom.title, &rom_item.rom.console);

                let rom_wo_extension = &rom[..rom.rfind('.').unwrap_or(rom.len())];
                let dest_path = format!("{}/{}/{}", local_root_path, console, rom_wo_extension);

                let rom_path = format!("/{}/{}", console, rom);
                let remote_rom_path = format!("{}/{}", remote_root_path, rom_path);
                let local_console_path = format!("{}/{}", local_root_path, console);
                let temp_download_dir = format!("{}/.downloading", local_console_path);
                let part_path = format!("{}/{}.part", temp_download_dir, rom);

                // Connect to the local SSH server
                let (mut remote_file, stat) = thread_sess
                    .scp_recv(std::path::Path::new(&remote_rom_path))
                    .unwrap();
                let total = stat.size() as f64;
                let mut received: u64 = 0;

                // Show a progress bar while downloading
                // Download the file via scp
                let mut buf: Vec<u8> = vec![0; 8192];

                // Open file before the loop, write chunks as they arrive, rename at the end
                fs::create_dir_all(&temp_download_dir).unwrap();
                let mut local_file = fs::File::create(&part_path).unwrap();
                let mut last_send = std::time::Instant::now();
                const THROTTLE_MS: u128 = 33;
                loop {
                    let n = remote_file.read(&mut buf).unwrap();
                    if n == 0 {
                        break;
                    }
                    local_file.write_all(&buf[..n]).unwrap();
                    received += n as u64;
                    let now = std::time::Instant::now();
                    if now.duration_since(last_send).as_millis() >= THROTTLE_MS
                        || received >= stat.size()
                    {
                        last_send = now;
                        let percent = received as f64 / total;
                        tx.send(DesEvent::Progress(RomDownload {
                            rom: BaseRom {
                                console: console.clone(),
                                title: rom.clone(),
                            },
                            download_percent: percent,
                            total_size: total as u64,
                            total_received: received,
                        }))
                        .unwrap();
                    }
                }
                // Update the progress bar

                let completed_download = format!("{}/{}", temp_download_dir, rom_wo_extension);

                des_ssh::unzip(&part_path, &completed_download);
                fs::rename(&completed_download, &dest_path).unwrap();

                // Optional: delete the zip after extraction
                fs::remove_file(&part_path).unwrap();
                fs::remove_dir_all(temp_download_dir).unwrap();

                // Close the channel and wait for the whole content to be transferred
                remote_file.send_eof().unwrap();
                remote_file.wait_eof().unwrap();
                remote_file.close().unwrap();
                remote_file.wait_close().unwrap();
                tx.send(DesEvent::DownloadComplete).unwrap();
            }
        });
    }

    fn set_mode(&mut self, new_mode: AppMode) {
        match new_mode {
            AppMode::SelectConsole => {
                self.mode = AppMode::SelectConsole;
                self.current_console = String::new();
            }
            AppMode::SelectRom => {
                self.mode = AppMode::SelectRom;
                let selected_index = self.highlighted_console.selected().unwrap();
                self.current_console = self.consoles_list[selected_index].clone();

                let existing_roms = des_ssh::list_roms(self.sess, &self.current_console);

                self.roms_list = existing_roms
                    .iter()
                    .map(|r| {
                        RomItem::new(
                            r.clone(),
                            self.current_console.clone(),
                            Some(self.is_selected_rom(r, &self.current_console)),
                        )
                    })
                    .collect();
                self.highlighted_rom.select_first();
            }
            AppMode::Downloading => {
                self.mode = AppMode::Downloading;
            }
            _ => {}
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> io::Result<()> {
        if key_event.kind == KeyEventKind::Press {
            // Global keybinds
            match key_event.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.exit = true;
                    return Ok(());
                }
                KeyCode::Char('d') => {
                    self.set_mode(AppMode::Downloading);
                    self.download();
                }
                _ => {}
            }
            // Mode-specific keybinds
            match self.mode {
                AppMode::SelectConsole => self.handle_console_select_controls(key_event)?,
                AppMode::SelectRom => self.handle_roms_select_controls(key_event)?,
                AppMode::SelectSelection => self.handle_selection_select_controls(key_event)?,
                AppMode::Downloading => self.handle_download_controls(key_event)?,
            }
        }
        Ok(())
    }

    fn handle_console_select_controls(&mut self, key_event: KeyEvent) -> io::Result<()> {
        match key_event.code {
            KeyCode::Char('k') | KeyCode::Up => self.highlighted_console.select_previous(),
            KeyCode::Char('j') | KeyCode::Down => self.highlighted_console.select_next(),
            KeyCode::Char('l') | KeyCode::Enter => self.set_mode(AppMode::SelectRom),
            _ => {}
        }
        Ok(())
    }

    fn handle_roms_select_controls(&mut self, key_event: KeyEvent) -> io::Result<()> {
        match key_event.code {
            KeyCode::Char('k') | KeyCode::Up => self.highlighted_rom.select_previous(),
            KeyCode::Char('j') | KeyCode::Down => self.highlighted_rom.select_next(),
            KeyCode::Char('h') | KeyCode::Backspace => self.set_mode(AppMode::SelectConsole),
            KeyCode::Char(' ') => {
                self.toggle_rom_selected();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_download_controls(&mut self, key_event: KeyEvent) -> io::Result<()> {
        match key_event.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let max = self.selected_roms.len().saturating_sub(1);
                self.download_scroll = (self.download_scroll + 1).min(max);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.download_scroll = self.download_scroll.saturating_sub(1);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_selection_select_controls(&mut self, _key_event: KeyEvent) -> io::Result<()> {
        Ok(())
    }
}

pub fn main(sess: &ssh2::Session, consoles_list: Vec<String>) -> io::Result<()> {
    let (event_tx, event_rx) = mpsc::channel::<DesEvent>();
    let mut app_state = AppState::new(sess, consoles_list, event_tx);

    let tx_to_input_events = app_state.event_tx.clone();
    thread::spawn(move || {
        handle_input_events(tx_to_input_events);
    });

    ratatui::run(|terminal| app(terminal, &mut app_state, event_rx))?;
    ratatui::restore();

    Ok(())
}

fn app(
    terminal: &mut DefaultTerminal,
    app_state: &mut AppState,
    rx: mpsc::Receiver<DesEvent>,
) -> std::io::Result<()> {
    app_state.highlighted_console.select_first();

    while !app_state.exit {
        terminal.draw(|frame| render(frame, app_state))?;

        match rx.recv().unwrap() {
            DesEvent::Input(key_event) => app_state.handle_key_event(key_event)?,
            DesEvent::Progress(rom_download) => {
                if let Some(rom) = app_state
                    .selected_roms
                    .iter_mut()
                    .find(|r| r.rom.title == rom_download.rom.title)
                {
                    rom.download_percent = rom_download.download_percent;
                    rom.total_size = rom_download.total_size;
                    rom.total_received = rom_download.total_received;
                }
            }
            DesEvent::DownloadComplete => {
                if !app_state
                    .selected_roms
                    .iter()
                    .any(|r| r.download_percent < 1.0)
                {
                    app_state.selected_roms.clear();
                    app_state.set_mode(AppMode::SelectConsole);
                }
            }
        }
    }
    Ok(())
}

fn render(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.mode == AppMode::Downloading {
        render_download_screen(frame, app_state);
    } else {
        render_main_screen(frame, app_state);
    }
}

fn render_main_screen(frame: &mut Frame, app_state: &mut AppState) {
    let base_layout = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .margin(1)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)]);

    let [base_left_layout, roms_area] = base_layout.areas(frame.area());

    let outer_layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(vec![Constraint::Fill(2), Constraint::Fill(1)]);

    let [consoles_area, selection_area] = outer_layout.areas(base_left_layout);

    // Consoles block
    let consoles_block = Block::default()
        .title_top(Line::from(" CONSOLES ").centered().bold())
        .borders(Borders::ALL);

    let consoles_list = if app_state.mode == AppMode::SelectConsole {
        List::new(app_state.consoles_list.clone())
            .block(consoles_block)
            .bold()
            .fg(Color::Blue)
            .highlight_symbol("> ")
            .highlight_style(Modifier::REVERSED)
    } else {
        List::new(app_state.consoles_list.clone())
            .block(consoles_block)
            .bold()
            .fg(Color::Blue)
            .highlight_symbol("> ")
    };

    frame.render_stateful_widget(
        consoles_list,
        consoles_area,
        &mut app_state.highlighted_console,
    );

    // Selected ROMs block
    let total_selected_size = app_state.selected_roms.iter().map(|r| r.total_size).sum();
    let selected_roms_block = Block::new()
        .title_top(
            Line::from(format!(
                " SELECTED ROMS ({}) ",
                bytes_to_human_readable(total_selected_size)
            ))
            .centered(),
        )
        .bold()
        .fg(Color::Green)
        .borders(Borders::ALL);

    #[allow(clippy::all)]
    let selected_roms_list = List::from(
        app_state
            .selected_roms
            .iter()
            .map(|r| format!("{}/{}", r.rom.console.clone(), r.rom.title.clone()))
            .collect(),
    )
    .block(selected_roms_block);

    frame.render_widget(selected_roms_list, selection_area);

    // ROMs block
    let mut roms: Vec<ListItem> = vec![];
    for rom in &app_state.roms_list {
        let color = if rom.is_selected {
            Color::Green
        } else {
            Color::Magenta
        };
        roms.push(ListItem::new(rom.rom.title.clone()).style(Style::default().fg(color)));
    }

    let roms_block_console_title = if app_state.current_console.is_empty() {
        String::new()
    } else {
        format!(" {} ", &app_state.current_console)
    };
    let roms_block = Block::default()
        .title_top(Line::from(" ROMS ").centered().bold())
        .title_top(Line::from(roms_block_console_title).left_aligned().italic())
        .fg(Color::Magenta)
        .borders(Borders::ALL);

    let roms_list = if app_state.mode == AppMode::SelectRom {
        List::new(roms)
            .block(roms_block)
            .bold()
            .highlight_symbol("> ")
            .highlight_style(Modifier::REVERSED)
    } else {
        List::new(roms)
            .block(roms_block)
            .bold()
            .highlight_symbol("> ")
    };

    frame.render_stateful_widget(roms_list, roms_area, &mut app_state.highlighted_rom);
}

fn render_download_screen(frame: &mut Frame, app_state: &mut AppState) {
    let bar_height = 3;
    let num_roms_selected = app_state.selected_roms.len();
    if num_roms_selected == 0 {
        return;
    }
    let visible = (frame.area().height / bar_height) as usize;
    let visible = visible.min(num_roms_selected);
    let max_scroll = num_roms_selected.saturating_sub(visible);
    app_state.download_scroll = app_state.download_scroll.min(max_scroll);
    let scroll = app_state.download_scroll;
    let layout =
        Layout::vertical(vec![Constraint::Length(bar_height); visible]).split(frame.area());

    for i in 0..visible {
        let idx = scroll + i;
        let rom_download = &app_state.selected_roms[idx];
        let percent = rom_download.download_percent;
        let bar_label = format!(
            "{:.2}% ({} / {})",
            percent * 100.0,
            bytes_to_human_readable(rom_download.total_received),
            bytes_to_human_readable(rom_download.total_size)
        );
        let bar_color = if percent >= 1.0 {
            Color::Green
        } else {
            Color::Yellow
        };
        let bar = Gauge::default()
            .gauge_style(Style::default().fg(bar_color))
            .label(bar_label.fg(Color::White).bold())
            .ratio(percent)
            .block(
                Block::bordered()
                    .title(rom_download.rom.title.clone())
                    .border_set(border::THICK),
            );
        frame.render_widget(bar, layout[i]);
    }

    // Optional scrollbar indicator
    if num_roms_selected > visible {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let scrollbar_area = Rect {
            x: frame.area().right().saturating_sub(1),
            y: frame.area().top(),
            width: 1,
            height: frame.area().height,
        };
        frame.render_stateful_widget(
            scrollbar,
            scrollbar_area,
            &mut ScrollbarState::new(num_roms_selected).position(scroll),
        );
    }
}

fn bytes_to_human_readable(bytes: u64) -> String {
    let bytes = bytes as f64;
    let megabytes = bytes / 1_048_576.0;

    if megabytes >= 1024.0 {
        let gigabytes = bytes / 1_073_741_824.0;
        format!("{:.2}GB", gigabytes)
    } else {
        format!("{:.2}MB", megabytes)
    }
}
