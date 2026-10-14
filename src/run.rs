use crate::{
    protocol::{Event, Worker},
    registry::{Channel, atomic_write, home},
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
    detail: &'a str,
}
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
            .unwrap_or(home()?.join("construction_side/demio/runtime"));
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
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or(home()?.join(".local/state"));
    let data = serde_json::json!({"channel_id":channel.id,"completed":position.completed,"view":position.view,"status":status});
    atomic_write(
        &base.join("demio/last-session.json"),
        &serde_json::to_vec_pretty(&data)?,
    )
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
        detail,
    };
    atomic_write(&session.join("panel.json"), &serde_json::to_vec(&display)?)?;
    Ok(format!(
        "Demio — {} [{}]\r\n\r\n{}  ·  {}  ·  {}\r\n{}\r\n\r\n{}\r\n\r\n{}\r\n\r\nF9 / Enter  Next    F7  Previous view    q / Ctrl+C  Quit\r\nPrevious view never repeats or undoes a completed action.\r\n",
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
    print!("\x1b[{color}m{text}\x1b[0m");
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
pub fn run(
    channel: Channel,
    manifest: PathBuf,
    theme: &str,
    auto: bool,
    no_overlay: bool,
    cancelled: Arc<AtomicBool>,
) -> Result<()> {
    let scratch = Scratch::new()?;
    let worker = Worker::start(&channel, &manifest, &scratch.0)?;
    println!("Preparing {} [{}]…", channel.name, channel.id);
    worker.send("prepare", None, channel.steps[0].timeout_seconds)?;
    worker.wait(&cancelled)?;
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
                                detail = error.to_string();
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
            pending.map(|v| v.0),
            status,
            &detail,
            &scratch.0,
        )?;
        if text != drawn {
            paint(&text, theme)?;
            drawn = text;
        }
        let mut input = input_rx.try_recv().ok();
        if event::poll(Duration::from_millis(100))? {
            if let TerminalEvent::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    input = match key.code {
                        KeyCode::F(9) | KeyCode::Enter => Some("next".into()),
                        KeyCode::F(7) => Some("previous".into()),
                        KeyCode::Char('q') | KeyCode::Esc => Some("quit".into()),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            Some("quit".into())
                        }
                        _ => input,
                    };
                }
            }
        }
        match input.as_deref() {
            Some("quit") => break,
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
