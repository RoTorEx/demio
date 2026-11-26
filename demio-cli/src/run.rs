use crate::{
    protocol::{Controls, Event, Rejected, RouteChanged, SessionExpired, Worker},
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
        self.view.and_then(|view| view.checked_sub(1))
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
    scenario_id: &'a str,
    step: String,
    title: &'a str,
    role: &'a str,
    say: &'a str,
    wait: &'a str,
    detail: &'a str,
    generation: u64,
    primary: Option<&'static str>,
    can_previous: bool,
    reviewing: bool,
}
/// Yellow marks a step's wait tip; `paint` restores the theme colour after it.
const YELLOW: &str = "\x1b[1;33m";
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
fn journal(channel: &Channel, position: &Position, status: &str) {
    crate::usage::event(
        "scenario_progress",
        match status {
            "Error" | "PreparationError" | "Outcome unknown" | "Step failed" | "Preview failed" => {
                "error"
            }
            "Finished" => "success",
            "Stopped" => "canceled",
            _ => "observed",
        },
        serde_json::json!({"completed":position.completed,"total":channel.steps.len(),"status":status}),
    );
}

fn terminal_text(name: &str, display: &Display) -> String {
    let wait = if display.wait.is_empty() {
        String::new()
    } else {
        format!(
            "{YELLOW}Wait: {}{RESET}\r\n\r\n",
            display.wait.replace('\n', "\r\n")
        )
    };
    let primary = match display.primary {
        Some("start") => "s  Start scenario    ",
        Some("next") => "F9  Next    ",
        Some("confirm") => "c  Confirm completion    ",
        Some("restore") => "r  Check route    ",
        _ => "",
    };
    let back = if display.can_previous {
        "F7  Previous view    "
    } else {
        ""
    };
    format!(
        "Demio — {} [{}]\r\n\r\n{}  ·  {}  ·  {}\r\n{}\r\n\r\n{}\r\n\r\n{wait}{}\r\n\r\n{primary}{back}q / Ctrl+C  Stop\r\n",
        name,
        display.scenario_id,
        display.status,
        display.step,
        display.role,
        display.title,
        display.say.replace('\n', "\r\n"),
        display.detail.replace('\n', "\r\n")
    )
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
    let worker = Worker::start(channel, manifest, &scratch.0, Controls::Automatic)?;
    worker.send("check", None, 30)?;
    let result = worker.wait(cancelled, None);
    worker.cleanup(None);
    result?;
    println!("Ready: {} [{}]", channel.name, channel.id);
    Ok(())
}
const REHEARSE_PROBE_SECONDS: u64 = 30;
const ROUTE_CHECK_INTERVAL: Duration = Duration::from_millis(500);
const ROUTE_CHECK_TIMEOUT: u64 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Start,
    Next,
    Previous,
    Confirm,
    Restore,
    Quit,
}

impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Next => "next",
            Self::Previous => "previous",
            Self::Confirm => "confirm",
            Self::Restore => "restore",
            Self::Quit => "quit",
        }
    }

    fn from_panel(message: &str, generation: u64) -> Option<Self> {
        if message == "quit" {
            return Some(Self::Quit);
        }
        let (name, sent_generation) = message.split_once(' ')?;
        if sent_generation.parse::<u64>().ok()? != generation {
            return None;
        }
        match name {
            "start" => Some(Self::Start),
            "next" => Some(Self::Next),
            "previous" => Some(Self::Previous),
            "confirm" => Some(Self::Confirm),
            "restore" => Some(Self::Restore),
            _ => None,
        }
    }

    fn from_terminal(event: TerminalEvent) -> Option<Self> {
        let TerminalEvent::Key(key) = event else {
            return None;
        };
        if key.kind != KeyEventKind::Press {
            return None;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(Self::Quit),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Self::Quit),
            KeyCode::Char('s') => Some(Self::Start),
            KeyCode::Char('c') => Some(Self::Confirm),
            KeyCode::Char('r') => Some(Self::Restore),
            KeyCode::F(9) => Some(Self::Next),
            KeyCode::F(7) => Some(Self::Previous),
            _ => None,
        }
    }
}

fn read_action(
    receiver: &mpsc::Receiver<String>,
    generation: u64,
) -> Result<Option<(Action, &'static str)>> {
    let mut chosen = None;
    for message in receiver.try_iter().take(64) {
        match Action::from_panel(&message, generation) {
            Some(Action::Quit) => chosen = Some((Action::Quit, "panel")),
            Some(action) if chosen.is_none() => chosen = Some((action, "panel")),
            _ => (),
        }
    }
    let mut delay = if chosen.is_some() {
        Duration::ZERO
    } else {
        Duration::from_millis(50)
    };
    for _ in 0..64 {
        if !event::poll(delay)? {
            break;
        }
        delay = Duration::ZERO;
        match Action::from_terminal(event::read()?) {
            Some(Action::Quit) => chosen = Some((Action::Quit, "terminal")),
            Some(action) if chosen.is_none() => chosen = Some((action, "terminal")),
            _ => (),
        }
    }
    Ok(chosen)
}

fn journal_action(action: Action, source: &str) {
    crate::usage::event(
        "navigation",
        "observed",
        serde_json::json!({"command":action.name(),"source":source}),
    );
}

#[derive(Clone, Copy)]
enum Work {
    Prepare,
    Step { index: usize, execute: bool },
}

#[derive(Clone, Copy)]
enum RouteIntent {
    Monitor,
    Ready,
    Restore,
    Advance,
    Preview(usize),
}

struct RouteCheck {
    index: usize,
    intent: RouteIntent,
}

enum Previous {
    Preview(usize),
    Review(usize),
}

enum Phase {
    Introduction,
    Ready,
    CheckingRoute,
    // Review uses the displayed step number, with zero for the introduction.
    RouteChanged {
        review: Option<usize>,
    },
    RestoringRoute {
        review: Option<usize>,
    },
    Working(Work),
    Confirm {
        index: usize,
        recovery: bool,
    },
    Blocked {
        index: Option<usize>,
        status: &'static str,
    },
}

struct Presenter {
    phase: Phase,
    position: Position,
    detail: String,
    dry: bool,
    waiting: bool,
    rehearsal_windows: bool,
    generation: u64,
    deadline: Option<Instant>,
    guarded: bool,
    monitor_route: bool,
    can_restore_route: bool,
    route_check: Option<RouteCheck>,
    next_route_check: Instant,
}

impl Presenter {
    fn new(dry: bool) -> Self {
        Self {
            phase: Phase::Introduction,
            position: Position::default(),
            detail: if dry {
                "Start scenario opens the rehearsal. Steps will be read aloud only."
            } else {
                "Start scenario prepares the demo."
            }
            .into(),
            dry,
            waiting: false,
            rehearsal_windows: false,
            generation: 0,
            deadline: None,
            guarded: false,
            monitor_route: true,
            can_restore_route: false,
            route_check: None,
            next_route_check: Instant::now(),
        }
    }

    fn change(&mut self, phase: Phase, detail: impl Into<String>) {
        self.phase = phase;
        self.detail = detail.into();
        self.waiting = false;
        self.generation += 1;
    }

    fn primary(&self, total: usize) -> Option<Action> {
        match self.phase {
            Phase::Introduction => Some(Action::Start),
            Phase::Ready if self.position.next(total).is_some() => Some(Action::Next),
            Phase::Confirm { .. } => Some(Action::Confirm),
            Phase::RouteChanged { .. } => Some(Action::Restore),
            _ => None,
        }
    }

    fn status(&self, total: usize) -> &'static str {
        match self.phase {
            Phase::Introduction => "Introduction",
            Phase::CheckingRoute => "Checking route",
            Phase::RouteChanged { .. } => "Route changed",
            Phase::RestoringRoute { .. } => "Restoring route",
            Phase::Working(Work::Prepare) => "Preparing",
            Phase::Working(_) if self.waiting => "Waiting",
            Phase::Working(_) => "Working",
            Phase::Confirm { recovery: true, .. } => "Step failed",
            Phase::Confirm { .. } => "Awaiting confirmation",
            Phase::Blocked { status, .. } => status,
            Phase::Ready if self.position.completed == total => "Finished",
            Phase::Ready if self.dry => "Dry run",
            Phase::Ready => "Ready",
        }
    }

    fn route_review(&self) -> Option<usize> {
        match self.phase {
            Phase::RouteChanged { review } | Phase::RestoringRoute { review } => review,
            _ => None,
        }
    }

    fn index(&self) -> Option<usize> {
        if let Some(step) = self.route_review() {
            return step.checked_sub(1);
        }
        match self.phase {
            Phase::Ready
            | Phase::CheckingRoute
            | Phase::RouteChanged { .. }
            | Phase::RestoringRoute { .. } => self.position.view.or(Some(0)),
            Phase::Working(Work::Step { index, .. }) | Phase::Confirm { index, .. } => Some(index),
            Phase::Blocked { index, .. } => index,
            Phase::Introduction | Phase::Working(Work::Prepare) => None,
        }
    }

    fn previous(&self) -> Option<Previous> {
        match self.phase {
            Phase::Ready => self.position.previous().map(Previous::Preview),
            Phase::RouteChanged { .. } => self.index().map(Previous::Review),
            _ => None,
        }
    }

    fn display<'a>(&'a self, channel: &'a Channel) -> Display<'a> {
        let total = channel.steps.len();
        let index = self.index();
        let (title, role, say, wait) = match index {
            Some(index) => {
                let step = &channel.steps[index];
                (&*step.title, &*step.role, &*step.say, &*step.wait)
            }
            None => ("Introduction", "Presenter", channel.introduction(), ""),
        };
        Display {
            status: self.status(total),
            channel: &channel.name,
            scenario_id: &channel.id,
            step: format!("{}/{total}", index.map_or(0, |index| index + 1)),
            title,
            role,
            say,
            wait,
            detail: &self.detail,
            generation: self.generation,
            primary: self.primary(total).map(Action::name),
            can_previous: self.previous().is_some(),
            reviewing: self.route_review().is_some(),
        }
    }

    fn ready(&mut self, total: usize) {
        let detail = match (self.dry, self.position.completed) {
            (true, count) if count == total && self.position.view == Some(total - 1) => {
                "End of the scenario."
            }
            (true, _) if self.rehearsal_windows => "Rehearsal view ready.",
            (true, _) => "Dry run, narration only.",
            (false, 0) => "Setup complete.",
            (false, count) if count == total => "Demo complete.",
            (false, _) => "Step complete.",
        };
        self.change(Phase::Ready, detail);
    }

    fn guarded_step(&self, total: usize) -> Option<usize> {
        if !self.guarded {
            return None;
        }
        self.position
            .next(total)
            .and_then(|(index, execute)| execute.then_some(index))
    }

    fn prepare_forward(&mut self, channel: &Channel, worker: &Worker) {
        if let Some(index) = self.guarded_step(channel.steps.len()) {
            self.verify_route(channel, worker, index, RouteIntent::Ready);
        } else {
            self.ready(channel.steps.len());
        }
    }

    fn verify_route(
        &mut self,
        channel: &Channel,
        worker: &Worker,
        index: usize,
        intent: RouteIntent,
    ) {
        if matches!(intent, RouteIntent::Restore) {
            let review = self.route_review();
            self.change(
                Phase::RestoringRoute { review },
                "Checking the rehearsal view…",
            );
        } else if !matches!(intent, RouteIntent::Monitor) {
            self.change(Phase::CheckingRoute, "Checking the rehearsal view…");
        }
        if let Some(check) = &mut self.route_check {
            debug_assert_eq!(check.index, index);
            check.intent = intent;
            return;
        }
        let step = &channel.steps[index];
        let (operation, timeout) =
            if self.can_restore_route && matches!(intent, RouteIntent::Restore) {
                ("restore", step.timeout_seconds)
            } else {
                ("verify", ROUTE_CHECK_TIMEOUT)
            };
        match worker.send(operation, Some(&step.id), timeout) {
            Ok(()) => self.route_check = Some(RouteCheck { index, intent }),
            Err(error) => self.blocked(Some(index), "Outcome unknown", error.to_string()),
        }
    }

    fn route_changed(&mut self, error: anyhow::Error) {
        let review = self.route_review();
        self.change(Phase::RouteChanged { review }, error.to_string());
    }

    fn finish_route_check(
        &mut self,
        channel: &Channel,
        worker: &Worker,
        check: RouteCheck,
        result: Result<serde_json::Value>,
    ) {
        self.next_route_check = Instant::now() + ROUTE_CHECK_INTERVAL;
        match result {
            Ok(_) => match check.intent {
                RouteIntent::Monitor => (),
                RouteIntent::Ready | RouteIntent::Restore => self.ready(channel.steps.len()),
                RouteIntent::Advance => self.step(channel, Some(worker), check.index, true),
                RouteIntent::Preview(index) => self.step(channel, Some(worker), index, false),
            },
            Err(error) if error.is::<RouteChanged>() || error.is::<Rejected>() => {
                self.route_changed(error);
            }
            Err(error) => self.blocked(
                Some(check.index),
                "Outcome unknown",
                format!("Could not verify the route. {error:#}\nStop and inspect the application."),
            ),
        }
    }

    fn blocked(&mut self, index: Option<usize>, status: &'static str, detail: impl Into<String>) {
        self.change(Phase::Blocked { index, status }, detail);
    }

    fn may_have_pending_operation(&self) -> bool {
        self.route_check.is_some()
            || matches!(
                self.phase,
                Phase::Working(_)
                    | Phase::Blocked {
                        status: "Outcome unknown",
                        ..
                    }
            )
    }

    fn step(&mut self, channel: &Channel, worker: Option<&Worker>, index: usize, execute: bool) {
        if self.dry && !self.rehearsal_windows {
            self.position.finish(index, execute);
            self.ready(channel.steps.len());
            return;
        }
        let (operation, detail) = if self.dry {
            ("rehearse", "Opening the rehearsal view…")
        } else if execute {
            ("run", "Executing step…")
        } else {
            ("preview", "Viewing completed step…")
        };
        let step = &channel.steps[index];
        let result = worker
            .context("Adapter unavailable")
            .and_then(|worker| worker.send(operation, Some(&step.id), step.timeout_seconds));
        match result {
            Ok(()) => self.change(Phase::Working(Work::Step { index, execute }), detail),
            Err(error) => self.blocked(Some(index), "Outcome unknown", error.to_string()),
        }
    }

    fn finish_work(
        &mut self,
        channel: &Channel,
        worker: &Worker,
        work: Work,
        result: Result<serde_json::Value>,
    ) {
        let total = channel.steps.len();
        match (work, result) {
            (Work::Prepare, result) if self.dry => {
                self.rehearsal_windows = result.is_ok();
                self.step(channel, Some(worker), 0, true);
            }
            (Work::Prepare, Ok(reply)) => {
                self.deadline = Some(Instant::now() + Duration::from_secs(channel.session_seconds));
                self.guarded = reply
                    .get("verify_route")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true);
                self.can_restore_route =
                    self.guarded && reply["restore_route"].as_bool() == Some(true);
                self.monitor_route = reply["monitor_route"].as_bool().unwrap_or(true);
                self.prepare_forward(channel, worker);
            }
            (Work::Prepare, Err(error)) => {
                self.blocked(None, "Preparation failed", format!("{error:#}"));
                journal(channel, &self.position, "PreparationError");
            }
            (Work::Step { index, execute }, Err(error)) if self.dry => {
                self.position.finish(index, execute);
                self.rehearsal_windows = error.is::<Rejected>();
                self.change(Phase::Ready, format!("Window not opened. {error:#}"));
            }
            (
                Work::Step {
                    index,
                    execute: true,
                },
                Ok(_),
            ) if !self.dry && channel.steps[index].confirm => {
                self.change(
                    Phase::Confirm {
                        index,
                        recovery: false,
                    },
                    "Check the required result, then confirm completion.",
                );
            }
            (Work::Step { index, execute }, Ok(_)) => {
                self.position.finish(index, execute);
                if !self.dry {
                    journal(
                        channel,
                        &self.position,
                        if self.position.completed == total {
                            "Finished"
                        } else {
                            "Ready"
                        },
                    );
                }
                self.prepare_forward(channel, worker);
            }
            (Work::Step { .. }, Err(error)) if self.guarded && error.is::<RouteChanged>() => {
                if self.guarded_step(total).is_some() {
                    self.route_changed(error);
                } else {
                    self.change(Phase::Ready, format!("{error:#}\nTry the preview again."));
                }
            }
            (
                Work::Step {
                    index,
                    execute: true,
                },
                Err(error),
            ) if error.is::<Rejected>() => {
                self.change(
                    Phase::Confirm {
                        index,
                        recovery: true,
                    },
                    format!(
                        "{error:#}\nConfirm completion only after verifying the step manually."
                    ),
                );
                journal(channel, &self.position, "Step failed");
            }
            (Work::Step { index, .. }, Err(error)) => {
                let status = if error.is::<Rejected>() {
                    "Preview failed"
                } else {
                    "Outcome unknown"
                };
                self.blocked(
                    Some(index),
                    status,
                    format!("{error:#}\nStop and inspect the application."),
                );
                journal(channel, &self.position, self.status(total));
            }
        }
    }
}

fn print_script(channel: &Channel, name: &str) {
    let total = channel.steps.len();
    println!(
        "Dry run: {} [{}]. Nothing is executed.\n\n0/{total}  Introduction\n{}\n",
        name,
        channel.id,
        channel.introduction()
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
        if step.confirm {
            println!("Presenter confirmation required in a live run.\n");
        }
    }
}

fn auto_run(
    channel: &Channel,
    manifest: &Path,
    cancelled: &AtomicBool,
    operation: crate::usage::Operation,
) -> Result<()> {
    if channel.steps.iter().any(|step| step.confirm) {
        bail!("This scenario requires presenter confirmation. Run it interactively.");
    }
    let scratch = Scratch::new()?;
    let worker = Worker::start(channel, manifest, &scratch.0, Controls::Automatic)?;
    let mut position = Position::default();
    let mut prepared = false;
    let mut deadline = None;
    let result = (|| -> Result<()> {
        println!(
            "Preparing {} [{}]…",
            channel.display_name(manifest),
            channel.id
        );
        worker.send("prepare", None, channel.steps[0].timeout_seconds)?;
        worker.wait(cancelled, None)?;
        prepared = true;
        let end = Instant::now() + Duration::from_secs(channel.session_seconds);
        deadline = Some(end);
        for (index, step) in channel.steps.iter().enumerate() {
            if Instant::now() >= end {
                return Err(SessionExpired.into());
            }
            if cancelled.load(Ordering::Relaxed) {
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
            worker.wait(cancelled, deadline)?;
            position.finish(index, true);
            journal(channel, &position, "Ready");
        }
        Ok(())
    })();
    worker.cleanup(deadline);
    drop(worker);
    let (status, outcome) = if result.is_ok() {
        ("Finished", "success")
    } else if cancelled.load(Ordering::Relaxed) {
        ("Stopped", "canceled")
    } else if !prepared {
        ("PreparationError", "error")
    } else {
        ("Error", "error")
    };
    journal(channel, &position, status);
    operation.finish(outcome, serde_json::json!({
        "completed":position.completed,"total":channel.steps.len(),"scenario_id":channel.id,"mode":"auto"
    }));
    result
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
    if auto {
        return auto_run(&channel, &manifest, &cancelled, operation);
    }
    let terminal_name = channel.display_name(&manifest);
    let total = channel.steps.len();
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        if !dry {
            bail!("Interactive run requires a terminal; use --auto for an explicit scripted run");
        }
        print_script(&channel, &terminal_name);
        operation.finish(
            "success",
            serde_json::json!({
                "completed":0,"total":total,"scenario_id":channel.id,"mode":"dry_run"
            }),
        );
        return Ok(());
    }

    let scratch = Scratch::new()?;
    let panel_path = scratch.0.join("panel.json");
    let mut presenter = Presenter::new(dry);
    let initial = presenter.display(&channel);
    atomic_write(&panel_path, &serde_json::to_vec(&initial)?)?;
    let (input_tx, input_rx) = mpsc::channel();
    let panel = if no_overlay {
        None
    } else {
        Some(overlay(&scratch.0, input_tx)?)
    };
    let screen = Screen::enter()?;
    paint(&terminal_text(&terminal_name, &initial), theme)?;
    let mut worker: Option<Worker> = None;
    let mut drawn_generation = presenter.generation;
    let mut progress_changed = false;
    let mut stop_source = None;
    while !cancelled.load(Ordering::Relaxed) {
        if progress_changed || drawn_generation != presenter.generation {
            let frame = presenter.display(&channel);
            atomic_write(&panel_path, &serde_json::to_vec(&frame)?)?;
            paint(&terminal_text(&terminal_name, &frame), theme)?;
            drawn_generation = presenter.generation;
            progress_changed = false;
        }

        // Consume input against the visible phase before applying a worker reply.
        // A key pressed while waiting cannot advance the next step.
        let action = match read_action(&input_rx, presenter.generation)? {
            Some((Action::Quit, source)) => {
                stop_source = Some(source);
                break;
            }
            Some((action, source)) => {
                journal_action(action, source);
                Some(action)
            }
            None => None,
        };
        if presenter.deadline.is_some_and(|end| Instant::now() >= end) {
            let in_flight = presenter.may_have_pending_operation();
            presenter.deadline = None;
            presenter.blocked(
                presenter.index(),
                "Session expired",
                SessionExpired.to_string(),
            );
            if let Some(expired) = worker.take() {
                if !in_flight {
                    expired.cleanup(None);
                }
            }
            continue;
        }
        if action.is_some() && action == presenter.primary(total) {
            match action {
                Some(Action::Start) => {
                    let (op, timeout) = if dry {
                        ("rehearse", REHEARSE_PROBE_SECONDS)
                    } else {
                        ("prepare", channel.steps[0].timeout_seconds)
                    };
                    let controls = if no_overlay {
                        Controls::Terminal
                    } else {
                        Controls::Presenter
                    };
                    let started = Worker::start(&channel, &manifest, &scratch.0, controls)
                        .and_then(|worker| {
                            worker.send(op, None, timeout)?;
                            Ok(worker)
                        });
                    match started {
                        Ok(started) => {
                            worker = Some(started);
                            presenter.change(Phase::Working(Work::Prepare), "Preparing scenario…");
                        }
                        Err(_) if dry => presenter.step(&channel, None, 0, true),
                        Err(error) => {
                            presenter.blocked(None, "Preparation failed", format!("{error:#}"));
                            journal(&channel, &presenter.position, "PreparationError");
                        }
                    }
                }
                Some(Action::Next) => {
                    if let Some((index, execute)) = presenter.position.next(total) {
                        if let Some(worker) =
                            worker.as_ref().filter(|_| execute && presenter.guarded)
                        {
                            presenter.verify_route(&channel, worker, index, RouteIntent::Advance);
                        } else {
                            presenter.step(&channel, worker.as_ref(), index, execute);
                        }
                    }
                }
                Some(Action::Restore) => {
                    if let (Some(index), Some(worker)) = (presenter.guarded_step(total), &worker) {
                        presenter.verify_route(&channel, worker, index, RouteIntent::Restore);
                    }
                }
                Some(Action::Confirm) => {
                    if let Phase::Confirm { index, recovery } = presenter.phase {
                        presenter.position.finish(index, true);
                        if let Some(worker) = &worker {
                            presenter.prepare_forward(&channel, worker);
                        } else {
                            presenter.ready(total);
                        }
                        journal(
                            &channel,
                            &presenter.position,
                            if recovery { "Recovered" } else { "Confirmed" },
                        );
                    }
                }
                _ => (),
            }
            continue;
        }
        if let (Some(Action::Previous), Some(previous)) = (action, presenter.previous()) {
            match previous {
                Previous::Review(step) => {
                    presenter.phase = Phase::RouteChanged { review: Some(step) };
                    presenter.generation += 1;
                }
                Previous::Preview(index) => {
                    if let (Some(upcoming), Some(worker)) = (presenter.guarded_step(total), &worker)
                    {
                        presenter.verify_route(
                            &channel,
                            worker,
                            upcoming,
                            RouteIntent::Preview(index),
                        );
                    } else {
                        presenter.step(&channel, worker.as_ref(), index, false);
                    }
                }
            }
            continue;
        }
        if let Some(worker) = &worker {
            if let (Phase::Working(_), Some(progress)) = (&presenter.phase, worker.take_progress())
            {
                progress_changed =
                    presenter.detail != progress.detail || presenter.waiting != progress.waiting;
                presenter.detail = progress.detail;
                presenter.waiting = progress.waiting;
            }
            while let Ok(event) = worker.events.try_recv() {
                if let Event::Done(result) = event {
                    if let Some(check) = presenter.route_check.take() {
                        presenter.finish_route_check(&channel, worker, check, result);
                    } else if let Phase::Working(work) = presenter.phase {
                        presenter.finish_work(&channel, worker, work, result);
                    }
                }
            }
            let monitor_due = presenter.monitor_route
                && matches!(presenter.phase, Phase::Ready)
                && presenter.route_check.is_none()
                && Instant::now() >= presenter.next_route_check;
            if let Some(index) = presenter.guarded_step(total).filter(|_| monitor_due) {
                presenter.verify_route(&channel, worker, index, RouteIntent::Monitor);
            }
        }
    }
    drop(screen);
    drop(panel);
    if let Some(worker) = worker {
        let deadline = presenter
            .deadline
            .filter(|_| presenter.may_have_pending_operation());
        worker.cleanup(deadline);
    }
    if let Some(source) = stop_source {
        journal_action(Action::Quit, source);
    }
    let failed = matches!(
        presenter.phase,
        Phase::Blocked { .. } | Phase::Confirm { recovery: true, .. }
    );
    let (status, outcome) = if failed {
        ("Error", "error")
    } else if presenter.position.completed == total {
        ("Finished", "success")
    } else {
        ("Stopped", "canceled")
    };
    journal(&channel, &presenter.position, status);
    operation.finish(
        outcome,
        serde_json::json!({
            "completed":presenter.position.completed,"total":total,"scenario_id":channel.id,
            "mode":if dry {"dry_run"} else {"interactive"}
        }),
    );
    if failed {
        bail!("{}", presenter.detail);
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
