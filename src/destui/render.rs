use super::{AppMode, AppState};
use crate::utils;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{
        Block, Borders, Gauge, List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

pub fn render(frame: &mut Frame, app_state: &mut AppState) {
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
                utils::bytes_to_human_readable(total_selected_size)
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
            utils::bytes_to_human_readable(rom_download.total_received),
            utils::bytes_to_human_readable(rom_download.total_size)
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
