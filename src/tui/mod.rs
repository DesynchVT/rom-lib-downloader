use std::collections::HashMap;

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

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
    selected_roms: HashMap<String, String>,
    mode: AppMode,
}

pub fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    ratatui::run(app)?;
    ratatui::restore();

    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
    let mut app_state = AppState::default();
    loop {
        terminal.draw(|frame| render(frame, &mut app_state))?;

        if let Some(key) = event::read()?.as_key_press_event() {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                KeyCode::Char('k') | KeyCode::Up => app_state.highlighted_console.select_previous(),
                KeyCode::Char('j') | KeyCode::Down => app_state.highlighted_console.select_next(),
                _ => {}
            }
        }
    }
}

fn render(frame: &mut Frame, app_state: &mut AppState) {
    let consoles = vec![
        ListItem::new("wii"),
        ListItem::new("nds"),
        ListItem::new("n64"),
        ListItem::new("psx"),
        ListItem::new("ps2"),
        ListItem::new("psp"),
    ];
    let roms = vec![
        ListItem::new("Crash 1"),
        ListItem::new("Spyro Dragon"),
        ListItem::new("Skate game"),
        ListItem::new("Disney Pixar's Action Game Featuring Hercules"),
        ListItem::new("The Game"),
        ListItem::new("something retro"),
    ];

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
        List::new(consoles.clone())
            .block(
                Block::default()
                    .title_top(Line::from(" CONSOLES ").centered().bold())
                    .borders(Borders::ALL),
            )
            .bold()
            .fg(Color::Blue)
            .highlight_symbol("> ")
            .highlight_style(Style::default().fg(Color::White)),
        //.highlight_style(Modifier::REVERSED),
        outer_layout[0],
        &mut app_state.highlighted_console,
    );

    // frame.render_widget(
    //     Paragraph::new("wii\npsx\netc").block(
    //         Block::new()
    //             .title_top(Line::from(" CONSOLES ").centered())
    //             .bold()
    //             .fg(Color::Blue)
    //             .borders(Borders::ALL),
    //     ),
    //     outer_layout[0],
    // );
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
    frame.render_widget(
        List::new(roms.clone())
            .block(
                Block::default()
                    .title_top(Line::from(" ROMS ").centered().bold())
                    .borders(Borders::ALL),
            )
            .bold()
            .fg(Color::Magenta)
            .highlight_symbol("> ")
            .highlight_style(Style::default().fg(Color::White)),
        base_layout[1],
    );
}
