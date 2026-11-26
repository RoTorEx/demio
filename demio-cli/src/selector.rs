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
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
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

fn compact_tokens(count: u128) -> String {
    const SUFFIXES: [&str; 7] = ["", "K", "M", "B", "T", "P", "E"];
    let (mut unit, mut divisor) = (0, 1);
    while unit + 1 < SUFFIXES.len() && count >= divisor * 1000 {
        unit += 1;
        divisor *= 1000;
    }
    if unit == 0 {
        return count.to_string();
    }
    let mut rounded = (count * 100 + divisor / 2) / divisor;
    if rounded == 100_000 && unit + 1 < SUFFIXES.len() {
        unit += 1;
        rounded = 100;
    }
    let whole = rounded / 100;
    let suffix = SUFFIXES[unit];
    match rounded % 100 {
        0 => format!("{whole}{suffix}"),
        fraction if fraction % 10 == 0 => format!("{whole}.{}{suffix}", fraction / 10),
        fraction => format!("{whole}.{fraction:02}{suffix}"),
    }
}

fn run_duration(seconds: u64) -> String {
    [
        (seconds / 3_600, "h"),
        (seconds / 60 % 60, "m"),
        (seconds % 60, "s"),
    ]
    .into_iter()
    .filter(|(amount, _)| *amount > 0)
    .map(|(amount, unit)| format!("{amount} {unit}"))
    .collect::<Vec<_>>()
    .join(" ")
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
        Line::from(vec![
            Span::styled("Name: ", Style::default().fg(colors.label)),
            Span::styled(
                entry.name.clone(),
                Style::default()
                    .fg(colors.project)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        field("ID: ", entry.id.clone()),
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
    text.lines.push(field("Project: ", entry.project.clone()));
    if let Some(demo) = &entry.demo {
        text.lines.push(field("Demo: ", demo.name.clone()));
    }
    text.lines.extend([
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
        text.lines.extend([Line::default(), heading("Steps")]);
        text.lines.push(Line::from(vec![
            Span::styled("0  ", Style::default().fg(colors.label)),
            Span::raw("Introduction"),
        ]));
        text.lines.extend(
            channel
                .introduction()
                .lines()
                .map(|line| Line::raw(format!("   {line}"))),
        );
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
    text.lines
        .extend([Line::default(), heading("Development tokens")]);
    if let Some(tokens) = entry.development_tokens {
        text.lines.extend([
            field(
                "Total: ",
                format!(
                    "{} ({})",
                    compact_tokens(tokens.total()),
                    tokens.basis_label()
                ),
            ),
            field(
                "Input: ",
                format!(
                    "{} ({} cached)",
                    compact_tokens(tokens.input.into()),
                    compact_tokens(tokens.cached_input.into())
                ),
            ),
            field("Output: ", compact_tokens(tokens.output.into())),
        ]);
    } else {
        text.lines.push(Line::raw("Unrecorded"));
    }
    if let Ok((channel, _)) = &resolved {
        text.lines.extend([
            Line::default(),
            heading("Run limit"),
            field(
                "Max run time: ",
                format!("{} after setup", run_duration(channel.session_seconds)),
            ),
        ]);
    }
    text.lines.extend([
        Line::default(),
        heading("Source"),
        field("Manifest: ", entry.manifest.display().to_string()),
    ]);
    if let Some(demo) = &entry.demo {
        text.lines
            .push(field("Demo folder: ", demo.folder.display().to_string()));
    }
    if let Ok((channel, path)) = &resolved {
        if let Ok(workspace) = channel.workspace_path(path) {
            text.lines
                .push(field("Workspace: ", workspace.display().to_string()));
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
    selected: usize,
    list_offset: usize,
    list_limit: usize,
    detail_offset: u16,
    detail_limit: u16,
    follow_selection: bool,
    list: Option<ScenarioList>,
}

impl View {
    fn list_state(&mut self, row: Option<usize>, rows: usize, visible: usize) -> ListState {
        self.list_limit = rows.saturating_sub(visible);
        self.list_offset = self.list_offset.min(self.list_limit);
        if let Some(row) = row.filter(|_| self.follow_selection && visible > 0) {
            self.list_offset = self
                .list_offset
                .clamp((row + 1).saturating_sub(visible), row);
        }
        let on_screen = self.list_offset..self.list_offset + visible;
        ListState::default()
            .with_offset(self.list_offset)
            .with_selected(row.filter(|row| on_screen.contains(row)))
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

const ID_LENGTH: usize = 6;
const ID_WIDTH: usize = ID_LENGTH + 1;
const DATE_WIDTH: usize = 8;
const COLUMN_GAP: &str = "  ";
const TREE_INDENT: &str = "  ";
const SELECTION_MARKER: &str = "› ";

fn short_id(id: &str) -> String {
    let mut chars = id.chars();
    let mut shortened = chars.by_ref().take(ID_LENGTH).collect::<String>();
    if chars.next().is_some() {
        shortened.push('…');
    }
    shortened
}

fn wrap_group_title(title: &str, width: usize) -> Vec<String> {
    let width = width.max(2);
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut used = 0;
    for word in title.split_whitespace() {
        let word = Span::raw(word);
        if used > 0 && used + 1 + word.width() > width {
            lines.push(std::mem::take(&mut current));
            used = 0;
        }
        if used > 0 {
            current.push(' ');
            used += 1;
        }
        for grapheme in word.styled_graphemes(Style::default()) {
            let cells = Span::raw(grapheme.symbol).width();
            if used > 0 && used + cells > width {
                lines.push(std::mem::take(&mut current));
                used = 0;
            }
            current.push_str(grapheme.symbol);
            used += cells;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

struct ScenarioList {
    width: u16,
    rows: usize,
    entry_rows: Vec<usize>,
    header: Line<'static>,
    widget: List<'static>,
}

fn scenario_list(entries: &[Listing], width: u16, colors: &Palette) -> ScenarioList {
    let item_width = usize::from(width).saturating_sub(2 + Span::raw(SELECTION_MARKER).width());
    let name_width = item_width
        .saturating_sub(TREE_INDENT.len() + ID_WIDTH + DATE_WIDTH + COLUMN_GAP.len() * 2)
        as u16;
    let label_style = Style::default().fg(colors.label);
    let group_style = Style::default()
        .fg(colors.detail)
        .add_modifier(Modifier::BOLD);
    let mut rows = Vec::new();
    let mut entry_rows = Vec::with_capacity(entries.len());
    for group in entries.chunk_by(|a, b| group_key(a) == group_key(b)) {
        let title = group[0]
            .demo
            .as_ref()
            .map(|demo| demo.name.as_str())
            .unwrap_or("Standalone scenarios");
        rows.extend(
            wrap_group_title(title, item_width)
                .into_iter()
                .map(|line| ListItem::new(Line::styled(line, group_style))),
        );
        for (position, entry) in group.iter().enumerate() {
            let branch = if position + 1 == group.len() {
                "└ "
            } else {
                "├ "
            };
            let name = entry.name.replace(" · ", " | ");
            let name = if entry.available {
                name
            } else {
                format!("! {name}")
            };
            let date = entry
                .created_at
                .map(|date| {
                    date.with_timezone(&chrono_tz::Europe::Warsaw)
                        .format("%d.%m.%y")
                        .to_string()
                })
                .unwrap_or_else(|| "Unknown".into());
            entry_rows.push(rows.len());
            rows.push(ListItem::new(Line::from(vec![
                Span::styled(branch, Style::default().fg(colors.detail)),
                Span::styled(format!("{:<ID_WIDTH$}", short_id(&entry.id)), label_style),
                Span::raw(COLUMN_GAP),
                Span::styled(
                    format!("{date:<DATE_WIDTH$}"),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw(COLUMN_GAP),
                Span::styled(
                    fit_cell(&name, name_width),
                    Style::default().fg(colors.project),
                ),
            ])));
        }
    }
    let header = Line::styled(
        format!(
            "  {TREE_INDENT}{:<ID_WIDTH$}{COLUMN_GAP}{:<DATE_WIDTH$}{COLUMN_GAP}Name",
            "ID", "Date"
        ),
        label_style,
    );
    let row_count = rows.len();
    let widget = List::new(rows)
        .highlight_style(
            Style::default()
                .bg(colors.selected)
                .fg(colors.selected_text)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(SELECTION_MARKER)
        .highlight_spacing(HighlightSpacing::Always);
    ScenarioList {
        width,
        rows: row_count,
        entry_rows,
        header,
        widget,
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
        .list
        .as_ref()
        .is_none_or(|list| list.width != panes[0].width)
    {
        view.list = Some(scenario_list(entries, panes[0].width, colors));
    }
    let list_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Demio · {} scenarios ", entries.len()))
        .border_style(Style::default().fg(colors.list));
    let list_area = list_block.inner(panes[0]);
    frame.render_widget(list_block, panes[0]);
    let list = view.list.as_ref().expect("list was built");
    let body_height = list_area.height.saturating_sub(2);
    let row = list.entry_rows.get(view.selected).copied();
    let mut state = view.list_state(row, list.rows, usize::from(body_height));
    let list = view.list.as_ref().expect("list was built");
    if list_area.height > 0 {
        frame.render_widget(
            list.header.clone(),
            Rect {
                height: 1,
                ..list_area
            },
        );
    }
    frame.render_stateful_widget(
        &list.widget,
        Rect {
            y: list_area.y.saturating_add(2),
            height: body_height,
            ..list_area
        },
        &mut state,
    );
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
        follow_selection: true,
        ..View::default()
    };
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
        let previous = view.selected;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(None);
            }
            KeyCode::Up | KeyCode::Char('k') => view.selected = view.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                view.selected = (view.selected + 1).min(entries.len().saturating_sub(1))
            }
            KeyCode::Home => view.selected = 0,
            KeyCode::End => view.selected = entries.len().saturating_sub(1),
            KeyCode::PageDown => redraw = view.scroll_details(5),
            KeyCode::PageUp => redraw = view.scroll_details(-5),
            KeyCode::Char(key @ ('e' | 'f')) if !entries.is_empty() => {
                message = match open_source(&entries[view.selected], key == 'e') {
                    Ok(message) => message.into(),
                    Err(error) => format!("Cannot open: {error:#}"),
                };
                redraw = true;
            }
            KeyCode::Enter | KeyCode::Char('d') if !entries.is_empty() => {
                match config.resolve(&entries[view.selected].id) {
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
        if previous != view.selected {
            detail = details(config, &entries[view.selected], &colors);
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
                selected: 59,
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
            assert!(selection.contains("└ demo-5…  20.10.26  "), "{selection}");
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
            assert!(detail.contains("Development tokens"));
            assert!(detail.contains("350") && detail.contains("(allocated)"));
            assert!(detail.find("Metadata") < detail.find("Description"));
            assert!(detail.find("Description") < detail.find("Development tokens"));
            assert!(detail.find("Development tokens") < detail.find("Manifest:"));
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

    #[test]
    fn shows_complete_group_titles_and_six_character_ids() {
        assert_eq!(short_id("kPfDZ5"), "kPfDZ5");
        assert_eq!(short_id("1234567"), "123456…");
        assert_eq!(short_id("browser-demo"), "browse…");
        assert_eq!(
            wrap_group_title("Standalone scenarios", 24),
            ["Standalone scenarios"]
        );
        assert_eq!(
            wrap_group_title("Demio rehearsals", 10),
            ["Demio", "rehearsals"]
        );
        assert_eq!(wrap_group_title("LongGroup", 5), ["LongG", "roup"]);
    }

    #[test]
    fn rounds_token_counts_and_formats_run_limits() {
        for (count, label) in [
            (999, "999"),
            (1_000, "1K"),
            (11_574, "11.57K"),
            (2_248_399, "2.25M"),
            (999_999, "1M"),
            (1_250_000_000, "1.25B"),
        ] {
            assert_eq!(compact_tokens(count), label);
        }
        assert_eq!(run_duration(7_200), "2 h");
        assert_eq!(run_duration(3_661), "1 h 1 m 1 s");
    }
}
