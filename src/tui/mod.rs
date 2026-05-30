use std::fs;
use std::io::Read;
use std::{collections::HashMap, io::Write};

use color_eyre::eyre::Result;
use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::des_ssh::{self, ssh_download};

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
    Quitting,
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
            AppMode::Quitting => {
                self.mode = AppMode::Quitting;
            }
            _ => {}
        }
    }
}

pub fn main(sess: &ssh2::Session, consoles_list: Vec<String>) -> color_eyre::Result<()> {
    color_eyre::install()?;

    ratatui::run(|terminal| app(terminal, sess, consoles_list))?;
    ratatui::restore();

    Ok(())
}

fn app(
    terminal: &mut DefaultTerminal,
    sess: &ssh2::Session,
    consoles_list: Vec<String>,
) -> color_eyre::Result<()> {
    let mut app_state = AppState::new(sess, consoles_list);

    app_state.highlighted_console.select_first();
    while app_state.mode != AppMode::Quitting {
        terminal.draw(|frame| render(frame, &mut app_state))?;

        if app_state.mode == AppMode::Downloading {
            continue;
        }

        if let Some(key) = event::read()?.as_key_press_event() {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => app_state.set_mode(AppMode::Quitting),
                KeyCode::Char('k') | KeyCode::Up => match app_state.mode {
                    AppMode::SelectConsole => app_state.highlighted_console.select_previous(),
                    AppMode::SelectRom => app_state.highlighted_rom.select_previous(),
                    AppMode::SelectSelection => {}
                    _ => {}
                },
                KeyCode::Char('j') | KeyCode::Down => match app_state.mode {
                    AppMode::SelectConsole => app_state.highlighted_console.select_next(),
                    AppMode::SelectRom => app_state.highlighted_rom.select_next(),
                    AppMode::SelectSelection => {}
                    _ => {}
                },
                KeyCode::Char('l') | KeyCode::Enter => match app_state.mode {
                    AppMode::SelectConsole => app_state.set_mode(AppMode::SelectRom),
                    AppMode::SelectSelection => {}
                    _ => {}
                },
                KeyCode::Char('h') | KeyCode::Backspace => match app_state.mode {
                    AppMode::SelectRom => {
                        app_state.mode = AppMode::SelectConsole;
                    }
                    AppMode::SelectSelection => {
                        app_state.set_mode(AppMode::SelectConsole);
                    }
                    _ => {}
                },
                KeyCode::Char(' ') => match app_state.mode {
                    AppMode::SelectRom => {
                        let rom_index = app_state.highlighted_rom.selected().unwrap();
                        let mut selected_rom = app_state.roms_list.get(rom_index).unwrap().clone();
                        selected_rom.is_selected = true;

                        // Insert ROM into selection list
                        app_state.selected_roms.push(selected_rom);
                    }
                    AppMode::SelectSelection => {
                        todo!();
                    }
                    _ => {}
                },
                KeyCode::Char('d') => {
                    app_state.set_mode(AppMode::Downloading);
                    app_state.selected_roms.clone().into_iter().for_each(|rom| {
                        tui_download(sess, &app_state);
                    });
                    app_state.selected_roms = vec![];
                    app_state.mode = AppMode::SelectConsole;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn render(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.mode != AppMode::Downloading {
        let base_layout = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .margin(1)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(frame.area());

        let outer_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(vec![Constraint::Fill(2), Constraint::Fill(1)])
            .split(base_layout[0]);

        // Consoles block
        frame.render_stateful_widget(
            List::new(app_state.consoles_list.clone())
                .block(
                    Block::default()
                        .title_top(Line::from(" CONSOLES ").centered().bold())
                        .borders(Borders::ALL),
                )
                .bold()
                .fg(Color::Blue)
                .highlight_symbol("> ")
                //    .highlight_style(Style::default().fg(Color::White)),
                .highlight_style(Modifier::REVERSED),
            outer_layout[0],
            &mut app_state.highlighted_console,
        );

        // Selected ROMs block
        frame.render_widget(
            #[allow(clippy::all)]
            List::from(
                app_state
                    .selected_roms
                    .iter()
                    .map(|r| r.rom_title.clone())
                    .collect(),
            )
            .block(
                Block::new()
                    .title_top(Line::from(" SELECTED ROMS ").centered())
                    .bold()
                    .fg(Color::Green)
                    .borders(Borders::ALL),
            ),
            outer_layout[1],
        );

        // ROMs block
        let roms_block_console_title = if app_state.current_console.is_empty() {
            String::new()
        } else {
            format!(" {} ", &app_state.current_console)
        };
        let mut roms: Vec<ListItem> = vec![];
        for rom in &app_state.roms_list {
            roms.push(ListItem::from(rom.rom_title.clone()));
        }
        frame.render_stateful_widget(
            List::new(roms)
                .block(
                    Block::default()
                        .title_top(Line::from(" ROMS ").centered().bold())
                        .title_top(Line::from(roms_block_console_title).left_aligned().italic())
                        .borders(Borders::ALL),
                )
                .bold()
                .fg(Color::Magenta)
                .highlight_symbol("> ")
                .highlight_style(Style::default().fg(Color::White)),
            base_layout[1],
            &mut app_state.highlighted_rom,
        );
    } else {
    }
}
fn tui_download(sess: &ssh2::Session, app_state: &AppState) {
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
        let (mut remote_file, stat) = sess
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
