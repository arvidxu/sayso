//! A local HTTP/1.1 server that answers as each test scripts it and records the requests.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Reply {
    Body(Body),
    Status(u16),
}

#[derive(Debug, Clone)]
pub struct Body {
    pub bytes: Vec<u8>,
    /// Whether `Range: bytes=N-` is honoured with `206`; otherwise it is ignored and the
    /// whole file is sent with `200`.
    pub ranges: bool,
    /// Answers a range with `206` but sends the whole file and says so, as if the range
    /// had started at the first byte.
    pub wrong_offset: bool,
    /// The total size `Content-Range` claims, when it is not the real one.
    pub claimed_total: Option<usize>,
    /// Closes the connection after sending this many bytes of the file.
    pub cut_after: Option<usize>,
    /// Sends the file in pieces of this many bytes, waiting this long before each piece.
    pub slow: Option<(usize, Duration)>,
    /// After sending this many bytes, waits this long with the connection open.
    pub stall_after: Option<(usize, Duration)>,
}

impl Body {
    pub fn new(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
            ranges: true,
            wrong_offset: false,
            claimed_total: None,
            cut_after: None,
            slow: None,
            stall_after: None,
        }
    }

    pub fn without_ranges(self) -> Self {
        Self {
            ranges: false,
            ..self
        }
    }

    pub fn wrong_offset(self) -> Self {
        Self {
            wrong_offset: true,
            ..self
        }
    }

    pub fn claimed_total(self, total: usize) -> Self {
        Self {
            claimed_total: Some(total),
            ..self
        }
    }

    pub fn cut_after(self, sent: usize) -> Self {
        Self {
            cut_after: Some(sent),
            ..self
        }
    }

    pub fn slowly(self, piece: usize, pause: Duration) -> Self {
        Self {
            slow: Some((piece, pause)),
            ..self
        }
    }

    pub fn stall_after(self, sent: usize, wait: Duration) -> Self {
        Self {
            stall_after: Some((sent, wait)),
            ..self
        }
    }

    pub fn reply(self) -> Reply {
        Reply::Body(self)
    }
}

impl Reply {
    pub fn file(bytes: &[u8]) -> Self {
        Body::new(bytes).reply()
    }

    pub fn file_without_ranges(bytes: &[u8]) -> Self {
        Body::new(bytes).without_ranges().reply()
    }

    pub fn file_cut_after(bytes: &[u8], sent: usize) -> Self {
        Body::new(bytes).cut_after(sent).reply()
    }

    pub fn file_slowly(bytes: &[u8], piece: usize, pause: Duration) -> Self {
        Body::new(bytes).slowly(piece, pause).reply()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub path: String,
    pub range: Option<String>,
    pub status: u16,
    /// How many bytes of the file the server sends in its answer, unless the client goes
    /// away first.
    pub body_bytes_sent: usize,
}

#[derive(Default)]
struct Script {
    /// For each path, the replies still to give. The last one is repeated.
    replies: HashMap<String, Vec<Reply>>,
    requests: Vec<Request>,
}

fn lock(script: &Mutex<Script>) -> MutexGuard<'_, Script> {
    script.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Stops when dropped.
pub struct TestServer {
    addr: SocketAddr,
    script: Arc<Mutex<Script>>,
    stop: Arc<AtomicBool>,
}

impl TestServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
        let addr = listener.local_addr().expect("local address");
        let script = Arc::new(Mutex::new(Script::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_script, thread_stop) = (Arc::clone(&script), Arc::clone(&stop));
        thread::spawn(move || {
            for stream in listener.incoming() {
                if thread_stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let script = Arc::clone(&thread_script);
                thread::spawn(move || serve(stream, &script));
            }
        });
        Self { addr, script, stop }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    pub fn route(&self, path: &str, reply: Reply) {
        self.route_in_turn(path, vec![reply]);
    }

    /// Repeats the last reply once the others are used up.
    pub fn route_in_turn(&self, path: &str, replies: Vec<Reply>) {
        assert!(!replies.is_empty(), "at least one reply");
        lock(&self.script).replies.insert(path.to_owned(), replies);
    }

    pub fn requests(&self) -> Vec<Request> {
        lock(&self.script).requests.clone()
    }

    pub fn requests_for(&self, path: &str) -> Vec<Request> {
        self.requests()
            .into_iter()
            .filter(|request| request.path == path)
            .collect()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes the accepting thread so that it sees the stop.
        let _ = TcpStream::connect(self.addr);
    }
}

pub fn unreachable_url(path: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let addr = listener.local_addr().expect("local address");
    drop(listener);
    format!("http://{addr}{path}")
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        206 => "Partial Content",
        404 => "Not Found",
        416 => "Range Not Satisfiable",
        500 => "Internal Server Error",
        _ => "Status",
    }
}

/// The first byte asked for by a `bytes=N-` range.
fn range_start(range: &str) -> Option<usize> {
    range
        .trim()
        .strip_prefix("bytes=")?
        .strip_suffix('-')?
        .parse()
        .ok()
}

fn serve(stream: TcpStream, script: &Mutex<Script>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone the connection"));
    let mut first = String::new();
    if reader.read_line(&mut first).unwrap_or(0) == 0 {
        return;
    }
    let path = first.split_whitespace().nth(1).unwrap_or("").to_owned();
    let mut range = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("range")
        {
            range = Some(value.trim().to_owned());
        }
    }
    let reply = {
        let mut script = lock(script);
        match script.replies.get_mut(&path) {
            Some(replies) if replies.len() > 1 => Some(replies.remove(0)),
            Some(replies) => replies.first().cloned(),
            None => None,
        }
    };
    let plan = plan(reply.unwrap_or(Reply::Status(404)), range.as_deref());
    // Recorded before answering, so that a test sees it as soon as the client is done.
    lock(script).requests.push(Request {
        path,
        range,
        status: plan.status,
        body_bytes_sent: plan.limit,
    });
    send(stream, &plan);
}

struct Plan {
    status: u16,
    head: String,
    payload: Vec<u8>,
    /// How many bytes of `payload` are sent before the connection is closed.
    limit: usize,
    slow: Option<(usize, Duration)>,
    stall_after: Option<(usize, Duration)>,
}

fn plan(reply: Reply, range: Option<&str>) -> Plan {
    let body = match reply {
        Reply::Status(status) => {
            let head = format!(
                "HTTP/1.1 {status} {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                reason(status)
            );
            return Plan {
                status,
                head,
                payload: Vec::new(),
                limit: 0,
                slow: None,
                stall_after: None,
            };
        }
        Reply::Body(body) => body,
    };
    let total = body.bytes.len();
    let start = if body.ranges {
        range.and_then(range_start)
    } else {
        None
    };
    let (status, extra, payload) = match start {
        Some(start) if start >= total => {
            let head = format!(
                "HTTP/1.1 416 {}\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\n\
                 Connection: close\r\n\r\n",
                reason(416)
            );
            return Plan {
                status: 416,
                head,
                payload: Vec::new(),
                limit: 0,
                slow: None,
                stall_after: None,
            };
        }
        Some(_) if body.wrong_offset => (
            206,
            format!(
                "Content-Range: bytes 0-{}/{}\r\n",
                total - 1,
                body.claimed_total.unwrap_or(total)
            ),
            body.bytes,
        ),
        Some(start) => (
            206,
            format!(
                "Content-Range: bytes {start}-{}/{}\r\n",
                total - 1,
                body.claimed_total.unwrap_or(total)
            ),
            body.bytes[start..].to_vec(),
        ),
        None => (200, String::new(), body.bytes),
    };
    let head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: application/octet-stream\r\n\
         Content-Length: {}\r\nAccept-Ranges: bytes\r\n{extra}Connection: close\r\n\r\n",
        reason(status),
        payload.len()
    );
    let limit = body
        .cut_after
        .map_or(payload.len(), |cut| cut.min(payload.len()));
    Plan {
        status,
        head,
        payload,
        limit,
        slow: body.slow,
        stall_after: body.stall_after,
    }
}

fn send(mut stream: TcpStream, plan: &Plan) {
    if stream.write_all(plan.head.as_bytes()).is_ok() {
        let (piece, pause) = plan.slow.unwrap_or((plan.limit.max(1), Duration::ZERO));
        let mut sent = 0;
        let mut stalled = false;
        while sent < plan.limit {
            thread::sleep(pause);
            let mut end = (sent + piece.max(1)).min(plan.limit);
            if let Some((at, wait)) = plan.stall_after {
                if sent >= at && !stalled {
                    stalled = true;
                    thread::sleep(wait);
                } else if sent < at {
                    end = end.min(at.max(sent + 1));
                }
            }
            if stream.write_all(&plan.payload[sent..end]).is_err() {
                break;
            }
            let _ = stream.flush();
            sent = end;
        }
    }
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}
