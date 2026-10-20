use crate::registry::Channel;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub enum Event {
    Progress(String),
    Done(Result<Value>),
}
pub struct Operation {
    pub op: String,
    pub step: Option<String>,
    pub timeout: u64,
}
pub struct Worker {
    tx: Option<Sender<Operation>>,
    pub events: Receiver<Event>,
    child: Arc<Mutex<Child>>,
    handle: Option<JoinHandle<()>>,
}
struct Adapter {
    stdin: ChildStdin,
    replies: Receiver<Result<Value>>,
    sequence: u64,
    session: String,
}
impl Adapter {
    fn call(&mut self, op: Operation, events: &Sender<Event>) -> Result<Value> {
        self.sequence += 1;
        let id = self.sequence;
        serde_json::to_writer(
            &mut self.stdin,
            &json!({"id":id,"op":op.op,"step":op.step,"session_dir":self.session}),
        )?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        let deadline = Instant::now() + Duration::from_secs(op.timeout);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                bail!("Adapter timed out; step outcome is unknown and will not be retried");
            }
            let reply = self
                .replies
                .recv_timeout(left)
                .context("Adapter stopped responding or timed out; outcome is unknown")??;
            if reply.get("id").and_then(Value::as_u64) != Some(id) {
                bail!("Adapter response ID does not match the active operation");
            }
            match reply.get("event").and_then(Value::as_str) {
                Some("status") => {
                    let _ = events.send(Event::Progress(
                        reply
                            .get("detail")
                            .and_then(Value::as_str)
                            .unwrap_or("Working")
                            .chars()
                            .take(500)
                            .collect(),
                    ));
                }
                Some("done") => {
                    if reply.get("ok").and_then(Value::as_bool) != Some(true) {
                        bail!(
                            "{}",
                            reply
                                .get("error")
                                .and_then(Value::as_str)
                                .unwrap_or("Adapter reported failure; outcome is unknown")
                        );
                    }
                    return Ok(reply);
                }
                _ => bail!("Adapter event must be status or done"),
            }
        }
    }
}
impl Worker {
    pub fn start(channel: &Channel, manifest: &Path, session: &Path) -> Result<Self> {
        fs::create_dir_all(session)?;
        let workspace = channel.workspace_path(manifest)?;
        let mut command = Command::new(&channel.adapter[0]);
        command
            .args(&channel.adapter[1..])
            .current_dir(&workspace)
            .env("DEMIO_SESSION_DIR", session)
            .env("DEMIO_WORKSPACE", &workspace)
            .env("DEMIO_SCENARIO", manifest)
            .env(
                "DEMIO_PARAMETERS",
                serde_json::to_string(&channel.parameters)?,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .process_group(0);
        let mut child = command.spawn().context(
            "Could not start adapter; verify its executable and dependencies with demio doctor",
        )?;
        let stdin = child.stdin.take().context("Adapter stdin unavailable")?;
        let stdout = child.stdout.take().context("Adapter stdout unavailable")?;
        let child = Arc::new(Mutex::new(child));
        let (reply_tx, replies) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let reply = line.map_err(anyhow::Error::from).and_then(|line| {
                    if line.len() > 1_048_576 {
                        bail!("Adapter reply exceeded one MiB");
                    }
                    serde_json::from_str(&line)
                        .context("Adapter stdout must contain JSON lines only")
                });
                if reply_tx.send(reply).is_err() {
                    break;
                }
            }
        });
        let mut adapter = Adapter {
            stdin,
            replies,
            sequence: 0,
            session: session.to_string_lossy().into(),
        };
        let (tx, requests) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let handle = thread::spawn(move || {
            while let Ok(operation) = requests.recv() {
                let result = adapter.call(operation, &event_tx);
                if event_tx.send(Event::Done(result)).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            tx: Some(tx),
            events,
            child,
            handle: Some(handle),
        })
    }
    pub fn send(&self, op: &str, step: Option<&str>, timeout: u64) -> Result<()> {
        self.tx
            .as_ref()
            .context("Adapter already closed")?
            .send(Operation {
                op: op.into(),
                step: step.map(str::to_owned),
                timeout,
            })?;
        Ok(())
    }
    pub fn wait(&self, cancelled: &std::sync::atomic::AtomicBool) -> Result<Value> {
        loop {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                bail!("Cancelled");
            }
            match self.events.recv_timeout(Duration::from_millis(100)) {
                Ok(Event::Done(result)) => return result,
                Ok(Event::Progress(detail)) => eprintln!("{detail}"),
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(error) => return Err(error.into()),
            }
        }
    }
    pub fn cleanup(&self) {
        if self.send("cleanup", None, 2).is_ok() {
            let end = Instant::now() + Duration::from_secs(2);
            while Instant::now() < end {
                if matches!(
                    self.events.recv_timeout(Duration::from_millis(50)),
                    Ok(Event::Done(_))
                ) {
                    break;
                }
            }
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.tx.take();
        if let Ok(mut child) = self.child.lock() {
            // The child has not been reaped, so its dedicated group ID cannot be reused.
            let group = format!("-{}", child.id());
            let _ = Command::new("/bin/kill")
                .args(["-TERM", "--", &group])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            thread::sleep(Duration::from_millis(150));
            let _ = Command::new("/bin/kill")
                .args(["-KILL", "--", &group])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = child.wait();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
