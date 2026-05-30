use std::fs;
use std::io::{self, Read, Write};

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::des_ssh;

#[derive(Debug, Default, Clone)]
struct RomItem {
    rom_title: String,
    console: String,
    is_selected: bool,
}

#[derive(PartialEq)]
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
    selected_roms: Vec<RomItem>,
    mode: AppMode,
    current_console: String,
    exit: bool,
}

impl<'a> AppState<'a> {
    fn new(sess: &'_ ssh2::Session, consoles_list: Vec<String>) -> AppState<'_> {
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
        }
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
                    .map(|r| RomItem {
                        rom_title: r.clone(),
                        console: self.current_console.clone(),
                        is_selected: false,
                    })
                    .collect();
                self.highlighted_rom.select_first();
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
                    self.selected_roms.clone().into_iter().for_each(|rom| {
                        tui_download(self);
                    });
                    self.selected_roms = vec![];
                    self.set_mode(AppMode::SelectConsole);
                }
                _ => {}
            }
            // Mode-specific keybinds
            match self.mode {
                AppMode::SelectConsole => self.handle_console_select_controls(key_event)?,
                AppMode::SelectRom => self.handle_roms_select_controls(key_event)?,
                AppMode::SelectSelection => self.handle_selection_select_controls(key_event)?,
                _ => {}
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
                let rom_index = self.highlighted_rom.selected().unwrap();
                let mut selected_rom = self.roms_list.get(rom_index).unwrap().clone();
                selected_rom.is_selected = true;

                // Insert ROM into selection list
                self.selected_roms.push(selected_rom);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_selection_select_controls(&mut self, key_event: KeyEvent) -> io::Result<()> {
        Ok(())
    }
}

pub fn main(sess: &ssh2::Session, consoles_list: Vec<String>) -> io::Result<()> {
    let mut app_state = AppState::new(sess, consoles_list);

    ratatui::run(|terminal| app(terminal, &mut app_state))?;
    ratatui::restore();

    Ok(())
}

fn app(terminal: &mut DefaultTerminal, app_state: &mut AppState) -> std::io::Result<()> {
    app_state.highlighted_console.select_first();

    while !app_state.exit {
        terminal.draw(|frame| render(frame, app_state))?;

        if let Some(key_event) = event::read()?.as_key_press_event() {
            app_state.handle_key_event(key_event)?;
        }
    }
    Ok(())
}

fn render(frame: &mut Frame, app_state: &mut AppState) {
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

    let consoles_list = List::new(app_state.consoles_list.clone())
        .block(consoles_block)
        .bold()
        .fg(Color::Blue)
        .highlight_symbol("> ")
        //    .highlight_style(Style::default().fg(Color::White)),
        .highlight_style(Modifier::REVERSED);

    frame.render_stateful_widget(
        consoles_list,
        consoles_area,
        &mut app_state.highlighted_console,
    );

    // Selected ROMs block
    let selected_roms_block = Block::new()
        .title_top(Line::from(" SELECTED ROMS ").centered())
        .bold()
        .fg(Color::Green)
        .borders(Borders::ALL);

    #[allow(clippy::all)]
    let selected_roms_list = List::from(
        app_state
            .selected_roms
            .iter()
            .map(|r| r.rom_title.clone())
            .collect(),
    )
    .block(selected_roms_block);

    frame.render_widget(selected_roms_list, selection_area);

    // ROMs block
    let mut roms: Vec<ListItem> = vec![];
    for rom in &app_state.roms_list {
        roms.push(ListItem::from(rom.rom_title.clone()));
    }

    let roms_block_console_title = if app_state.current_console.is_empty() {
        String::new()
    } else {
        format!(" {} ", &app_state.current_console)
    };
    let roms_block = Block::default()
        .title_top(Line::from(" ROMS ").centered().bold())
        .title_top(Line::from(roms_block_console_title).left_aligned().italic())
        .borders(Borders::ALL);

    let roms_list = List::new(roms)
        .block(roms_block)
        .bold()
        .fg(Color::Magenta)
        .highlight_symbol("> ")
        .highlight_style(Style::default().fg(Color::White));

    frame.render_stateful_widget(roms_list, roms_area, &mut app_state.highlighted_rom);
}

fn tui_download(app_state: &AppState) {
    let local_root_path =
        dotenv::var("LOCAL_ROM_ROOT_PATH").expect("LOCAL_ROM_ROOT_PATH not set in .env");
    let remote_root_path =
        dotenv::var("REMOTE_ROM_ROOT_PATH").expect("REMOTE_ROM_ROOT_PATH not set in .env");

    for rom_item in &app_state.selected_roms {
        let (rom, console) = (&rom_item.rom_title, &rom_item.console);

        let rom_wo_extension = &rom[..rom.rfind('.').unwrap_or(rom.len())];
        let dest_path = format!("{}/{}/{}", local_root_path, console, rom_wo_extension);

        let rom_path = format!("/{}/{}", console, rom);
        let remote_rom_path = format!("{}/{}", remote_root_path, rom_path);
        let local_console_path = format!("{}/{}", local_root_path, console);
        let temp_download_dir = format!("{}/.downloading", local_console_path);
        let part_path = format!("{}/{}.part", temp_download_dir, rom);

        // Connect to the local SSH server
        let (mut remote_file, stat) = app_state
            .sess
            .scp_recv(std::path::Path::new(&remote_rom_path))
            .unwrap();
        println!("remote file size: {}", stat.size());

        // Show a progress bar while downloading
        // Download the file via scp
        let mut buf = vec![0u8; 8192];

        // Open file before the loop, write chunks as they arrive, rename at the end
        fs::create_dir_all(&temp_download_dir).unwrap();
        let mut local_file = fs::File::create(&part_path).unwrap();
        loop {
            let n = remote_file.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            local_file.write_all(&buf[..n]).unwrap();
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
    }
}
