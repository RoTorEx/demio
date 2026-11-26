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
    widgets::{
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
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
}

fn palette(theme: &str) -> Palette {
    let (list, detail) = match theme {
        "classic" => (Color::Blue, Color::Magenta),
        "forest" => (Color::Green, Color::LightGreen),
        "sunset" => (Color::LightRed, Color::LightMagenta),
        "ocean" => (Color::Cyan, Color::Blue),
        "midnight" => (Color::DarkGray, Color::LightBlue),
        _ => (Color::LightBlue, Color::LightMagenta),
    };
    Palette { list, detail }
}

const EMPHASIS: Style = Style::new().add_modifier(Modifier::BOLD);
const SELECTION_MARK: &str = "› ";

fn display_name(entry: &Listing) -> String {
    if entry.demo.is_some() {
        entry.name.replace(" · ", " | ")
    } else {
        entry.display_name.replace(" · ", " | ")
    }
}

fn fit_name(name: String, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let name = name.replace(['\n', '\r', '\t'], " ");
    let span = Span::raw(name.as_str());
    if span.width() <= width {
        return name;
    }
    let mut fitted = String::new();
    let mut remaining = width - 1;
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

fn details(config: &Config, entry: &Listing) -> Text<'static> {
    let heading = |label| Line::styled(label, EMPHASIS);
    let field =
        |label, value: String| Line::from(vec![Span::styled(label, EMPHASIS), Span::raw(value)]);
    let mut text = Text::from(Line::styled(display_name(entry), EMPHASIS));
    let resolved = config.resolve(&entry.id);
    if let Err(error) = &resolved {
        text.lines
            .push(Line::styled(format!("Unavailable: {error:#}"), EMPHASIS));
    }
    let description = entry.description.trim();
    if !description.is_empty()
        && !resolved
            .as_ref()
            .is_ok_and(|(channel, _)| channel.introduction().trim() == description)
    {
        text.lines.extend(Text::raw(description.to_owned()).lines);
    }
    if let Ok((channel, _)) = &resolved {
        text.lines
            .extend([Line::default(), heading("0  Introduction")]);
        text.lines
            .extend(Text::raw(channel.introduction().to_owned()).lines);
        text.lines.extend([Line::default(), heading("Steps")]);
        let show_roles = channel.steps.iter().any(|step| step.role != "Presenter");
        for (index, step) in channel.steps.iter().enumerate() {
            let mut spans = vec![
                Span::raw(format!("{:<3}", index + 1)),
                Span::raw(step.title.clone()),
            ];
            if show_roles {
                spans.push(Span::raw(format!(" · {}", step.role)));
            }
            if step.confirm {
                spans.push(Span::styled("  [Confirm]", EMPHASIS));
            }
            text.lines.push(Line::from(spans));
            if !step.wait.trim().is_empty() {
                text.lines
                    .extend(Text::raw(format!("   Wait: {}", step.wait.trim())).lines);
            }
        }
    }

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
    text.lines.extend([
        Line::default(),
        heading("Source and settings"),
        field("ID: ", entry.id.clone()),
        field("Project: ", entry.project.clone()),
        field("Created date: ", date),
        field("Created time: ", time),
        field("Manifest: ", entry.manifest.display().to_string()),
    ]);
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
    if let Some(demo) = &entry.demo {
        text.lines.extend([
            Line::default(),
            heading("Owning demo"),
            Line::raw(demo.name.clone()),
            field("Demo folder: ", demo.folder.display().to_string()),
        ]);
        text.lines.extend(Text::raw(demo.description.clone()).lines);
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
    let list_width = (area.width.saturating_sub(1) / 2).min(46);
    let [list, _, detail] = Layout::horizontal([
        Constraint::Length(list_width),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    [list, detail]
}

fn draw(
    frame: &mut Frame,
    entries: &[Listing],
    state: &mut ListState,
    detail: &Text<'_>,
    scroll: &mut u16,
    colors: &Palette,
    message: &str,
) {
    let area = frame.area();
    let footer_height = if message.is_empty() { 2 } else { 3 };
    let regions =
        Layout::vertical([Constraint::Min(0), Constraint::Length(footer_height)]).split(area);
    let [list_area, detail_area] = panes(regions[0]);
    let list_block = Block::bordered()
        .title(format!(" Demio · {} scenarios ", entries.len()))
        .title_style(EMPHASIS.fg(Color::Reset))
        .border_style(Style::default().fg(colors.list))
        .padding(Padding::horizontal(1));
    let name_width = usize::from(list_block.inner(list_area).width)
        .saturating_sub(Span::raw(SELECTION_MARK).width());
    let has_demos = entries.iter().any(|entry| entry.demo.is_some());
    let mut items = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if has_demos && (index == 0 || group_key(entry) != group_key(&entries[index - 1])) {
            let title = entry
                .demo
                .as_ref()
                .map(|demo| demo.name.as_str())
                .unwrap_or("Standalone scenarios");
            items.push(ListItem::new(fit_name(title.to_owned(), name_width)).style(EMPHASIS));
        }
        let name = display_name(entry);
        let label = if entry.available {
            name
        } else {
            format!("! {name}")
        };
        items.push(ListItem::new(fit_name(label, name_width)));
    }
    let list = List::new(items)
        .block(list_block)
        .highlight_style(
            Style::default()
                .fg(Color::Reset)
                .bg(Color::Reset)
                .add_modifier(Modifier::REVERSED),
        )
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_symbol(SELECTION_MARK);
    frame.render_stateful_widget(list, list_area, state);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Details ")
        .title_style(EMPHASIS.fg(Color::Reset))
        .border_style(Style::default().fg(colors.detail))
        .padding(Padding::horizontal(1));
    let details_area = block.inner(detail_area);
    let paragraph = if entries.is_empty() {
        Paragraph::new("No demos registered.\nUse demio register FILE.")
    } else {
        Paragraph::new(detail.clone())
    }
    .wrap(Wrap { trim: false });
    let overflow = paragraph
        .line_count(details_area.width.max(1))
        .saturating_sub(usize::from(details_area.height));
    *scroll = (*scroll).min(overflow.min(usize::from(u16::MAX)) as u16);
    frame.render_widget(paragraph.scroll((*scroll, 0)).block(block), detail_area);
    if overflow > 0 && !details_area.is_empty() {
        let track = Rect::new(
            detail_area.x,
            details_area.y,
            detail_area.width,
            details_area.height,
        );
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None)
            .thumb_symbol("┃")
            .thumb_style(Style::default().fg(colors.detail));
        let mut position = ScrollbarState::new(overflow + 1).position(usize::from(*scroll));
        frame.render_stateful_widget(scrollbar, track, &mut position);
    }
    let primary = if area.width >= 65 {
        "↑↓ Select   Enter Open presenter   PgUp/PgDn Details   Esc Quit"
    } else {
        "↑↓ Select   Enter Open   PgUp/PgDn Details   Esc Quit"
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::raw(primary),
            Line::raw("d Rehearse   e VS Code   f Finder"),
            Line::styled(message, EMPHASIS),
        ]),
        regions[1],
    );
}

/// The selected scenario ID, and whether it should start as a dry run.
pub fn choose(config: &Config, cancelled: &AtomicBool) -> Result<Option<(String, bool)>> {
    let entries = grouped(config.listing());
    let mut state = ListState::default();
    state.select((!entries.is_empty()).then(|| selected_row(&entries, 0)));
    let mut selected: usize = 0;
    let mut scroll = 0u16;
    let mut message = String::new();
    let colors = palette(&config.theme);
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
                &mut scroll,
                &colors,
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
                KeyCode::PageDown => scroll = scroll.saturating_add(5),
                KeyCode::PageUp => scroll = scroll.saturating_sub(5),
                KeyCode::Char(key @ ('e' | 'f')) if !entries.is_empty() => {
                    message = match open_source(&entries[selected], key == 'e') {
                        Ok(message) => message.into(),
                        Err(error) => format!("Cannot open: {error:#}"),
                    };
                }
                KeyCode::Enter | KeyCode::Char('d') if !entries.is_empty() => {
                    match config.resolve(&entries[selected].id) {
                        Ok(_) => {
                            return Ok(Some((
                                entries[selected].id.clone(),
                                key.code == KeyCode::Char('d'),
                            )));
                        }
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
    fn renders_readable_selection_and_long_names_at_multiple_sizes() {
        let long_name = "IANA download, timer and confirmation";
        let entries: Vec<_> = (0..60)
            .map(|n| {
                let name = if n == 59 {
                    long_name.to_owned()
                } else {
                    format!("Example {n:02}")
                };
                Listing {
                    demo: None,
                    id: format!("demo-{n:02}"),
                    display_name: name.clone(),
                    name,
                    project: "Project".into(),
                    description: "A visible demo outcome.".into(),
                    created_at: Some(
                        chrono::DateTime::parse_from_rfc3339("2026-10-20T12:00:00+02:00").unwrap(),
                    ),
                    manifest: "missing.json".into(),
                    available: false,
                }
            })
            .collect();
        for (width, height) in [(140, 30), (92, 20), (80, 24), (60, 30)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut state = ListState::default().with_selected(Some(59));
            let colors = palette("ocean");
            let detail = details(&Config::default(), &entries[59]);
            let mut scroll = 0;
            terminal
                .draw(|frame| {
                    draw(
                        frame,
                        &entries,
                        &mut state,
                        &detail,
                        &mut scroll,
                        &colors,
                        "",
                    )
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
            assert!(list.contains("› ! IANA download, timer"));
            assert_eq!(list.contains(long_name), width >= 92);
            assert_eq!(list.contains('…'), width < 92);
            assert!(!list.contains("demo-59") && !list.contains("2026-10-20"));
            let selection_x = areas[0].x + 2;
            let selection_y = (areas[0].y..areas[0].bottom())
                .find(|y| buffer[(selection_x, *y)].symbol() == "›")
                .unwrap();
            for x in selection_x..areas[0].right() - 2 {
                let cell = &buffer[(x, selection_y)];
                assert_eq!((cell.fg, cell.bg), (Color::Reset, Color::Reset));
                assert_eq!(cell.modifier, Modifier::REVERSED);
            }
            assert_ne!(
                buffer[(areas[0].x, areas[0].y)].fg,
                buffer[(areas[1].x, areas[1].y)].fg,
                "List and detail borders have distinct theme roles"
            );
            for area in areas {
                let title_cell = (area.x..area.right())
                    .map(|x| &buffer[(x, area.y)])
                    .find(|cell| cell.symbol() == "D")
                    .unwrap();
                assert_eq!(
                    title_cell.fg,
                    Color::Reset,
                    "Titles use readable text colors"
                );
            }
            assert!(detail.contains("demo-59"), "Full ID remains in details");
            assert!(
                detail.contains("2026-10-20"),
                "Creation date remains in details"
            );
            let body = Rect::new(areas[1].x + 2, areas[1].y + 1, areas[1].width - 4, 3);
            assert!(
                pane_text(body)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .contains(long_name),
                "The selected title wraps in details"
            );
            assert!(
                detail.contains("A visible demo outcome."),
                "The selected description must be readable above technical fields"
            );
            assert!(detail.contains("Details"));
            assert_eq!(areas[0].y, areas[1].y);
            assert!(areas[0].right() <= areas[1].x);
            assert!(areas[0].width <= 46 && areas[1].width >= areas[0].width);
            terminal
                .draw(|frame| {
                    draw(
                        frame,
                        &[],
                        &mut ListState::default(),
                        &Text::default(),
                        &mut scroll,
                        &colors,
                        "",
                    )
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
    fn fits_labels_by_terminal_width() {
        let label = "Cafe\u{301} 東京";
        assert_eq!(fit_name(label.into(), 9), label);
        assert_eq!(fit_name(label.into(), 8), "Cafe\u{301} 東…");
        assert_eq!(fit_name("東京".into(), 1), "…");
    }
}
