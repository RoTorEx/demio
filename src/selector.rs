use crate::registry::{Config, Listing};
use anyhow::{Context, Result, bail};
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
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
};
use std::{
    io,
    process::Command,
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

struct Palette {
    list: Color,
    detail: Color,
    label: Color,
    project: Color,
    selected: Color,
    selected_text: Color,
}

fn palette(theme: &str) -> Palette {
    let (list, detail, label, project, selected, selected_text) = match theme {
        "classic" => (
            Color::Blue,
            Color::Magenta,
            Color::Blue,
            Color::Cyan,
            Color::Blue,
            Color::White,
        ),
        "forest" => (
            Color::Green,
            Color::LightGreen,
            Color::LightGreen,
            Color::Yellow,
            Color::Green,
            Color::Black,
        ),
        "sunset" => (
            Color::LightRed,
            Color::LightMagenta,
            Color::Yellow,
            Color::LightRed,
            Color::LightMagenta,
            Color::Black,
        ),
        "ocean" => (
            Color::Cyan,
            Color::Blue,
            Color::LightCyan,
            Color::LightBlue,
            Color::Cyan,
            Color::Black,
        ),
        "midnight" => (
            Color::DarkGray,
            Color::LightBlue,
            Color::LightMagenta,
            Color::LightBlue,
            Color::DarkGray,
            Color::White,
        ),
        _ => (
            Color::LightBlue,
            Color::LightMagenta,
            Color::LightCyan,
            Color::LightYellow,
            Color::Blue,
            Color::White,
        ),
    };
    Palette {
        list,
        detail,
        label,
        project,
        selected,
        selected_text,
    }
}

fn display_name(entry: &Listing) -> String {
    if entry.demo.is_some() {
        entry.name.replace(" · ", " | ")
    } else {
        entry.display_name.replace(" · ", " | ")
    }
}

fn group_key(entry: &Listing) -> String {
    entry
        .demo
        .as_ref()
        .map(|demo| demo.manifest.to_string_lossy().into_owned())
        .unwrap_or_else(|| "legacy".into())
}

fn grouped(entries: Vec<Listing>) -> Vec<Listing> {
    let mut groups: Vec<(String, Vec<Listing>)> = Vec::new();
    for entry in entries {
        let key = group_key(&entry);
        if let Some((_, items)) = groups.iter_mut().find(|(group, _)| group == &key) {
            items.push(entry);
        } else {
            groups.push((key, vec![entry]));
        }
    }
    groups.into_iter().flat_map(|(_, items)| items).collect()
}

fn selected_row(entries: &[Listing], selected: usize) -> usize {
    if !entries.iter().any(|entry| entry.demo.is_some()) {
        return selected;
    }
    let mut headers = 0;
    for index in 0..=selected.min(entries.len().saturating_sub(1)) {
        if index == 0 || group_key(&entries[index]) != group_key(&entries[index - 1]) {
            headers += 1;
        }
    }
    selected + headers
}

fn details(config: &Config, entry: &Listing) -> String {
    let (date, time) = entry
        .created_at
        .map(|date| {
            let date = date.with_timezone(&chrono_tz::Europe::Warsaw);
            (
                date.format("%Y-%m-%d").to_string(),
                date.format("%H:%M:%S %Z (UTC%:z) Warsaw").to_string(),
            )
        })
        .unwrap_or_else(|| ("Unknown".into(), "Unknown".into()));
    let mut text = format!(
        "Scenario | Presenter\n\n{}\n\nDescription\n{}\n\nIdentity | Registry\n\nID: {}\nProject: {}\nCreated date: {}\nCreated time: {}\n\nEnvironment | Project adapter\n\nManifest\n{}\n",
        display_name(entry),
        if entry.description.trim().is_empty() {
            "No description provided"
        } else {
            &entry.description
        },
        entry.id,
        entry.project,
        date,
        time,
        entry.manifest.display()
    );
    if let Some(demo) = &entry.demo {
        text = format!(
            "Demo | Shared components\n\n{}\n{}\n\n{}",
            demo.name, demo.description, text
        );
        text = text.replace(
            "Environment | Project adapter\n\n",
            &format!(
                "Environment | Project adapter\n\nDemo folder\n{}\n\n",
                demo.folder.display()
            ),
        );
    }
    match config.resolve(&entry.id) {
        Ok((channel, path)) => {
            text.push_str(&format!(
                "\nWorkspace\n{}\n\nExecution | Demio\n\nSession limit: {} seconds\n\nSteps | Project adapter\n\n",
                channel.workspace_path(&path).unwrap_or_default().display(), channel.session_seconds
            ));
            for (index, step) in channel.steps.iter().enumerate() {
                text.push_str(&format!("{}. {} | {}\n", index + 1, step.title, step.role));
            }
        }
        Err(error) => {
            text = format!("Unavailable: cannot launch this scenario\n\n{text}");
            text.push_str(&format!(
                "\nUnavailable: {error:#}\nRepair the manifest before running this demo."
            ));
        }
    }
    text
}

fn styled_details<'a>(detail: &'a str, colors: &Palette) -> Text<'a> {
    Text::from(
        detail
            .lines()
            .enumerate()
            .map(|(index, line)| {
                if line.starts_with("Unavailable:") {
                    return Line::styled(line, Style::default().fg(Color::LightRed));
                }
                if matches!(
                    line,
                    "Demo | Shared components"
                        | "Scenario | Presenter"
                        | "Identity | Registry"
                        | "Environment | Project adapter"
                        | "Execution | Demio"
                        | "Steps | Project adapter"
                ) {
                    return Line::styled(
                        line,
                        Style::default()
                            .fg(colors.detail)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                if matches!(
                    line,
                    "Description" | "Manifest" | "Workspace" | "Demo folder"
                ) {
                    return Line::styled(
                        line,
                        Style::default()
                            .fg(colors.label)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                for label in [
                    "ID: ",
                    "Project: ",
                    "Created date: ",
                    "Created time: ",
                    "Session limit: ",
                ] {
                    if let Some(value) = line.strip_prefix(label) {
                        return Line::from(vec![
                            Span::styled(label, Style::default().fg(colors.label)),
                            Span::raw(value),
                        ]);
                    }
                }
                Line::styled(
                    line,
                    if index == 0 {
                        Style::default()
                            .fg(colors.project)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn open_source(entry: &Listing, editor: bool) -> Result<&'static str> {
    let folder = entry
        .demo
        .as_ref()
        .map(|demo| demo.folder.as_path())
        .or_else(|| entry.manifest.parent())
        .context("Manifest has no parent folder")?;
    if !folder.is_dir() {
        bail!("Scenario folder does not exist: {}", folder.display());
    }
    let output = if editor {
        if !entry.manifest.is_file() {
            bail!("Manifest does not exist: {}", entry.manifest.display());
        }
        match Command::new("code")
            .arg("--new-window")
            .arg(folder)
            .arg(&entry.manifest)
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Command::new("/usr/bin/open")
                .args(["-a", "Visual Studio Code"])
                .arg(folder)
                .arg(&entry.manifest)
                .output()
                .context("Could not open Visual Studio Code")?,
            Err(error) => return Err(error).context("Could not open Visual Studio Code"),
        }
    } else {
        let mut command = Command::new("open");
        if entry.manifest.is_file() {
            command.arg("-R").arg(&entry.manifest);
        } else {
            command.arg(folder);
        }
        command.output().context("Could not open Finder")?
    };
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(if editor {
        "Opened scenario folder and manifest in VS Code"
    } else {
        "Opened scenario in Finder"
    })
}

fn panes(area: Rect) -> [Rect; 2] {
    let regions =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
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
    let colors = palette(theme);
    let has_demos = entries.iter().any(|entry| entry.demo.is_some());
    let mut rows = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if has_demos && (index == 0 || group_key(entry) != group_key(&entries[index - 1])) {
            let count = entries
                .iter()
                .filter(|other| group_key(other) == group_key(entry))
                .count();
            let title = entry
                .demo
                .as_ref()
                .map(|demo| demo.name.as_str())
                .unwrap_or("Standalone scenarios");
            rows.push(
                Row::new([Cell::from(""), Cell::from(format!("▾ {title} ({count})"))]).style(
                    Style::default()
                        .fg(colors.detail)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        }
        rows.push(Row::new(vec![
            Cell::from(
                entry
                    .created_at
                    .map(|date| {
                        date.with_timezone(&chrono_tz::Europe::Warsaw)
                            .format("%Y-%m-%d")
                            .to_string()
                    })
                    .unwrap_or_else(|| "Unknown".into()),
            )
            .style(Style::default().fg(Color::Gray)),
            Cell::from(format!(
                "{}{}",
                if entry.available { "" } else { "! " },
                display_name(entry)
            )),
        ]));
    }
    let demo_count = entries
        .iter()
        .filter_map(|entry| entry.demo.as_ref().map(|demo| &demo.manifest))
        .collect::<std::collections::HashSet<_>>()
        .len();
    let title = if has_demos {
        format!(
            " Demio | Demos: {demo_count} | Scenarios: {} ",
            entries.len()
        )
    } else {
        format!(" Demio | Scenarios: {} ", entries.len())
    };
    let table = Table::new(rows, [Constraint::Length(10), Constraint::Min(0)])
        .column_spacing(3)
        .style(Style::default().fg(colors.project))
        .header(
            Row::new(["Date", "Name"])
                .style(Style::default().fg(colors.label))
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(colors.list)),
        )
        .row_highlight_style(
            Style::default()
                .bg(colors.selected)
                .fg(colors.selected_text)
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
            Paragraph::new(styled_details(detail, &colors))
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Details ")
                        .border_style(Style::default().fg(colors.detail)),
                ),
            panes[1],
        );
    }
    frame.render_widget(
        Paragraph::new(format!(
            "↑/↓ or j/k Select · Enter Run · PgUp/PgDn Details · q/Esc Quit\ne VS Code · f Finder   {message}"
        ))
        .style(Style::default().fg(Color::Gray)),
        regions[1],
    );
}

pub fn choose(config: &Config, cancelled: &AtomicBool) -> Result<Option<String>> {
    let entries = grouped(config.listing());
    let mut state = TableState::default();
    state.select((!entries.is_empty()).then(|| selected_row(&entries, 0)));
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
                KeyCode::Char(key @ ('e' | 'f')) if !entries.is_empty() => {
                    message = match open_source(&entries[selected], key == 'e') {
                        Ok(message) => message.into(),
                        Err(error) => format!("Cannot open: {error:#}"),
                    };
                }
                KeyCode::Enter if !entries.is_empty() => {
                    match config.resolve(&entries[selected].id) {
                        Ok(_) => return Ok(Some(entries[selected].id.clone())),
                        Err(error) => message = format!("Cannot run: {error}"),
                    }
                }
                _ => (),
            }
            if previous != selected {
                state.select(Some(selected_row(&entries, selected)));
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
                demo: None,
                id: format!("demo-{n:02}"),
                name: format!("Example {n:02}"),
                display_name: format!("Example {n:02}"),
                project: "Project".into(),
                description: "A visible demo outcome.".into(),
                created_at: Some(
                    chrono::DateTime::parse_from_rfc3339("2026-10-20T12:00:00+02:00").unwrap(),
                ),
                manifest: "missing.json".into(),
                available: false,
            })
            .collect();
        for (width, height) in [(140, 30), (80, 30), (60, 30)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut state = TableState::default().with_selected(Some(59));
            let detail = details(&Config::default(), &entries[59]);
            terminal
                .draw(|frame| draw(frame, &entries, &mut state, &detail, 0, "ocean", ""))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let areas = panes(Rect::new(0, 0, width, height - 2));
            let pane_text = |area: Rect| -> String {
                (area.y..area.bottom())
                    .map(|y| {
                        (area.x..area.right())
                            .map(|x| buffer[(x, y)].symbol())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let list = pane_text(areas[0]);
            let detail = pane_text(areas[1]);
            assert!(
                list.contains("Example 59"),
                "Selected row must scroll into view"
            );
            assert!(!list.contains("demo-59"), "IDs must not use list space");
            assert!(
                list.contains("2026-10-20   ! Example 59"),
                "Date and name must have a visible column gap"
            );
            assert_ne!(
                buffer[(areas[0].x, areas[0].y)].fg,
                buffer[(areas[1].x, areas[1].y)].fg,
                "List and detail borders have distinct theme roles"
            );
            assert!(detail.contains("demo-59"), "Full ID remains in details");
            assert!(
                detail.contains("A visible demo outcome."),
                "The selected description must be readable above technical fields"
            );
            assert!(detail.contains("Details"));
            assert_eq!(areas[0].y, areas[1].y);
            assert!(areas[0].right() <= areas[1].x);
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
