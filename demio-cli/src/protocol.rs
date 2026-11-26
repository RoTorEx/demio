use crate::registry::Channel;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub enum Event {
    Done(Result<Value>),
    CleanupComplete,
}

pub struct Progress {
    pub detail: String,
    pub waiting: bool,
}

const MAX_REPLY_BYTES: usize = 1024 * 1024;

fn read_replies(mut stdout: impl BufRead, replies: SyncSender<Result<Value>>) {
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        // Allow CRLF after a full-size reply. Reject longer payloads before EOF.
        match (&mut stdout)
            .take((MAX_REPLY_BYTES + 2) as u64)
            .read_until(b'\n', &mut buffer)
        {
            Ok(0) => break,
            Ok(_) => (),
            Err(error) => {
                let _ = replies.send(Err(error.into()));
                break;
            }
        }
        let line = buffer
            .strip_suffix(b"\n")
            .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
            .unwrap_or(&buffer);
        if line.len() > MAX_REPLY_BYTES {
            let _ = replies.send(Err(anyhow::anyhow!("Adapter reply exceeded one MiB")));
            break;
        }
        let reply =
            serde_json::from_slice(line).context("Adapter stdout must contain JSON lines only");
        if replies.send(reply).is_err() {
            break;
        }
    }
}

#[derive(Debug)]
pub struct SessionExpired;

impl std::fmt::Display for SessionExpired {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("The scenario reached its time limit. Inspect the application.")
    }
}

impl std::error::Error for SessionExpired {}

#[derive(Debug)]
pub struct Rejected(String);

impl std::fmt::Display for Rejected {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Rejected {}

#[derive(Debug)]
pub struct RouteChanged(pub String);

impl std::fmt::Display for RouteChanged {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RouteChanged {}

pub struct Operation {
    pub op: String,
    pub step: Option<String>,
    pub timeout: u64,
}
pub struct Worker {
    tx: Option<Sender<Operation>>,
    pub events: Receiver<Event>,
    progress: Arc<Mutex<Option<Progress>>>,
    child: Arc<Mutex<Child>>,
    handle: Option<JoinHandle<()>>,
}
struct Adapter {
    stdin: ChildStdin,
    replies: Receiver<Result<Value>>,
    sequence: u64,
    session: String,
    progress: Arc<Mutex<Option<Progress>>>,
}
impl Adapter {
    fn call(&mut self, op: &Operation) -> Result<Value> {
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
            let reply_id = reply.get("id").and_then(Value::as_u64);
            if reply_id != Some(id) {
                // A final reply can arrive after an operation timed out. Only cleanup
                // can consume such a reply and keep waiting for its own response.
                if op.op == "cleanup" && reply_id.is_some_and(|received| received < id) {
                    continue;
                }
                bail!("Adapter response ID does not match the active operation");
            }
            match reply.get("event").and_then(Value::as_str) {
                Some("status") => {
                    if let Ok(mut progress) = self.progress.lock() {
                        *progress = Some(Progress {
                            detail: reply
                                .get("detail")
                                .and_then(Value::as_str)
                                .unwrap_or("Working")
                                .chars()
                                .take(500)
                                .collect(),
                            waiting: reply.get("waiting").and_then(Value::as_bool) == Some(true),
                        });
                    }
                }
                Some("done") => match reply.get("ok").and_then(Value::as_bool) {
                    Some(true) => return Ok(reply),
                    Some(false) => {
                        let error = reply
                            .get("error")
                            .and_then(Value::as_str)
                            .unwrap_or("Adapter reported a failure");
                        if reply.get("route_changed").and_then(Value::as_bool) == Some(true) {
                            return Err(RouteChanged(error.into()).into());
                        }
                        return Err(Rejected(error.into()).into());
                    }
                    None => bail!("Adapter done event must include a boolean ok"),
                },
                _ => bail!("Adapter event must be status or done"),
            }
        }
    }
}
impl Worker {
    pub fn start(
        channel: &Channel,
        manifest: &Path,
        session: &Path,
        terminal_controls: bool,
    ) -> Result<Self> {
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
                "DEMIO_TERMINAL_CONTROLS",
                if terminal_controls { "1" } else { "0" },
            )
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
        let (reply_tx, replies) = mpsc::sync_channel(4);
        thread::spawn(move || read_replies(BufReader::new(stdout), reply_tx));
        let progress = Arc::new(Mutex::new(None));
        let mut adapter = Adapter {
            stdin,
            replies,
            sequence: 0,
            session: session.to_string_lossy().into(),
            progress: progress.clone(),
        };
        let (tx, requests) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let handle = thread::spawn(move || {
            while let Ok(operation) = requests.recv() {
                let result = adapter.call(&operation);
                if operation.op == "cleanup" {
                    let _ = event_tx.send(Event::CleanupComplete);
                    break;
                }
                if event_tx.send(Event::Done(result)).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            tx: Some(tx),
            events,
            progress,
            child,
            handle: Some(handle),
        })
    }
    pub fn send(&self, op: &str, step: Option<&str>, timeout: u64) -> Result<()> {
        let _ = self.take_progress();
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
    pub fn take_progress(&self) -> Option<Progress> {
        self.progress.lock().ok()?.take()
    }

    pub fn wait(&self, cancelled: &AtomicBool, deadline: Option<Instant>) -> Result<Value> {
        loop {
            let poll = Duration::from_millis(100);
            let remaining =
                deadline.map_or(poll, |end| end.saturating_duration_since(Instant::now()));
            if remaining.is_zero() {
                return Err(SessionExpired.into());
            }
            if cancelled.load(Ordering::Relaxed) {
                bail!("Cancelled");
            }
            let event = self.events.recv_timeout(poll.min(remaining));
            if let Some(progress) = self.take_progress() {
                eprintln!("{}", progress.detail);
            }
            match event {
                Ok(Event::Done(result)) => {
                    if deadline.is_some_and(|end| Instant::now() >= end) {
                        return Err(SessionExpired.into());
                    }
                    return result;
                }
                Ok(Event::CleanupComplete) => bail!("Adapter has closed"),
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(error) => return Err(error.into()),
            }
        }
    }
    pub fn cleanup(&self, deadline: Option<Instant>) {
        let now = Instant::now();
        let grace = now + Duration::from_secs(2);
        let end = deadline.map_or(grace, |deadline| deadline.min(grace));
        if end <= now {
            return;
        }
        if self.send("cleanup", None, 2).is_ok() {
            while Instant::now() < end {
                match self
                    .events
                    .recv_timeout(end.saturating_duration_since(Instant::now()))
                {
                    Ok(Event::CleanupComplete) | Err(_) => break,
                    Ok(_) => (),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reads_a_reply_at_the_limit_with_crlf() {
        let mut bytes = br#"{"id":1,"event":"done","ok":true}"#.to_vec();
        bytes.resize(MAX_REPLY_BYTES, b' ');
        bytes.extend_from_slice(b"\r\n");
        let (sender, receiver) = mpsc::sync_channel(1);
        read_replies(Cursor::new(bytes), sender);
        assert_eq!(receiver.recv().unwrap().unwrap()["ok"], true);
    }

    #[test]
    fn rejects_an_oversized_line_without_reading_its_end() {
        let source = Cursor::new(vec![b'x'; MAX_REPLY_BYTES * 3]);
        let mut reader = BufReader::new(source);
        let (sender, receiver) = mpsc::sync_channel(1);
        read_replies(&mut reader, sender);
        assert!(
            receiver
                .recv()
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("one MiB")
        );
        assert!(reader.get_ref().position() < (MAX_REPLY_BYTES * 2) as u64);
    }
}
