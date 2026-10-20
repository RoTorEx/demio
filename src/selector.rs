use crate::registry::{Config, Listing};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
};
use std::{
    io,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
        let _ = terminal::disable_raw_mode();
    }
}

fn accent(theme: &str) -> Color {
    match theme {
        "classic" => Color::White,
        "forest" => Color::Green,
        "sunset" => Color::LightRed,
        "ocean" => Color::Cyan,
        "midnight" => Color::LightMagenta,
        _ => Color::LightBlue,
    }
}

fn details(config: &Config, entry: &Listing) -> String {
    let mut text = format!(
        "{}\n\nID: {}\nProject: {}\nCreated: {} Warsaw\n\n{}\n\nManifest: {}\n",
        entry.display_name,
        entry.id,
        entry.project,
        entry.created_label(),
        if entry.description.trim().is_empty() {
            "No description provided"
        } else {
            &entry.description
        },
        entry.manifest.display()
    );
    match config.resolve(&entry.id) {
        Ok((channel, path)) => {
            text.push_str(&format!(
                "Workspace: {}\nSession limit: {} seconds\n\nSteps ({})\n",
                channel.workspace_path(&path).unwrap_or_default().display(),
                channel.session_seconds,
                channel.steps.len()
            ));
            for (index, step) in channel.steps.iter().enumerate() {
                text.push_str(&format!("{}. {} · {}\n", index + 1, step.title, step.role));
            }
        }
        Err(error) => text.push_str(&format!(
            "\nUnavailable: {error:#}\nRepair the manifest before running this demo."
        )),
    }
    text
}

fn panes(area: Rect) -> [Rect; 2] {
    let layout = if area.width < 100 {
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)])
    } else {
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
    };
    let regions = layout.split(area);
    [regions[0], regions[1]]
}

fn max_scroll(detail: &str, area: Rect) -> u16 {
    let lines = Paragraph::new(detail)
        .wrap(Wrap { trim: false })
        .line_count(area.width.saturating_sub(2).max(1));
    lines
        .saturating_sub(area.height.saturating_sub(2) as usize)
        .min(u16::MAX as usize) as u16
}

fn draw(
    frame: &mut Frame,
    entries: &[Listing],
    state: &mut TableState,
    detail: &str,
    scroll: u16,
    theme: &str,
    message: &str,
) {
    let area = frame.area();
    let regions = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).split(area);
    let panes = panes(regions[0]);
    let scroll = scroll.min(max_scroll(detail, panes[1]));
    let color = accent(theme);
    let rows = entries.iter().map(|entry| {
        Row::new(vec![
            Cell::from(entry.id.clone()),
            Cell::from(
                entry
                    .created_at
                    .map(|date| {
                        date.with_timezone(&chrono_tz::Europe::Warsaw)
                            .format("%Y-%m-%d")
                            .to_string()
                    })
                    .unwrap_or_else(|| "Unknown".into()),
            ),
            Cell::from(format!(
                "{}{}",
                if entry.available { "" } else { "! " },
                entry.display_name
            )),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(38),
            Constraint::Length(10),
            Constraint::Min(8),
        ],
    )
    .header(
        Row::new(["ID", "Created", "Name"])
            .style(Style::default().fg(color))
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Demio · Demonstrations ({}) ", entries.len())),
    )
    .row_highlight_style(
        Style::default()
            .bg(color)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("› ");
    frame.render_stateful_widget(table, panes[0], state);
    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new("No demos registered.\nUse demio register FILE.")
                .block(Block::default().borders(Borders::ALL).title(" Details ")),
            panes[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new(detail)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Details ")
                        .border_style(Style::default().fg(color)),
                ),
            panes[1],
        );
    }
    frame.render_widget(
        Paragraph::new(format!(
            "↑/↓ or j/k Select · Enter Run · PgUp/PgDn Details · q/Esc Quit\n{message}"
        )),
        regions[1],
    );
}

pub fn choose(config: &Config, cancelled: &AtomicBool) -> Result<Option<String>> {
    let entries = config.listing();
    let mut state = TableState::default();
    state.select((!entries.is_empty()).then_some(0));
    let mut selected: usize = 0;
    let mut scroll = 0u16;
    let mut message = String::new();
    let mut detail = entries
        .first()
        .map(|entry| details(config, entry))
        .unwrap_or_default();
    terminal::enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        terminal.draw(|frame| {
            draw(
                frame,
                &entries,
                &mut state,
                &detail,
                scroll,
                &config.theme,
                &message,
            )
        })?;
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            let previous = selected;
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(entries.len().saturating_sub(1))
                }
                KeyCode::Home => selected = 0,
                KeyCode::End => selected = entries.len().saturating_sub(1),
                KeyCode::PageDown => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height.saturating_sub(2));
                    scroll = scroll
                        .saturating_add(5)
                        .min(max_scroll(&detail, panes(area)[1]));
                }
                KeyCode::PageUp => scroll = scroll.saturating_sub(5),
                KeyCode::Enter if !entries.is_empty() => {
                    match config.resolve(&entries[selected].id) {
                        Ok(_) => return Ok(Some(entries[selected].id.clone())),
                        Err(error) => message = format!("Cannot run: {error}"),
                    }
                }
                _ => (),
            }
            if previous != selected {
                state.select(Some(selected));
                detail = details(config, &entries[selected]);
                scroll = 0;
                message.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    #[test]
    fn renders_empty_and_long_lists_at_multiple_sizes() {
        let entries: Vec<_> = (0..60)
            .map(|n| Listing {
                id: format!("demo-{n:02}"),
                name: "Example".into(),
                display_name: "Project · Example".into(),
                project: "Project".into(),
                description: "Description".into(),
                created_at: None,
                manifest: "missing.json".into(),
                available: false,
            })
            .collect();
        for (width, height) in [(140, 24), (80, 24), (40, 12)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut state = TableState::default().with_selected(Some(59));
            let detail = details(&Config::default(), &entries[59]);
            terminal
                .draw(|frame| draw(frame, &entries, &mut state, &detail, 0, "ocean", ""))
                .unwrap();
            let content: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            assert!(
                content.contains("demo-59"),
                "Selected row must scroll into view"
            );
            assert!(content.contains("Details"));
            terminal
                .draw(|frame| draw(frame, &[], &mut TableState::default(), "", 0, "calm", ""))
                .unwrap();
            let content: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            assert!(content.contains("No demos registered"));
        }
    }
}
