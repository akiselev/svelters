//! AU2: the out-of-process transport (E2E-INSPECTOR §1.9). [`Session::call`]'s schema, unchanged,
//! as NDJSON JSON-RPC 2.0 ([`crate::rpc`]) over a local socket.
//!
//! Threads (std only, no async runtime, as everywhere in svelters):
//! - a **listener** thread accepts connections;
//! - per connection, a **reader** thread parses lines and a **writer** thread sends replies, so
//!   the UI thread never touches the socket;
//! - the **UI thread** drains requests with [`Server::pump`] (never blocks) between frames, or
//!   [`Server::pump_wait`] in a headless runner that has nothing else to do, and runs each through
//!   the session. Requests from all connections run in arrival order, one at a time.
//!
//! The reader answers `app.health` and `app.interrupt` itself, without the UI thread, so a hung app
//! can be diagnosed and a runaway JS loop broken ([`Server::set_interrupt`]: the host's
//! `interrupt_flag`).
//!
//! **Off by default.** [`Server::start`] fails with `automation.disabled` until the app has called
//! [`crate::enable`], and needs an open [`Session`]; nothing listens before. The socket is local
//! only: a Unix socket with mode 0600, or TCP on the loopback address. Tokens and per-client
//! policy are not here yet (the user's cut, 2026-10-22); `session.hello`'s `token` is ignored.

use crate::rpc::{self, LineRead, Request};
use crate::{Driver, Error, Session};
use serde_json::{Value, json};
use std::io::{self, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Where the socket is. Text form ([`Endpoint::parse`]): `unix:/path`, a bare path (Unix), or
/// `tcp:PORT` (loopback only; `tcp:0` picks a free port, and [`Server::address`] says which).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Endpoint {
    #[cfg(unix)]
    Unix(PathBuf),
    /// A port on 127.0.0.1.
    Tcp(u16),
}

impl Endpoint {
    pub fn parse(s: &str) -> Result<Endpoint, Error> {
        if let Some(port) = s.strip_prefix("tcp:") {
            return port
                .parse()
                .map(Endpoint::Tcp)
                .map_err(|_| Error::new("transport.address", format!("`{s}`: want tcp:PORT")));
        }
        #[cfg(unix)]
        {
            let p = s.strip_prefix("unix:").unwrap_or(s);
            if p.is_empty() {
                return Err(Error::new("transport.address", "empty socket path"));
            }
            Ok(Endpoint::Unix(PathBuf::from(p)))
        }
        #[cfg(not(unix))]
        Err(Error::new(
            "transport.address",
            format!("`{s}`: only tcp:PORT on this platform (named pipes come later)"),
        ))
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(unix)]
            Endpoint::Unix(p) => write!(f, "unix:{}", p.display()),
            Endpoint::Tcp(port) => write!(f, "tcp:{port}"),
        }
    }
}

/// An app dispatch in front of a session ([`Server::pump_session`]).
pub type SessionCall<'a, D> = dyn FnMut(&mut Session<D>, &str, &Value) -> Result<Value, Error> + 'a;

/// One connection's byte stream (Unix or loopback TCP).
pub(crate) enum Stream {
    #[cfg(unix)]
    Unix(UnixStream),
    Tcp(TcpStream),
}

impl Stream {
    pub(crate) fn connect(e: &Endpoint) -> io::Result<Stream> {
        Ok(match e {
            #[cfg(unix)]
            Endpoint::Unix(p) => Stream::Unix(UnixStream::connect(p)?),
            Endpoint::Tcp(port) => {
                let s = TcpStream::connect((Ipv4Addr::LOCALHOST, *port))?;
                s.set_nodelay(true)?;
                Stream::Tcp(s)
            }
        })
    }

    pub(crate) fn try_clone(&self) -> io::Result<Stream> {
        Ok(match self {
            #[cfg(unix)]
            Stream::Unix(s) => Stream::Unix(s.try_clone()?),
            Stream::Tcp(s) => Stream::Tcp(s.try_clone()?),
        })
    }

    pub(crate) fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Stream::Unix(s) => s.set_read_timeout(t),
            Stream::Tcp(s) => s.set_read_timeout(t),
        }
    }

    pub(crate) fn shutdown(&self) {
        let _ = match self {
            #[cfg(unix)]
            Stream::Unix(s) => s.shutdown(std::net::Shutdown::Both),
            Stream::Tcp(s) => s.shutdown(std::net::Shutdown::Both),
        };
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Stream::Unix(s) => s.read(buf),
            Stream::Tcp(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Stream::Unix(s) => s.write(buf),
            Stream::Tcp(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Stream::Unix(s) => s.flush(),
            Stream::Tcp(s) => s.flush(),
        }
    }
}

enum Listener {
    #[cfg(unix)]
    Unix(UnixListener),
    Tcp(TcpListener),
}

impl Listener {
    fn accept(&self) -> io::Result<Stream> {
        Ok(match self {
            #[cfg(unix)]
            Listener::Unix(l) => Stream::Unix(l.accept()?.0),
            Listener::Tcp(l) => {
                let (s, peer) = l.accept()?;
                if !peer.ip().is_loopback() {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "not a loopback peer",
                    ));
                }
                s.set_nodelay(true)?;
                Stream::Tcp(s)
            }
        })
    }
}

/// What the IO thread knows without asking the UI thread (`app.health`).
#[derive(Default)]
struct Health {
    /// The request the UI thread is running, and since when.
    running: Option<(String, Instant)>,
    /// When the UI thread last drained the queue.
    last_pump: Option<Instant>,
    /// The driver's busy reasons after the last request (`None`: settled).
    busy: Option<String>,
    /// Virtual ms on the session's clock after the last request.
    now_ms: f64,
    /// Requests run on the UI thread so far.
    served: u64,
}

struct Shared {
    health: Mutex<Health>,
    interrupt: Mutex<Option<Arc<AtomicBool>>>,
    stop: AtomicBool,
    started: Instant,
}

/// A request queued for the UI thread, with where its reply goes.
struct Incoming {
    req: Request,
    reply: Sender<String>,
}

/// The socket server. Dropping it stops listening, closes nothing already queued, and removes the
/// Unix socket file.
pub struct Server {
    rx: Receiver<Incoming>,
    shared: Arc<Shared>,
    endpoint: Endpoint,
    #[cfg(unix)]
    unlink: Option<PathBuf>,
}

impl Server {
    /// Listens on `endpoint` for `session`'s clients. Fails with `automation.disabled` until the
    /// app has called [`crate::enable`]; a Unix socket path that a live server holds fails with
    /// `transport.address`, a stale one is replaced.
    pub fn start<D: Driver>(session: &Session<D>, endpoint: Endpoint) -> Result<Server, Error> {
        let _ = session;
        if !crate::is_enabled() {
            return Err(Error::new(
                "automation.disabled",
                "automation is off: the app has not called svelters_automation::enable()",
            ));
        }
        let io = |e: io::Error| Error::new("transport.io", format!("{endpoint}: {e}"));
        let (listener, endpoint) = match &endpoint {
            #[cfg(unix)]
            Endpoint::Unix(path) => {
                if path.exists() {
                    if UnixStream::connect(path).is_ok() {
                        return Err(Error::new(
                            "transport.address",
                            format!("{endpoint}: a server is listening there"),
                        ));
                    }
                    std::fs::remove_file(path).map_err(io)?;
                }
                let l = UnixListener::bind(path).map_err(io)?;
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                        .map_err(io)?;
                }
                (Listener::Unix(l), endpoint.clone())
            }
            Endpoint::Tcp(port) => {
                let l = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, *port)))
                    .map_err(io)?;
                let port = l.local_addr().map_err(io)?.port();
                (Listener::Tcp(l), Endpoint::Tcp(port))
            }
        };
        let shared = Arc::new(Shared {
            health: Mutex::new(Health::default()),
            interrupt: Mutex::new(None),
            stop: AtomicBool::new(false),
            started: Instant::now(),
        });
        let (tx, rx) = mpsc::channel();
        {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("svelters-automation-listen".into())
                .spawn(move || listen(listener, tx, shared))
                .map_err(io)?;
        }
        Ok(Server {
            rx,
            shared,
            #[cfg(unix)]
            unlink: match &endpoint {
                Endpoint::Unix(p) => Some(p.clone()),
                Endpoint::Tcp(_) => None,
            },
            endpoint,
        })
    }

    /// Where clients connect (with the port filled in for `tcp:0`).
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// The flag `app.interrupt` sets from the IO thread: the host's `Host::interrupt_flag()`, which
    /// interrupts the running script once.
    pub fn set_interrupt(&self, flag: Arc<AtomicBool>) {
        *self
            .shared
            .interrupt
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(flag);
    }

    /// Runs every queued request through `session` and queues the replies; never blocks. Call it
    /// on the UI thread between frames. Returns how many requests ran.
    pub fn pump<D: Driver>(&self, session: &mut Session<D>) -> usize {
        self.pump_session(
            None,
            &mut |s: &mut Session<D>, m: &str, p: &Value| s.call(m, p),
            session,
        )
    }

    /// [`Server::pump`] for a runner with nothing else to do: waits up to `timeout` for the first
    /// request, then drains the queue.
    pub fn pump_wait<D: Driver>(&self, session: &mut Session<D>, timeout: Duration) -> usize {
        self.pump_session(
            Some(timeout),
            &mut |s: &mut Session<D>, m: &str, p: &Value| s.call(m, p),
            session,
        )
    }

    /// [`Server::pump`] (`wait: None`) or [`Server::pump_wait`] with the app's own dispatch in
    /// front of the session's: extra methods, or answers decorated with app facts, and
    /// `session.call` for the rest.
    pub fn pump_session<D: Driver>(
        &self,
        wait: Option<Duration>,
        call: &mut SessionCall<'_, D>,
        session: &mut Session<D>,
    ) -> usize {
        let mut n = 0;
        let mut next = match wait {
            Some(t) => self.rx.recv_timeout(t).map_err(|_: RecvTimeoutError| ()),
            None => self.rx.try_recv().map_err(|_| ()),
        };
        while let Ok(i) = next {
            self.begin(&i);
            let outcome = call(session, &i.req.method, &i.req.params);
            self.end(
                i,
                outcome,
                Some((session.driver().busy(), session.now_ms())),
            );
            n += 1;
            next = self.rx.try_recv().map_err(|_| ());
        }
        self.lock().last_pump = Some(Instant::now());
        n
    }

    /// [`Server::pump`] with any dispatcher (cadatomic's, with the session's methods mounted in
    /// it). Never blocks. `app.health` then reports no busy reasons or clock.
    pub fn pump_with(&self, call: &mut dyn FnMut(&str, &Value) -> Result<Value, Error>) -> usize {
        let mut n = 0;
        while let Ok(i) = self.rx.try_recv() {
            self.begin(&i);
            let outcome = call(&i.req.method, &i.req.params);
            self.end(i, outcome, None);
            n += 1;
        }
        self.lock().last_pump = Some(Instant::now());
        n
    }

    fn begin(&self, i: &Incoming) {
        self.lock().running = Some((i.req.method.clone(), Instant::now()));
    }

    fn end(
        &self,
        i: Incoming,
        outcome: Result<Value, Error>,
        state: Option<(Option<String>, f64)>,
    ) {
        {
            let mut h = self.lock();
            h.running = None;
            h.served += 1;
            if let Some((busy, now)) = state {
                h.busy = busy;
                h.now_ms = now;
            }
        }
        if let Some(id) = &i.req.id {
            // The writer thread may be gone (the client hung up): nothing to tell.
            let _ = i.reply.send(rpc::reply(id, outcome).to_string());
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Health> {
        self.shared.health.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        // Wake the listener's blocking accept so it sees `stop`.
        let _ = Stream::connect(&self.endpoint);
        #[cfg(unix)]
        if let Some(p) = &self.unlink {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn listen(listener: Listener, tx: Sender<Incoming>, shared: Arc<Shared>) {
    loop {
        let conn = listener.accept();
        if shared.stop.load(Ordering::SeqCst) {
            return;
        }
        let Ok(conn) = conn else { continue };
        let (tx, shared) = (tx.clone(), shared.clone());
        let _ = std::thread::Builder::new()
            .name("svelters-automation-conn".into())
            .spawn(move || serve(conn, tx, shared));
    }
}

/// One connection: this thread reads; a writer thread owns the write half.
fn serve(conn: Stream, tx: Sender<Incoming>, shared: Arc<Shared>) {
    let Ok(mut write_half) = conn.try_clone() else {
        return;
    };
    let (out, out_rx) = mpsc::channel::<String>();
    let writer = std::thread::Builder::new()
        .name("svelters-automation-write".into())
        .spawn(move || {
            for line in out_rx {
                if write_half
                    .write_all(line.as_bytes())
                    .and_then(|_| write_half.write_all(b"\n"))
                    .and_then(|_| write_half.flush())
                    .is_err()
                {
                    break;
                }
            }
            write_half.shutdown();
        });
    if writer.is_err() {
        return;
    }
    let mut r = BufReader::new(conn);
    let mut buf = Vec::new();
    loop {
        match rpc::read_line(&mut r, &mut buf) {
            Ok(LineRead::Line) => {}
            Ok(LineRead::TooLong) => {
                let _ = out.send(
                    rpc::error_reply(
                        &Value::Null,
                        rpc::INVALID_REQUEST,
                        &Error::new("rpc.request", "line too long"),
                    )
                    .to_string(),
                );
                break;
            }
            Ok(LineRead::Eof) | Err(_) => break,
        }
        if buf.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let req = match rpc::parse_request(&buf) {
            Ok(r) => r,
            Err(reply) => {
                let _ = out.send(reply.to_string());
                continue;
            }
        };
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        match io_thread_answer(&req, &shared) {
            Some(outcome) => {
                if let Some(id) = &req.id {
                    let _ = out.send(rpc::reply(id, outcome).to_string());
                }
            }
            None => {
                if tx
                    .send(Incoming {
                        req,
                        reply: out.clone(),
                    })
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

/// The methods the IO thread answers itself (`None`: the UI thread's).
fn io_thread_answer(req: &Request, shared: &Shared) -> Option<Result<Value, Error>> {
    let method = req.method.as_str();
    if !matches!(method, "app.health" | "app.interrupt") {
        return None;
    }
    if !crate::is_enabled() {
        return Some(Err(Error::new("automation.disabled", "automation is off")));
    }
    Some(Ok(match method {
        "app.health" => {
            let h = shared.health.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            json!({
                "uptime_ms": now.duration_since(shared.started).as_secs_f64() * 1000.0,
                "running": h.running.as_ref().map(|(m, t)| json!({"method": m, "for_ms": now.duration_since(*t).as_secs_f64() * 1000.0})),
                "last_pump_ms_ago": h.last_pump.map(|t| now.duration_since(t).as_secs_f64() * 1000.0),
                "busy": h.busy,
                "settled": h.busy.is_none(),
                "clock_ms": h.now_ms,
                "served": h.served,
            })
        }
        _ => {
            let flag = shared
                .interrupt
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            let running = shared
                .health
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .running
                .as_ref()
                .map(|(m, _)| m.clone());
            match flag {
                Some(f) => {
                    f.store(true, Ordering::SeqCst);
                    json!({"interrupted": true, "running": running})
                }
                None => {
                    return Some(Err(Error::new(
                        "capability.absent",
                        "this app gave the server no interrupt flag",
                    )));
                }
            }
        }
    }))
}
