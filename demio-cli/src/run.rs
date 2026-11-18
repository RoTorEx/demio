use crate::{
    protocol::{Event, Worker},
    registry::{Channel, atomic_write},
};
use anyhow::{Context, Result, bail};
use crossterm::{
    cursor,
    event::{self, Event as TerminalEvent, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use serde::Serialize;
use std::{
    fs,
    io::{self, BufRead, IsTerminal, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Default)]
pub struct Position {
    pub completed: usize,
    pub view: Option<usize>,
}
impl Position {
    pub fn next(&self, total: usize) -> Option<(usize, bool)> {
        match self.view {
            Some(view) if view + 1 < self.completed => Some((view + 1, false)),
            _ if self.completed < total => Some((self.completed, true)),
            _ => None,
        }
    }
    pub fn previous(&self) -> Option<usize> {
        self.view.map(|v| v.saturating_sub(1))
    }
    pub fn finish(&mut self, index: usize, execute: bool) {
        if execute {
            self.completed = index + 1;
        }
        self.view = Some(index);
    }
}
#[derive(Serialize)]
struct Display<'a> {
    status: &'a str,
    channel: &'a str,
    step: String,
    title: &'a str,
    role: &'a str,
    say: &'a str,
    wait: &'a str,
    detail: &'a str,
}
/// Red marks a step's wait tip; `paint` restores the theme colour after it.
const RED: &str = "\x1b[1;31m";
const RESET: &str = "\x1b[0m";
struct Screen;
impl Screen {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
        Ok(Self)
    }
}
impl Drop for Screen {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), cursor::Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self> {
        let base = std::env::var_os("DEMIO_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or(std::env::temp_dir().join("demio"));
        fs::create_dir_all(&base)?;
        let name = format!(
            "{}-{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
            std::process::id()
        );
        let path = base.join(name);
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Overlay(Child);
impl Drop for Overlay {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn overlay(session: &Path, sender: mpsc::Sender<String>) -> Result<Overlay> {
    let program = std::env::current_exe()?
        .parent()
        .context("Executable parent unavailable")?
        .join("demio-overlay");
    let mut child = Command::new(&program)
        .arg(session)
        .arg(std::process::id().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| {
            format!(
                "Native panel missing at {}; use --no-overlay for terminal controls",
                program.display()
            )
        })?;
    let stdout = child.stdout.take().context("Overlay stdout unavailable")?;
    thread::spawn(move || {
        for line in io::BufReader::new(stdout)
            .lines()
            .map_while(|line| line.ok())
        {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    Ok(Overlay(child))
}
fn journal(channel: &Channel, position: &Position, status: &str) -> Result<()> {
    crate::usage::event(
        "scenario_progress",
        match status {
            "Error" | "PreparationError" => "error",
            "Finished" => "success",
            "Stopped" => "canceled",
            _ => "observed",
        },
        serde_json::json!({"completed":position.completed,"total":channel.steps.len(),"status":status}),
    );
    Ok(())
}

fn display(
    channel: &Channel,
    position: &Position,
    active: Option<usize>,
    status: &str,
    detail: &str,
    session: &Path,
) -> Result<String> {
    let index = active.or(position.view).unwrap_or(0);
    let step = &channel.steps[index];
    let display = Display {
        status,
        channel: &channel.name,
        step: format!(
            "{}/{}",
            active.or(position.view).map_or(0, |v| v + 1),
            channel.steps.len()
        ),
        title: &step.title,
        role: &step.role,
        say: &step.say,
        wait: &step.wait,
        detail,
    };
    atomic_write(&session.join("panel.json"), &serde_json::to_vec(&display)?)?;
    let wait = if display.wait.is_empty() {
        String::new()
    } else {
        format!(
            "{RED}Wait: {}{RESET}\r\n\r\n",
            display.wait.replace('\n', "\r\n")
        )
    };
    Ok(format!(
        "Demio — {} [{}]\r\n\r\n{}  ·  {}  ·  {}\r\n{}\r\n\r\n{}\r\n\r\n{wait}{}\r\n\r\nF9  Next    F7  Previous view    q / Ctrl+C  Quit\r\nPrevious view never repeats or undoes a completed action.\r\n",
        channel.name,
        channel.id,
        display.status,
        display.step,
        display.role,
        display.title,
        display.say.replace('\n', "\r\n"),
        detail
    ))
}
fn paint(text: &str, theme: &str) -> Result<()> {
    let color = match theme {
        "forest" => 32,
        "sunset" => 33,
        "ocean" => 36,
        "midnight" => 35,
        "classic" => 37,
        _ => 36,
    };
    execute!(io::stdout(), cursor::MoveTo(0, 0), Clear(ClearType::All))?;
    let text = text.replace(RESET, &format!("{RESET}\x1b[{color}m"));
    print!("\x1b[{color}m{text}{RESET}");
    io::stdout().flush()?;
    Ok(())
}
pub fn doctor(channel: &Channel, manifest: &Path, cancelled: &AtomicBool) -> Result<()> {
    let scratch = Scratch::new()?;
    let worker = Worker::start(channel, manifest, &scratch.0)?;
    worker.send("check", None, 30)?;
    let result = worker.wait(cancelled);
    worker.cleanup();
    result?;
    println!("Ready: {} [{}]", channel.name, channel.id);
    Ok(())
}
/// How long an adapter may take to answer the first rehearse before the dry run
/// continues as narration only.
const REHEARSE_PROBE_SECONDS: u64 = 30;
/// The next navigation command: a terminal key, else what the presenter window sent.
/// Each one is logged with its source (never narration) so an unexpected step can be traced.
fn key_input(panel: Option<String>) -> Result<Option<String>> {
    let mut input = panel;
    let mut source = "panel";
    if event::poll(Duration::from_millis(100))? {
        if let TerminalEvent::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                let typed = match key.code {
                    // Only F9/F7 and the window's buttons navigate; a stray Enter must not.
                    KeyCode::F(9) => Some("next"),
                    KeyCode::F(7) => Some("previous"),
                    KeyCode::Char('q') | KeyCode::Esc => Some("quit"),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some("quit")
                    }
                    _ => None,
                };
                if let Some(command) = typed {
                    input = Some(command.into());
                    source = "terminal";
                }
            }
        }
    }
    if let Some(command) = &input {
        crate::usage::event(
            "navigation",
            "observed",
            serde_json::json!({"command":command,"source":source}),
        );
    }
    Ok(input)
}
/// Walk a scenario's narration without preparing or running anything. The
/// adapter, when it answers `rehearse`, may only open or focus windows.
fn dry_run(
    channel: Channel,
    manifest: PathBuf,
    theme: &str,
    no_overlay: bool,
    cancelled: Arc<AtomicBool>,
    operation: crate::usage::Operation,
) -> Result<()> {
    let total = channel.steps.len();
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        println!(
            "Dry run: {} [{}]. Nothing is executed.\n",
            channel.name, channel.id
        );
        for (index, step) in channel.steps.iter().enumerate() {
            println!(
                "{}/{}  {}  {}\n{}\n",
                index + 1,
                total,
                step.role,
                step.title,
                step.say
            );
            if !step.wait.is_empty() {
                println!("Wait: {}\n", step.wait);
            }
        }
        operation.finish(
            "success",
            serde_json::json!({"completed":0,"total":total,"scenario_id":channel.id,"mode":"dry_run"}),
        );
        return Ok(());
    }
    let scratch = Scratch::new()?;
    let worker = Worker::start(&channel, &manifest, &scratch.0).ok();
    let mut windows = false;
    if let Some(worker) = &worker {
        println!("Opening windows for a dry run of {}…", channel.name);
        worker.send("rehearse", None, REHEARSE_PROBE_SECONDS)?;
        windows = worker.wait(&cancelled).is_ok();
    }
    let ready = "Dry run: nothing is executed. Speak, then press F9.";
    let mut detail = if windows {
        "Dry run: nothing is executed. Press F9 to begin."
    } else {
        "Dry run, narration only: the adapter opened no windows. Press F9 to begin."
    }
    .to_owned();
    let (input_tx, input_rx) = mpsc::channel();
    let mut view: Option<usize> = None;
    display(
        &channel,
        &Position::default(),
        None,
        "Dry run",
        &detail,
        &scratch.0,
    )?;
    let panel = if no_overlay {
        None
    } else {
        Some(overlay(&scratch.0, input_tx)?)
    };
    let screen = Screen::enter()?;
    let mut pending = false;
    let mut drawn = String::new();
    let mut furthest = 0;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        if let Some(worker) = &worker {
            while let Ok(event) = worker.events.try_recv() {
                match event {
                    Event::Progress(message) => detail = message,
                    Event::Done(result) => {
                        pending = false;
                        detail = match result {
                            Ok(_) => ready.into(),
                            Err(error) => {
                                format!("Window not opened: {error}. Nothing is executed.")
                            }
                        };
                    }
                }
            }
        }
        let position = Position { completed: 0, view };
        let text = display(&channel, &position, None, "Dry run", &detail, &scratch.0)?;
        if text != drawn {
            paint(&text, theme)?;
            drawn = text;
        }
        let target = match key_input(input_rx.try_recv().ok())?.as_deref() {
            Some("quit") => break,
            Some("next") if !pending => match view.map_or(0, |v| v + 1) {
                index if index < total => Some(index),
                _ => {
                    detail = "End of the scenario. F7 goes back; q quits.".into();
                    None
                }
            },
            Some("previous") if !pending => view.filter(|v| *v > 0).map(|v| v - 1),
            _ => None,
        };
        if let Some(index) = target {
            view = Some(index);
            furthest = furthest.max(index + 1);
            match &worker {
                Some(worker) if windows => {
                    worker.send(
                        "rehearse",
                        Some(&channel.steps[index].id),
                        channel.steps[index].timeout_seconds,
                    )?;
                    pending = true;
                    detail = "Opening windows…".into();
                }
                _ => detail = ready.into(),
            }
        }
    }
    drop(screen);
    drop(panel);
    if let Some(worker) = &worker {
        worker.cleanup();
    }
    operation.finish(
        if cancelled.load(Ordering::Relaxed) {
            "canceled"
        } else {
            "success"
        },
        serde_json::json!({"completed":furthest,"total":total,"scenario_id":channel.id,"mode":"dry_run"}),
    );
    Ok(())
}
pub fn run(
    channel: Channel,
    manifest: PathBuf,
    theme: &str,
    auto: bool,
    no_overlay: bool,
    dry: bool,
    cancelled: Arc<AtomicBool>,
) -> Result<()> {
    let operation = crate::usage::Operation::start("scenario_run");
    let mut channel = channel;
    channel.name = channel.display_name(&manifest);
    if dry {
        return dry_run(channel, manifest, theme, no_overlay, cancelled, operation);
    }
    let scratch = Scratch::new()?;
    let worker = Worker::start(&channel, &manifest, &scratch.0)?;
    println!("Preparing {} [{}]…", channel.name, channel.id);
    worker.send("prepare", None, channel.steps[0].timeout_seconds)?;
    if let Err(error) = worker.wait(&cancelled) {
        journal(&channel, &Position::default(), "PreparationError")?;
        return Err(error);
    }
    let mut position = Position::default();
    if auto {
        let deadline = Instant::now() + Duration::from_secs(channel.session_seconds);
        let result = (|| -> Result<()> {
            for (index, step) in channel.steps.iter().enumerate() {
                if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    bail!("Session stopped before next step");
                }
                println!(
                    "{}/{}  {}  {}\n{}",
                    index + 1,
                    channel.steps.len(),
                    step.role,
                    step.title,
                    step.say
                );
                worker.send("run", Some(&step.id), step.timeout_seconds)?;
                worker.wait(&cancelled)?;
                position.finish(index, true);
                journal(&channel, &position, "Ready")?;
            }
            Ok(())
        })();
        worker.cleanup();
        journal(
            &channel,
            &position,
            if result.is_ok() { "Finished" } else { "Error" },
        )?;
        operation.finish(if result.is_ok(){"success"}else if cancelled.load(Ordering::Relaxed){"canceled"}else{"error"},serde_json::json!({"completed":position.completed,"total":channel.steps.len(),"scenario_id":channel.id,"mode":"auto"}));
        return result;
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        worker.cleanup();
        bail!("Interactive run requires a terminal; use --auto for an explicit scripted run");
    }
    let (input_tx, input_rx) = mpsc::channel();
    display(
        &channel,
        &position,
        None,
        "Ready",
        "Press F9 to begin",
        &scratch.0,
    )?;
    let panel = if no_overlay {
        None
    } else {
        Some(overlay(&scratch.0, input_tx)?)
    };
    let screen = Screen::enter()?;
    let mut pending: Option<(usize, bool)> = None;
    let mut failure = None;
    // The step whose run failed. It stays on screen; F9 skips it once the presenter has done it by hand.
    let mut failed: Option<usize> = None;
    let mut detail = "Press F9 to begin".to_owned();
    let mut drawn = String::new();
    let deadline = Instant::now() + Duration::from_secs(channel.session_seconds);
    loop {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        if Instant::now() >= deadline {
            failure = Some("Session time limit reached".to_owned());
            break;
        }
        while let Ok(event) = worker.events.try_recv() {
            match event {
                Event::Progress(message) => detail = message,
                Event::Done(result) => {
                    if let Some((index, execute)) = pending.take() {
                        match result {
                            Ok(_) => {
                                position.finish(index, execute);
                                detail = if position.completed == channel.steps.len() {
                                    "Demo complete. Previous view is still available.".into()
                                } else {
                                    "Step complete. Speak, then press F9.".into()
                                };
                            }
                            Err(error) => {
                                failure = Some(error.to_string());
                                if execute {
                                    failed = Some(index);
                                    detail = format!(
                                        "{error}\r\nDo this step by hand, then press F9 to skip it; it won't run again. q stops."
                                    );
                                } else {
                                    detail = error.to_string();
                                }
                            }
                        }
                        journal(
                            &channel,
                            &position,
                            if failure.is_some() { "Error" } else { "Ready" },
                        )?;
                    }
                }
            }
        }
        let status = if failure.is_some() {
            "Error"
        } else if pending.is_some() {
            "Working"
        } else {
            "Ready"
        };
        let text = display(
            &channel,
            &position,
            pending.map(|v| v.0).or(failed),
            status,
            &detail,
            &scratch.0,
        )?;
        if text != drawn {
            paint(&text, theme)?;
            drawn = text;
        }
        let input = key_input(input_rx.try_recv().ok())?;
        match input.as_deref() {
            Some("quit") => break,
            Some("next") if pending.is_none() && failed.is_some() => {
                if let Some(index) = failed.take() {
                    position.finish(index, true);
                    failure = None;
                    detail = if position.completed == channel.steps.len() {
                        "Skipped the failed step without running it again. Demo complete.".into()
                    } else {
                        "Skipped the failed step without running it again. Speak, then press F9."
                            .into()
                    };
                    journal(&channel, &position, "Skipped")?;
                }
            }
            Some("next") if pending.is_none() && failure.is_none() => {
                if let Some((index, execute)) = position.next(channel.steps.len()) {
                    worker.send(
                        if execute { "run" } else { "preview" },
                        Some(&channel.steps[index].id),
                        channel.steps[index].timeout_seconds,
                    )?;
                    pending = Some((index, execute));
                    detail = if execute {
                        "Executing step…".into()
                    } else {
                        "Viewing completed step…".into()
                    };
                }
            }
            Some("previous") if pending.is_none() && failure.is_none() => {
                if let Some(index) = position.previous() {
                    worker.send(
                        "preview",
                        Some(&channel.steps[index].id),
                        channel.steps[index].timeout_seconds,
                    )?;
                    pending = Some((index, false));
                    detail = "Viewing completed step…".into();
                }
            }
            _ => (),
        }
    }
    drop(screen);
    drop(panel);
    worker.cleanup();
    journal(
        &channel,
        &position,
        if failure.is_some() {
            "Error"
        } else {
            "Stopped"
        },
    )?;
    operation.finish(if failure.is_some(){"error"}else if position.completed==channel.steps.len(){"success"}else{"canceled"},serde_json::json!({"completed":position.completed,"total":channel.steps.len(),"scenario_id":channel.id,"mode":"interactive"}));
    if let Some(error) = failure {
        bail!("{error}");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::Position;
    #[test]
    fn replay_does_not_repeat_mutations() {
        let mut position = Position::default();
        assert_eq!(position.next(3), Some((0, true)));
        position.finish(0, true);
        position.finish(1, true);
        position.finish(position.previous().unwrap(), false);
        assert_eq!(position.completed, 2);
        assert_eq!(position.next(3), Some((1, false)));
        position.finish(1, false);
        assert_eq!(position.next(3), Some((2, true)));
        position.finish(2, true);
        assert_eq!(position.next(3), None);
    }
}
