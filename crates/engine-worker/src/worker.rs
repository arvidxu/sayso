use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use dictation_types::{AppError, AppResult};

use crate::client::WorkerCommand;
use crate::wire::{
    REPLY_MARK, Reply, Request, parse_bytes, read_raw_line, write_line, write_samples,
};

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Returns how the worker ended.
pub(crate) fn stop(process: &Mutex<Child>) -> String {
    let mut process = lock(process);
    // Killing an already-ended process fails, which is fine.
    process.kill().ok();
    process
        .wait()
        .map_or_else(|err| err.to_string(), |status| status.to_string())
}

pub(crate) struct Pipes {
    to_worker: ChildStdin,
    from_worker: BufReader<ChildStdout>,
}

impl Pipes {
    pub(crate) fn send(&mut self, request: &Request, samples: &[f32]) -> AppResult<()> {
        write_line(&mut self.to_worker, "", request)?;
        if samples.is_empty() {
            return Ok(());
        }
        write_samples(&mut self.to_worker, samples)
    }

    /// `None` when the worker is gone.
    pub(crate) fn reply(&mut self) -> AppResult<Option<Reply>> {
        next_reply(&mut self.from_worker)
    }
}

/// Engine text on the same stream is forwarded to stderr whatever its encoding; only what
/// follows the reply mark is parsed.
fn next_reply(input: &mut impl BufRead) -> AppResult<Option<Reply>> {
    while let Some(line) = read_raw_line(input)? {
        let (chatter, reply) = split_reply(&line);
        if !chatter.is_empty() {
            writeln!(std::io::stderr(), "{}", String::from_utf8_lossy(chatter)).ok();
        }
        if let Some(reply) = reply {
            return parse_bytes(reply).map(Some);
        }
    }
    Ok(None)
}

/// Engine text without a line end can come just before a reply.
fn split_reply(line: &[u8]) -> (&[u8], Option<&[u8]>) {
    let mark = REPLY_MARK.as_bytes();
    match line.windows(mark.len()).position(|part| part == mark) {
        Some(at) => (&line[..at], Some(&line[at + mark.len()..])),
        None => (line, None),
    }
}

pub(crate) struct Worker {
    pub(crate) process: Arc<Mutex<Child>>,
    pub(crate) pipes: Pipes,
}

impl Worker {
    pub(crate) fn start(command: &WorkerCommand) -> AppResult<Self> {
        let mut launch = Command::new(&command.program);
        launch
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        without_console_window(&mut launch);
        let mut process = launch.spawn().map_err(|err| {
            AppError::asr(format!(
                "the recognition worker '{}' could not be started: {err}",
                command.program.display()
            ))
        })?;
        if let Err(err) = end_with_this_program(&process) {
            stop(&Mutex::new(process));
            return Err(err);
        }
        match (process.stdin.take(), process.stdout.take()) {
            (Some(to_worker), Some(from_worker)) => Ok(Self {
                process: Arc::new(Mutex::new(process)),
                pipes: Pipes {
                    to_worker,
                    from_worker: BufReader::new(from_worker),
                },
            }),
            _ => {
                stop(&Mutex::new(process));
                Err(AppError::internal("the recognition worker has no pipes"))
            }
        }
    }
}

/// Without this, a console-less parent gets a console window per worker, and closing it
/// would end the worker.
#[cfg(windows)]
fn without_console_window(launch: &mut Command) {
    use std::os::windows::process::CommandExt;

    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    launch.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn without_console_window(_launch: &mut Command) {}

/// The worker joins a job that the system kills when this program's last handle to it closes.
/// Elsewhere the worker watches its parent itself (see `host`).
#[cfg(windows)]
fn end_with_this_program(process: &Child) -> AppResult<()> {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;

    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    /// Kept open for the life of this program.
    struct Job(windows_sys::Win32::Foundation::HANDLE);
    // SAFETY: a job handle may be used from any thread.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();
    let job = JOB.get_or_init(|| {
        // SAFETY: system calls with a valid, zeroed limit structure.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).unwrap_or(0),
            );
            (set != 0).then_some(Job(job))
        }
    });
    let Some(job) = job else {
        return Err(AppError::internal(
            "the system refused to set up ending recognition workers with this program",
        ));
    };
    // SAFETY: both handles are valid for the duration of the call.
    let assigned = unsafe { AssignProcessToJobObject(job.0, process.as_raw_handle().cast()) };
    if assigned == 0 {
        return Err(AppError::internal(format!(
            "the recognition worker could not be tied to this program: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
fn end_with_this_program(_process: &Child) -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_is_found_anywhere_in_a_line() {
        let reply = format!("{REPLY_MARK}{{\"loaded\":true}}");
        let loaded = &b"{\"loaded\":true}"[..];
        assert_eq!(split_reply(reply.as_bytes()), (&b""[..], Some(loaded)));
        let glued = format!("no line end{reply}");
        assert_eq!(
            split_reply(glued.as_bytes()),
            (&b"no line end"[..], Some(loaded))
        );
        let chatter = &b"{\"text\":\"chatter\"}"[..];
        assert_eq!(split_reply(chatter), (chatter, None));
    }

    #[test]
    fn text_in_another_encoding_before_a_reply_is_passed_over() {
        // Engine text in a non-UTF-8 code page, on its own lines and glued to the reply.
        let mut stream = b"Modell geladen: Gr\xf6\xdfe 12\r\n\x82\xa0\x82\xa2\n".to_vec();
        stream.extend_from_slice(b"\xe9t\xe9 ");
        stream.extend_from_slice(REPLY_MARK.as_bytes());
        stream.extend_from_slice(b"{\"text\":\"\xc3\xa9t\xc3\xa9\"}\r\n");
        let mut input = std::io::Cursor::new(stream);
        match next_reply(&mut input) {
            Ok(Some(Reply::Text(text))) => assert_eq!(text, "\u{e9}t\u{e9}"),
            Ok(_) => panic!("not the reply that was sent"),
            Err(err) => panic!("the reply was not read: {err}"),
        }
        assert!(
            next_reply(&mut input).unwrap().is_none(),
            "the stream ended"
        );
    }
}
