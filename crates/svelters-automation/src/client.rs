//! AU2: the Rust client of the out-of-process transport. [`Client::call`] has
//! [`Session::call`](crate::Session::call)'s signature, so test bodies and cadatomic's `RpcDrive`
//! send the same methods in or out of process; [`App`] spawns an app under automation (the
//! reference host `svelters-run`, or any binary that takes `--automation <endpoint>`).

use crate::Error;
use crate::rpc::{self, LineRead};
use crate::server::{Endpoint, Stream};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// One connection. Calls are synchronous: one request, then its reply. Open a second client for
/// calls that must not wait behind a running one (`app.health`, `app.interrupt`).
pub struct Client {
    reader: BufReader<Stream>,
    writer: Stream,
    next: u64,
    buf: Vec<u8>,
}

fn closed(e: impl std::fmt::Display) -> Error {
    Error::new("transport.closed", e.to_string())
}

impl Client {
    pub fn connect(endpoint: &Endpoint) -> Result<Client, Error> {
        let s = Stream::connect(endpoint)
            .map_err(|e| Error::new("transport.io", format!("{endpoint}: {e}")))?;
        Ok(Client {
            writer: s
                .try_clone()
                .map_err(|e| Error::new("transport.io", e.to_string()))?,
            reader: BufReader::new(s),
            next: 1,
            buf: Vec::new(),
        })
    }

    /// [`Client::connect`], retried until `timeout` while the app starts.
    pub fn connect_retry(endpoint: &Endpoint, timeout: Duration) -> Result<Client, Error> {
        let deadline = Instant::now() + timeout;
        loop {
            match Client::connect(endpoint) {
                Ok(c) => return Ok(c),
                Err(e) if Instant::now() >= deadline => return Err(e),
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    /// How long a call may wait for its reply (`None`: forever, the default). A call that times out
    /// fails `timeout` and leaves the connection unusable (its reply would arrive late).
    pub fn set_timeout(&mut self, t: Option<Duration>) -> Result<(), Error> {
        self.reader
            .get_ref()
            .set_read_timeout(t)
            .map_err(|e| Error::new("transport.io", e.to_string()))
    }

    /// Sends `method` and waits for its reply: the in-process `Session::call` result, or its
    /// error code. A dead app fails `transport.closed`.
    pub fn call(&mut self, method: &str, params: &Value) -> Result<Value, Error> {
        let id = self.next;
        self.next += 1;
        let line = rpc::request(id, method, params).to_string();
        self.writer
            .write_all(line.as_bytes())
            .and_then(|_| self.writer.write_all(b"\n"))
            .and_then(|_| self.writer.flush())
            .map_err(closed)?;
        loop {
            match rpc::read_line(&mut self.reader, &mut self.buf) {
                Ok(LineRead::Line) => {}
                Ok(LineRead::Eof) => return Err(closed("the app closed the connection")),
                Ok(LineRead::TooLong) => return Err(Error::new("transport.io", "reply too long")),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(Error::new("timeout", format!("no reply to `{method}`")));
                }
                Err(e) => return Err(closed(e)),
            }
            let reply: Value = serde_json::from_slice(&self.buf)
                .map_err(|e| Error::new("rpc.parse", e.to_string()))?;
            if reply.get("id").and_then(Value::as_u64) == Some(id) {
                return rpc::outcome(&reply);
            }
            // A reply to an earlier call that timed out, or a notification: skip it.
        }
    }
}

/// An app process under automation, and a client connected to it. Dropping it kills the process.
pub struct App {
    pub child: Child,
    pub client: Client,
    pub endpoint: Endpoint,
    /// The line the app printed before listening (`SVELTERS_AUTOMATION=<endpoint>`) and after.
    stdout: Option<BufReader<ChildStdout>>,
}

impl App {
    /// Spawns `program args… --automation <endpoint>` and waits (up to `timeout`) for the app to
    /// print `SVELTERS_AUTOMATION=<endpoint>`, then connects. `tcp:0` lets the app pick the port.
    pub fn spawn(
        program: &Path,
        args: &[&str],
        endpoint: &Endpoint,
        timeout: Duration,
    ) -> Result<App, Error> {
        let mut child = Command::new(program)
            .args(args)
            .arg("--automation")
            .arg(endpoint.to_string())
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
            .map_err(|e| Error::new("transport.io", format!("{}: {e}", program.display())))?;
        let mut out = BufReader::new(child.stdout.take().ok_or_else(|| closed("no stdout"))?);
        let deadline = Instant::now() + timeout;
        let mut line = String::new();
        // The app prints the line once it listens; a crash before closes stdout.
        let endpoint = loop {
            line.clear();
            if out.read_line(&mut line).map_err(closed)? == 0 {
                let status = child.wait().ok();
                return Err(closed(format!(
                    "the app exited before listening ({status:?})"
                )));
            }
            if let Some(e) = line.trim().strip_prefix("SVELTERS_AUTOMATION=") {
                break Endpoint::parse(e)?;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                return Err(Error::new("timeout", "the app did not start listening"));
            }
        };
        let client = Client::connect_retry(&endpoint, timeout)?;
        Ok(App {
            child,
            client,
            endpoint,
            stdout: Some(out),
        })
    }

    pub fn call(&mut self, method: &str, params: &Value) -> Result<Value, Error> {
        self.client.call(method, params)
    }

    /// A second connection (for `app.health` and `app.interrupt` while a call runs).
    pub fn connect(&self) -> Result<Client, Error> {
        Client::connect(&self.endpoint)
    }

    /// Whether the process is still running.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.stdout.take();
    }
}
