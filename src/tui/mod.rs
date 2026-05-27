use std::collections::HashMap;

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::des_ssh;

#[derive(Debug, Default)]
enum AppMode {
    #[default]
    SelectConsole,
    SelectRom,
    SelectSelection,
    Downloading,
    Quitting,
    Confirming,
}

#[derive(Debug, Default)]
struct AppState {
    consoles_list: Vec<String>,
    roms_list: Vec<String>,
    highlighted_console: ListState,
    highlighted_rom: ListState,
    selected_roms: HashMap<String, Vec<String>>,
    mode: AppMode,
    current_console: String,
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
    let mut app_state = AppState {
        consoles_list,
        ..Default::default()
    };
    app_state.highlighted_console.select_first();
    loop {
        terminal.draw(|frame| render(frame, &mut app_state))?;

        if let Some(key) = event::read()?.as_key_press_event() {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
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
                    AppMode::SelectConsole => {
                        app_state.mode = AppMode::SelectRom;
                        let selected_index = app_state.highlighted_console.selected().unwrap();
                        app_state.current_console = app_state.consoles_list[selected_index].clone();

                        app_state.roms_list = des_ssh::list_roms(sess, &app_state.current_console);
                        app_state.highlighted_rom.select_first();
                    }
                    AppMode::SelectSelection => {}
                    _ => {}
                },
                KeyCode::Char('h') | KeyCode::Backspace => match app_state.mode {
                    AppMode::SelectRom => {
                        app_state.mode = AppMode::SelectConsole;
                        app_state.current_console = String::new();
                    }
                    AppMode::SelectSelection => {}
                    _ => {}
                },
                KeyCode::Char(' ') => match app_state.mode {
                    AppMode::SelectRom => {
                        todo!();
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

fn render(frame: &mut Frame, app_state: &mut AppState) {
    let base_layout = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .margin(1)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(frame.area());

    let outer_layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(vec![Constraint::Fill(2), Constraint::Fill(1)])
        .split(base_layout[0]);

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

    frame.render_widget(
        Paragraph::new("None").block(
            Block::new()
                .title_top(Line::from(" SELECTED ROMS ").centered())
                .bold()
                .fg(Color::Green)
                .borders(Borders::ALL),
        ),
        outer_layout[1],
    );

    let roms_block_console_title = if app_state.current_console.is_empty() {
        String::new()
    } else {
        format!(" {} ", &app_state.current_console)
    };
    frame.render_stateful_widget(
        List::new(app_state.roms_list.clone())
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
}
