use crate::registry::{Channel, Config, Listing};
use anyhow::{Context, Result, bail};
use crossterm::{
    event::{
        self, DisableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
    },
    execute,
    style::Print,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, HighlightSpacing, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Wrap,
    },
};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
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
            Color::Rgb(30, 32, 44),
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

fn fit_cell(text: &str, width: u16) -> String {
    if width == 0 {
        return String::new();
    }
    let text = text.replace(['\n', '\r', '\t'], " ");
    let span = Span::raw(text.as_str());
    if span.width() <= usize::from(width) {
        return text;
    }
    let mut fitted = String::new();
    let mut remaining = usize::from(width - 1);
    for grapheme in span.styled_graphemes(Style::default()) {
        let width = Span::raw(grapheme.symbol).width();
        if width > remaining {
            break;
        }
        fitted.push_str(grapheme.symbol);
        remaining -= width;
    }
    fitted.push('…');
    fitted
}

fn group_key(entry: &Listing) -> Option<&Path> {
    entry.demo.as_ref().map(|demo| demo.manifest.as_path())
}

fn grouped(entries: Vec<Listing>) -> Vec<Listing> {
    let mut positions = HashMap::new();
    let mut groups: Vec<Vec<Listing>> = Vec::new();
    for entry in entries {
        let index = *positions
            .entry(group_key(&entry).map(Path::to_path_buf))
            .or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
        groups[index].push(entry);
    }
    groups.into_iter().flatten().collect()
}

fn group_start(entries: &[Listing], index: usize) -> bool {
    index == 0 || group_key(&entries[index]) != group_key(&entries[index - 1])
}

fn selected_row(entries: &[Listing], selected: usize) -> Option<usize> {
    entries.get(selected)?;
    let headers = if entries.iter().any(|entry| entry.demo.is_some()) {
        (0..=selected)
            .filter(|&index| group_start(entries, index))
            .count()
    } else {
        0
    };
    Some(selected + headers)
}

fn details(config: &Config, entry: &Listing, colors: &Palette) -> Text<'static> {
    let heading = |label| {
        Line::styled(
            label,
            Style::default()
                .fg(colors.detail)
                .add_modifier(Modifier::BOLD),
        )
    };
    let field = |label, value: String| {
        Line::from(vec![
            Span::styled(label, Style::default().fg(colors.label)),
            Span::raw(value),
        ])
    };
    let resolved = config.resolve(&entry.id);
    let created = entry
        .created_at
        .map(|date| date.with_timezone(&chrono_tz::Europe::Warsaw));
    let mut text = Text::from(vec![
        heading("Metadata"),
        field("ID: ", entry.id.clone()),
        Line::from(vec![
            Span::styled("Name: ", Style::default().fg(colors.label)),
            Span::styled(
                entry.name.clone(),
                Style::default()
                    .fg(colors.project)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        field(
            "Type: ",
            entry
                .scenario_type
                .map_or("Unspecified", |kind| kind.label())
                .into(),
        ),
    ]);
    if let Err(error) = &resolved {
        text.lines.push(Line::styled(
            format!("Unavailable: {error:#}"),
            Style::default().fg(Color::LightRed),
        ));
    }
    text.lines.extend([
        field("Project: ", entry.project.clone()),
        field(
            "Created date: ",
            created.map_or_else(
                || "Unknown".into(),
                |date| date.format("%Y-%m-%d").to_string(),
            ),
        ),
        field(
            "Created time: ",
            created.map_or_else(
                || "Unknown".into(),
                |date| date.format("%H:%M:%S %Z (UTC%:z) Warsaw").to_string(),
            ),
        ),
    ]);
    text.lines.push(field(
        "Development tokens: ",
        entry
            .development_tokens
            .map_or_else(|| "Unrecorded".into(), |tokens| tokens.label()),
    ));
    if let Some(tokens) = entry.development_tokens {
        text.lines.push(field(
            "Token usage: ",
            format!(
                "{} input ({} cached), {} output",
                tokens.input, tokens.cached_input, tokens.output
            ),
        ));
    }
    text.lines
        .push(field("Manifest: ", entry.manifest.display().to_string()));
    if let Some(demo) = &entry.demo {
        text.lines.extend([
            field("Owning demo: ", demo.name.clone()),
            field("Demo folder: ", demo.folder.display().to_string()),
        ]);
    }
    if let Ok((channel, path)) = &resolved {
        if let Ok(workspace) = channel.workspace_path(path) {
            text.lines
                .push(field("Workspace: ", workspace.display().to_string()));
        }
        text.lines.push(field(
            "Session limit: ",
            format!("{} seconds", channel.session_seconds),
        ));
    }

    let description = entry.description.trim();
    text.lines.extend([Line::default(), heading("Description")]);
    if description.is_empty() {
        text.lines.push(Line::raw("No description provided"));
    } else {
        text.lines.extend(Text::raw(description.to_owned()).lines);
    }
    if let Some(demo) = &entry.demo {
        if !demo.description.trim().is_empty() && demo.description.trim() != description {
            text.lines
                .push(field("Demo description: ", demo.description.clone()));
        }
    }
    if let Ok((channel, _)) = &resolved {
        text.lines
            .extend([Line::default(), heading("0  Introduction")]);
        text.lines
            .extend(Text::raw(channel.introduction().to_owned()).lines);
        text.lines.extend([Line::default(), heading("Steps")]);
        for (index, step) in channel.steps.iter().enumerate() {
            let confirmation = if step.confirm { " · Confirm" } else { "" };
            text.lines.push(Line::from(vec![
                Span::styled(
                    format!("{}  ", index + 1),
                    Style::default().fg(colors.label),
                ),
                Span::raw(step.title.clone()),
                Span::styled(
                    format!(" · {}{confirmation}", step.role),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ]));
            if !step.wait.trim().is_empty() {
                text.lines
                    .extend(Text::raw(format!("   Wait: {}", step.wait.trim())).lines);
            }
        }
    }

    text
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
    let list_width = (area.width / 2).min(52);
    Layout::horizontal([Constraint::Length(list_width), Constraint::Min(0)]).areas(area)
}

#[derive(Default)]
struct View {
    row: Option<usize>,
    list_offset: usize,
    list_limit: usize,
    detail_offset: u16,
    detail_limit: u16,
    follow_selection: bool,
    table: Option<ListTable>,
}

impl View {
    fn table_state(&mut self, rows: usize, visible: usize) -> TableState {
        self.list_limit = rows.saturating_sub(visible);
        self.list_offset = self.list_offset.min(self.list_limit);
        if let Some(row) = self.row.filter(|_| self.follow_selection && visible > 0) {
            self.list_offset = self
                .list_offset
                .clamp((row + 1).saturating_sub(visible), row);
        }
        let on_screen = self.list_offset..self.list_offset + visible;
        TableState::new()
            .with_offset(self.list_offset)
            .with_selected(self.row.filter(|row| on_screen.contains(row)))
    }

    fn scroll_list(&mut self, rows: isize) -> bool {
        self.follow_selection = false;
        let next = self
            .list_offset
            .saturating_add_signed(rows)
            .min(self.list_limit);
        let moved = next != self.list_offset;
        self.list_offset = next;
        moved
    }

    fn scroll_details(&mut self, rows: i16) -> bool {
        let next = self
            .detail_offset
            .saturating_add_signed(rows)
            .min(self.detail_limit);
        let moved = next != self.detail_offset;
        self.detail_offset = next;
        moved
    }
}

struct ListTable {
    width: u16,
    rows: usize,
    widget: Table<'static>,
}

fn list_table(entries: &[Listing], width: u16, colors: &Palette) -> ListTable {
    let has_demos = entries.iter().any(|entry| entry.demo.is_some());
    let longest_id = entries
        .iter()
        .map(|entry| entry.id.len())
        .max()
        .unwrap_or(0);
    // Two borders, the selection marker, and one space between each pair of columns.
    let cells = width.saturating_sub(6);
    let id_width = (longest_id.clamp(2, 12) as u16).min(cells.saturating_sub(16).max(2));
    let date_width = cells.saturating_sub(id_width).min(8);
    let name_width = cells.saturating_sub(id_width + date_width);
    let mut rows = Vec::new();
    for group in entries.chunk_by(|a, b| group_key(a) == group_key(b)) {
        if has_demos {
            let count = group.len();
            let title = group[0]
                .demo
                .as_ref()
                .map(|demo| demo.name.as_str())
                .unwrap_or("Standalone scenarios");
            rows.push(
                Row::new([
                    "".into(),
                    "".into(),
                    fit_cell(&format!("▾ {title} ({count})"), name_width),
                ])
                .style(
                    Style::default()
                        .fg(colors.detail)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        }
        for entry in group {
            let name = entry.name.replace(" · ", " | ");
            let name = if entry.available {
                name
            } else {
                format!("! {name}")
            };
            rows.push(Row::new(vec![
                Cell::from(fit_cell(&entry.id, id_width)).style(Style::default().fg(colors.label)),
                Cell::from(
                    entry
                        .created_at
                        .map(|date| {
                            date.with_timezone(&chrono_tz::Europe::Warsaw)
                                .format("%d.%m.%y")
                                .to_string()
                        })
                        .unwrap_or_else(|| "Unknown".into()),
                )
                .style(Style::default().fg(Color::Gray)),
                Cell::from(fit_cell(&name, name_width)),
            ]));
        }
    }
    let row_count = rows.len();
    let table = Table::new(
        rows,
        [
            Constraint::Length(id_width),
            Constraint::Length(date_width),
            Constraint::Min(0),
        ],
    )
    .column_spacing(1)
    .style(Style::default().fg(colors.project))
    .header(
        Row::new(["ID", "Date", "Name"])
            .style(Style::default().fg(colors.label))
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Demio · {} scenarios ", entries.len()))
            .border_style(Style::default().fg(colors.list)),
    )
    .row_highlight_style(
        Style::default()
            .bg(colors.selected)
            .fg(colors.selected_text)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("› ")
    .highlight_spacing(HighlightSpacing::Always);
    ListTable {
        width,
        rows: row_count,
        widget: table,
    }
}

fn draw(
    frame: &mut Frame,
    entries: &[Listing],
    view: &mut View,
    detail: &Text<'_>,
    colors: &Palette,
    message: &str,
) -> [Rect; 2] {
    let area = frame.area();
    let footer_height = if message.is_empty() { 2 } else { 3 };
    let regions =
        Layout::vertical([Constraint::Min(0), Constraint::Length(footer_height)]).split(area);
    let panes = panes(regions[0]);
    if view
        .table
        .as_ref()
        .is_none_or(|table| table.width != panes[0].width)
    {
        view.table = Some(list_table(entries, panes[0].width, colors));
    }
    let row_count = view.table.as_ref().expect("table was built").rows;
    let mut state = view.table_state(row_count, usize::from(panes[0].height.saturating_sub(4)));
    let table = &view.table.as_ref().expect("table was built").widget;
    frame.render_stateful_widget(table, panes[0], &mut state);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Details ")
        .border_style(Style::default().fg(colors.detail));
    let details_area = block.inner(panes[1]);
    let paragraph = if entries.is_empty() {
        Paragraph::new("No demos registered.\nUse demio register FILE.")
    } else {
        Paragraph::new(detail.clone())
    }
    .wrap(Wrap { trim: false });
    let overflow = paragraph
        .line_count(details_area.width.max(1))
        .saturating_sub(usize::from(details_area.height));
    view.detail_limit = overflow.min(usize::from(u16::MAX)) as u16;
    view.detail_offset = view.detail_offset.min(view.detail_limit);
    frame.render_widget(
        paragraph.scroll((view.detail_offset, 0)).block(block),
        panes[1],
    );
    if overflow > 0 && !details_area.is_empty() {
        let track = Rect::new(
            panes[1].x,
            details_area.y,
            panes[1].width,
            details_area.height,
        );
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None)
            .thumb_style(Style::default().fg(colors.detail));
        let mut position =
            ScrollbarState::new(overflow + 1).position(usize::from(view.detail_offset));
        frame.render_stateful_widget(scrollbar, track, &mut position);
    }
    let primary = if area.width >= 73 {
        "↑/↓ or j/k Select · Enter Open presenter · PgUp/PgDn Details · q/Esc Quit"
    } else {
        "↑↓ Select · Enter Open · PgUp/PgDn Details · Esc Quit"
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(primary, Style::default().fg(colors.label)),
            Line::raw("d Rehearse · e VS Code · f Finder"),
            Line::styled(message, Style::default().add_modifier(Modifier::BOLD)),
        ]),
        regions[1],
    );
    panes
}

/// The validated scenario, its source, and whether to rehearse it.
pub fn choose(config: &Config, cancelled: &AtomicBool) -> Result<Option<(Channel, PathBuf, bool)>> {
    let entries = grouped(config.listing());
    let mut view = View {
        row: selected_row(&entries, 0),
        follow_selection: true,
        ..View::default()
    };
    let mut selected: usize = 0;
    let mut message = String::new();
    let colors = palette(&config.theme);
    let mut detail = entries
        .first()
        .map(|entry| details(config, entry, &colors))
        .unwrap_or_default();
    terminal::enable_raw_mode()?;
    let _restore = Restore;
    // Request button and wheel reports with positions. Passive motion needs no reports.
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        Print("\x1b[?1000h\x1b[?1006h")
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut areas = [Rect::default(); 2];
    let mut redraw = true;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        if redraw {
            terminal.draw(|frame| {
                areas = draw(frame, &entries, &mut view, &detail, &colors, &message);
            })?;
            redraw = false;
        }
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        let key = match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => key,
            Event::Resize(..) => {
                redraw = true;
                continue;
            }
            Event::Mouse(mouse) => {
                let position = Position::new(mouse.column, mouse.row);
                let delta = match mouse.kind {
                    MouseEventKind::ScrollDown => 1,
                    MouseEventKind::ScrollUp => -1,
                    _ => continue,
                };
                if areas[0].contains(position) {
                    redraw = view.scroll_list(delta);
                } else if areas[1].contains(position) {
                    redraw = view.scroll_details(delta as i16);
                }
                continue;
            }
            _ => continue,
        };
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
            KeyCode::PageDown => redraw = view.scroll_details(5),
            KeyCode::PageUp => redraw = view.scroll_details(-5),
            KeyCode::Char(key @ ('e' | 'f')) if !entries.is_empty() => {
                message = match open_source(&entries[selected], key == 'e') {
                    Ok(message) => message.into(),
                    Err(error) => format!("Cannot open: {error:#}"),
                };
                redraw = true;
            }
            KeyCode::Enter | KeyCode::Char('d') if !entries.is_empty() => {
                match config.resolve(&entries[selected].id) {
                    Ok((channel, path)) => {
                        return Ok(Some((channel, path, key.code == KeyCode::Char('d'))));
                    }
                    Err(error) => {
                        message = format!("Cannot run: {error}");
                        redraw = true;
                    }
                }
            }
            _ => (),
        }
        if matches!(
            key.code,
            KeyCode::Up | KeyCode::Down | KeyCode::Char('j' | 'k') | KeyCode::Home | KeyCode::End
        ) {
            redraw |= !view.follow_selection;
            view.follow_selection = true;
        }
        if previous != selected {
            view.row = selected_row(&entries, selected);
            detail = details(config, &entries[selected], &colors);
            view.detail_offset = 0;
            message.clear();
            redraw = true;
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
                id: format!("demo-{n:02}-historical-id"),
                name: format!("Example {n:02} with an extended name for clipping"),
                display_name: format!("Project · Example {n:02}"),
                scenario_type: Some(crate::registry::ScenarioType::Browser),
                development_tokens: Some(crate::registry::DevelopmentTokens {
                    input: 300,
                    cached_input: 220,
                    output: 50,
                    basis: crate::registry::TokenBasis::Allocated,
                }),
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
            let mut view = View {
                row: Some(59),
                follow_selection: true,
                ..View::default()
            };
            let colors = palette("calm");
            let detail = details(&Config::default(), &entries[59], &colors);
            terminal
                .draw(|frame| {
                    draw(frame, &entries, &mut view, &detail, &colors, "");
                })
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
            assert!(list.contains("ID") && list.contains("Date") && list.contains("Name"));
            let selection = list.lines().find(|line| line.contains('›')).unwrap();
            let (_, columns) = selection.split_once('›').unwrap();
            let columns: Vec<_> = columns.split_whitespace().collect();
            assert!(columns[0].starts_with("demo-59") && columns[0].ends_with('…'));
            assert!(columns[0].chars().count() <= 12);
            assert_eq!(columns[1], "20.10.26");
            assert!(selection.ends_with("…│"), "{selection}");
            let first_column = areas[0].x + 1;
            let selected_y = (areas[0].y..areas[0].bottom())
                .find(|y| buffer[(first_column, *y)].symbol() == "›")
                .unwrap();
            let selected_cell = &buffer[(first_column + 2, selected_y)];
            assert_eq!(selected_cell.fg, Color::Rgb(30, 32, 44));
            assert_eq!(selected_cell.bg, Color::Blue);
            assert_eq!(selected_cell.modifier, Modifier::BOLD);
            assert_ne!(
                buffer[(areas[0].x, areas[0].y)].fg,
                buffer[(areas[1].x, areas[1].y)].fg,
                "List and detail borders have distinct theme roles"
            );
            assert!(detail.contains("demo-59-historical-id"), "{detail}");
            assert!(detail.contains("Type: Browser"));
            assert!(detail.contains("Development tokens:"));
            assert!(detail.contains("350") && detail.contains("(allocated)"));
            assert!(detail.find("Metadata") < detail.find("Manifest:"));
            assert!(detail.find("Manifest:") < detail.find("Description"));
            assert!(detail.contains("A visible demo outcome."));
            assert!(detail.contains("Details"));
            assert_eq!(areas[0].y, areas[1].y);
            assert!(areas[0].right() <= areas[1].x);
            assert!(areas[0].width <= 52 && areas[1].width >= areas[0].width);
            terminal
                .draw(|frame| {
                    draw(
                        frame,
                        &[],
                        &mut View::default(),
                        &Text::default(),
                        &colors,
                        "",
                    );
                })
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

    #[test]
    fn skips_whole_graphemes_when_shortening_names() {
        assert_eq!(fit_cell("A界e\u{301} long", 5), "A界e\u{301}…");
        assert_eq!(fit_cell("A界e\u{301} long", 3), "A…");
    }
}
