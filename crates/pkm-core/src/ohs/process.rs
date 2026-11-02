//! Một tiến trình `serve --stdio` của OHS và giao thức JSON Lines của nó (`src/transports/stdio-server.ts` của fork):
//! - khởi động xong OHS in `{"ready":true}`; index chạy nền sau đó;
//! - request `{"id":"…","query":"…","options":{…}}` (tìm) hoặc `{"id":"…","action":"status"}`;
//! - phản hồi mang cùng `id`: `{"id","results":[…]}`, `{"id","status":{…}}` hoặc `{"id","error":"…"}`;
//! - request chạy đồng thời, phản hồi có thể về khác thứ tự;
//! - đóng stdin: OHS chờ request đang chạy (tối đa 60 s) rồi tự thoát.
//!
//! Dòng stdout không có `id` (sự kiện index khi bật `OHS_STDIO_EVENTS=1`), không phải JSON, hoặc không phải UTF-8 bị
//! bỏ qua. stdout và stderr luôn được đọc tới EOF: ngừng đọc thì pipe đầy và OHS treo khi ghi.
//!
//! Tiến trình chạy trong nhóm riêng (Unix: process group; Windows: Job Object đóng là giết), nên kill giết cả
//! tiến trình con mà OHS có thể sinh ra.

use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

/// Số dòng stderr cuối của OHS giữ lại để báo lỗi.
const STDERR_TAIL_LINES: usize = 40;

/// Số ký tự tối đa giữ lại của một dòng stderr.
const STDERR_LINE_CHARS: usize = 2000;

/// Thời gian tối đa `close` chờ luồng đọc stderr đọc hết pipe sau khi tiến trình đã thoát hoặc bị giết.
const STDERR_DRAIN: Duration = Duration::from_secs(1);

/// Việc xảy ra với tiến trình, gửi về luồng giám sát. Số là lần khởi động (`generation`) của tiến trình.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Ready(u64),
    /// stdout đóng: tiến trình đã hoặc sắp thoát.
    Exited(u64),
    /// `pkm` tắt module OHS.
    Shutdown,
}

/// Lỗi của một request gửi tới tiến trình.
#[derive(Debug)]
pub(crate) enum CallError {
    /// Tiến trình đã thoát, đóng stdout, hoặc stdin không ghi được nữa.
    Disconnected,
    Timeout(Duration),
    /// OHS trả `{"error": …}`.
    Remote(String),
}

/// Một dòng stdout của OHS, đã phân loại.
#[derive(Debug, PartialEq)]
enum Line {
    Ready,
    Response { id: String, body: Value },
    Other,
}

fn parse_line(line: &str) -> Line {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Line::Other;
    };
    if v.get("ready") == Some(&Value::Bool(true)) {
        return Line::Ready;
    }
    match v.get("id").and_then(Value::as_str) {
        Some(id) => Line::Response {
            id: id.to_owned(),
            body: v,
        },
        None => Line::Other,
    }
}

/// Đọc `r` theo dòng tới EOF và gọi `f` cho từng dòng (bỏ `\n`, `\r\n`). Byte không phải UTF-8 được thay bằng U+FFFD
/// thay vì dừng đọc. Chỉ dừng ở EOF hoặc lỗi I/O thật (pipe hỏng); `read_until` tự thử lại khi bị ngắt.
fn for_each_line(r: impl Read, mut f: impl FnMut(&str)) {
    let mut r = BufReader::new(r);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf);
                f(line.trim_end_matches(['\n', '\r']));
            }
        }
    }
}

/// Request đang chờ phản hồi. `closed` bật khi stdout đóng: request gửi sau đó báo lỗi ngay thay vì chờ hết hạn.
#[derive(Default)]
struct Waiters {
    map: HashMap<String, Sender<Value>>,
    closed: bool,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Một tiến trình OHS. `call` dùng được từ nhiều luồng cùng lúc và không bao giờ chặn quá thời hạn của nó.
pub(crate) struct Proc {
    generation: u64,
    pid: u32,
    /// Tiến trình, kèm `true` khi đã reap (nhóm đã được dọn một lần, không giết theo id nhóm nữa).
    child: Mutex<(Child, bool)>,
    tree: tree::Tree,
    /// Hàng đợi tới luồng ghi stdin. `None` sau khi đóng: luồng ghi ghi nốt rồi đóng stdin.
    writer: Mutex<Option<Sender<Vec<u8>>>>,
    waiters: Arc<Mutex<Waiters>>,
    next_id: AtomicU64,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    /// Đóng (sender bị drop) khi luồng đọc stderr đã đọc tới EOF. `None` sau khi `close` đã chờ một lần.
    stderr_done: Mutex<Option<Receiver<()>>>,
}

impl Proc {
    /// Chạy `cmd` (stdin, stdout, stderr được nối pipe ở đây) với ba luồng nền: đọc stdout, đọc stderr, ghi stdin.
    /// Không chờ `ready`: luồng đọc stdout gửi [`Event::Ready`] rồi [`Event::Exited`] vào `events`.
    pub(crate) fn spawn(
        mut cmd: Command,
        generation: u64,
        events: Sender<Event>,
    ) -> io::Result<Self> {
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        tree::prepare(&mut cmd);
        let mut child = cmd.spawn()?;
        let tree = tree::Tree::attach(&child);
        let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(stderr)) = pipes else {
            kill_and_reap(&tree, &mut child);
            return Err(io::Error::other(
                "không lấy được stdin, stdout, stderr của OHS",
            ));
        };
        let waiters: Arc<Mutex<Waiters>> = Arc::default();
        let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::default();
        let (writer, queue) = mpsc::channel();
        let (stderr_eof, stderr_done) = mpsc::channel();
        let started = start_threads(
            generation,
            Pipes {
                stdin,
                stdout,
                stderr,
            },
            queue,
            Arc::clone(&waiters),
            Arc::clone(&stderr_tail),
            stderr_eof,
            events,
        );
        if let Err(e) = started {
            kill_and_reap(&tree, &mut child);
            return Err(e);
        }
        Ok(Self {
            generation,
            pid: child.id(),
            child: Mutex::new((child, false)),
            tree,
            writer: Mutex::new(Some(writer)),
            waiters,
            next_id: AtomicU64::new(1),
            stderr_tail,
            stderr_done: Mutex::new(Some(stderr_done)),
        })
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    /// `n` dòng stderr cuối của OHS, nối bằng ` | `.
    pub(crate) fn stderr_tail(&self, n: usize) -> String {
        let t = lock(&self.stderr_tail);
        let skip = t.len().saturating_sub(n);
        t.iter()
            .skip(skip)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// Gửi `req` (object, chưa có `id`) và chờ phản hồi cùng `id` tối đa `timeout`. Bước gửi chỉ đặt dòng vào hàng
    /// đợi của luồng ghi, nên OHS ngừng đọc stdin cũng không làm `call` chặn quá `timeout`.
    pub(crate) fn call(
        &self,
        mut req: serde_json::Map<String, Value>,
        timeout: Duration,
    ) -> Result<Value, CallError> {
        let id = format!("r{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        req.insert("id".into(), Value::String(id.clone()));
        let rx = self.register(&id)?;
        let mut line = Value::Object(req).to_string();
        line.push('\n');
        let queued = lock(&self.writer)
            .as_ref()
            .is_some_and(|w| w.send(line.into_bytes()).is_ok());
        if !queued {
            lock(&self.waiters).map.remove(&id);
            return Err(CallError::Disconnected);
        }
        self.wait_reply(&id, &rx, timeout)
    }

    fn register(&self, id: &str) -> Result<Receiver<Value>, CallError> {
        let mut w = lock(&self.waiters);
        if w.closed {
            return Err(CallError::Disconnected);
        }
        let (tx, rx) = mpsc::channel();
        w.map.insert(id.to_owned(), tx);
        Ok(rx)
    }

    fn wait_reply(
        &self,
        id: &str,
        rx: &Receiver<Value>,
        timeout: Duration,
    ) -> Result<Value, CallError> {
        match rx.recv_timeout(timeout) {
            Ok(body) => match body.get("error") {
                Some(e) => Err(CallError::Remote(
                    e.as_str().map_or_else(|| e.to_string(), str::to_owned),
                )),
                None => Ok(body),
            },
            Err(RecvTimeoutError::Timeout) => {
                lock(&self.waiters).map.remove(id);
                Err(CallError::Timeout(timeout))
            }
            Err(RecvTimeoutError::Disconnected) => Err(CallError::Disconnected),
        }
    }

    /// Tiến trình OHS đã thoát chưa (không chặn). Thoát rồi thì giết nốt tiến trình con còn lại trong nhóm, vì chúng
    /// có thể giữ stdout mở và che mất EOF.
    pub(crate) fn try_exit(&self) -> Option<ExitStatus> {
        let mut guard = lock(&self.child);
        let (child, reaped) = &mut *guard;
        let status = child.try_wait().ok().flatten()?;
        if !*reaped {
            *reaped = true;
            self.tree.kill();
        }
        Some(status)
    }

    /// Đóng stdin để OHS tự thoát, chờ tối đa `timeout`; quá hạn thì giết cả nhóm. Luôn reap tiến trình (không để
    /// zombie). Không cần lock nào mà luồng ghi có thể giữ: luồng ghi đang chặn sẽ nhận lỗi khi tiến trình bị giết.
    /// Trả mã thoát, kèm `true` nếu tiến trình tự thoát.
    ///
    /// Trước khi trả, chờ luồng đọc stderr đọc hết pipe (tối đa [`STDERR_DRAIN`]), để [`Proc::stderr_tail`] gọi sau
    /// đó thấy cả những dòng cuối OHS ghi trước khi chết.
    pub(crate) fn close(&self, timeout: Duration) -> (Option<ExitStatus>, bool) {
        let closed = self.close_process(timeout);
        self.wait_stderr_eof();
        closed
    }

    fn close_process(&self, timeout: Duration) -> (Option<ExitStatus>, bool) {
        drop(lock(&self.writer).take());
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.try_exit() {
                return (Some(status), true);
            }
            if Instant::now() >= deadline {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let mut guard = lock(&self.child);
        let (child, reaped) = &mut *guard;
        *reaped = true;
        (kill_and_reap(&self.tree, child), false)
    }

    /// Chờ luồng đọc stderr gặp EOF. Có giới hạn: một tiến trình con lọt khỏi nhóm (Windows không có Job Object) có
    /// thể giữ stderr mở mãi.
    fn wait_stderr_eof(&self) {
        let Some(done) = lock(&self.stderr_done).take() else {
            return;
        };
        let _ = done.recv_timeout(STDERR_DRAIN);
    }
}

impl Drop for Proc {
    /// Lưới an toàn: tiến trình chưa được `close` thì bị giết (cả nhóm) và reap, không bao giờ mồ côi.
    fn drop(&mut self) {
        let (child, _) = self.child.get_mut().unwrap_or_else(PoisonError::into_inner);
        if let Ok(None) = child.try_wait() {
            kill_and_reap(&self.tree, child);
        }
    }
}

/// Giết cả nhóm trước khi reap (trên Unix, id nhóm còn được giữ khi tiến trình chưa được reap), rồi reap.
fn kill_and_reap(tree: &tree::Tree, child: &mut Child) -> Option<ExitStatus> {
    tree.kill();
    let _ = child.kill();
    child.wait().ok()
}

struct Pipes {
    stdin: ChildStdin,
    stdout: std::process::ChildStdout,
    stderr: std::process::ChildStderr,
}

/// Ba luồng nền của một tiến trình. Chúng tự kết thúc khi pipe đóng (tiến trình thoát hoặc bị giết) hoặc hàng đợi
/// ghi bị đóng; không cần join. Riêng luồng stderr báo qua `stderr_eof` khi đọc xong để `close` chờ.
fn start_threads(
    generation: u64,
    pipes: Pipes,
    queue: Receiver<Vec<u8>>,
    waiters: Arc<Mutex<Waiters>>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    stderr_eof: Sender<()>,
    events: Sender<Event>,
) -> io::Result<()> {
    let Pipes {
        mut stdin,
        stdout,
        stderr,
    } = pipes;
    thread::Builder::new()
        .name("ohs-stderr".into())
        .spawn(move || {
            for_each_line(stderr, |line| {
                let mut t = lock(&stderr_tail);
                if t.len() == STDERR_TAIL_LINES {
                    t.pop_front();
                }
                t.push_back(line.chars().take(STDERR_LINE_CHARS).collect());
            });
            // `stderr_eof` bị drop ở đây: `close` thôi chờ.
            drop(stderr_eof);
        })?;
    thread::Builder::new()
        .name("ohs-stdin".into())
        .spawn(move || {
            for buf in queue {
                if stdin.write_all(&buf).and_then(|()| stdin.flush()).is_err() {
                    break;
                }
            }
            // `stdin` bị drop ở đây: OHS nhận EOF.
        })?;
    thread::Builder::new()
        .name("ohs-stdout".into())
        .spawn(move || {
            let mut ready_sent = false;
            for_each_line(stdout, |line| match parse_line(line) {
                Line::Ready if !ready_sent => {
                    ready_sent = true;
                    let _ = events.send(Event::Ready(generation));
                }
                Line::Response { id, body } => {
                    if let Some(tx) = lock(&waiters).map.remove(&id) {
                        let _ = tx.send(body);
                    }
                }
                Line::Ready | Line::Other => {}
            });
            // Huỷ mọi sender: request còn chờ nhận `Disconnected` ngay.
            {
                let mut w = lock(&waiters);
                w.closed = true;
                w.map.clear();
            }
            let _ = events.send(Event::Exited(generation));
        })?;
    Ok(())
}

/// Nhóm tiến trình của OHS để giết cả cây.
#[cfg(unix)]
mod tree {
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command};

    /// Process group riêng, id nhóm bằng PID của OHS.
    pub(crate) struct Tree {
        pgid: Option<libc::pid_t>,
    }

    /// OHS làm trưởng một process group mới (cũng không nhận Ctrl+C của terminal thay cho `pkm`).
    pub(crate) fn prepare(cmd: &mut Command) {
        cmd.process_group(0);
    }

    impl Tree {
        pub(crate) fn attach(child: &Child) -> Self {
            Self {
                pgid: libc::pid_t::try_from(child.id()).ok(),
            }
        }

        /// SIGKILL cho cả nhóm. Gọi trước khi reap OHS, hoặc ngay sau khi reap mà nhóm còn thành viên: POSIX không cấp
        /// lại một PID khi còn process group mang id đó, nên không giết nhầm tiến trình khác.
        pub(crate) fn kill(&self) {
            if let Some(pgid) = self.pgid.filter(|p| *p > 1) {
                // SAFETY: `kill` chỉ là lời gọi hệ thống, không đụng bộ nhớ; id âm nghĩa là cả nhóm.
                unsafe {
                    libc::kill(-pgid, libc::SIGKILL);
                }
            }
        }
    }
}

/// Nhóm tiến trình của OHS để giết cả cây.
#[cfg(windows)]
mod tree {
    use std::os::windows::io::AsRawHandle;
    use std::process::{Child, Command};

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    /// Job Object có `KILL_ON_JOB_CLOSE`: đóng handle (kể cả khi `pkm` chết) là giết mọi tiến trình trong job.
    /// `None` khi không tạo hoặc gán được job; lúc đó chỉ giết được tiến trình OHS.
    pub(crate) struct Tree {
        job: Option<HANDLE>,
    }

    // SAFETY: handle của Job Object dùng được từ mọi luồng; `Tree` chỉ gọi hàm Win32 an toàn luồng với nó.
    unsafe impl Send for Tree {}
    unsafe impl Sync for Tree {}

    pub(crate) fn prepare(_cmd: &mut Command) {}

    impl Tree {
        pub(crate) fn attach(child: &Child) -> Self {
            Self {
                job: new_job(child),
            }
        }

        pub(crate) fn kill(&self) {
            if let Some(job) = self.job {
                // SAFETY: `job` là handle hợp lệ tới khi `Drop` đóng.
                unsafe {
                    TerminateJobObject(job, 1);
                }
            }
        }
    }

    fn new_job(child: &Child) -> Option<HANDLE> {
        // SAFETY: đối số null là hợp lệ (không thuộc tính, không tên); handle được kiểm trước khi dùng.
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return None;
        }
        // SAFETY: struct POD, toàn số 0 là giá trị hợp lệ.
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: con trỏ và kích thước trỏ tới `info` sống tới hết lời gọi; handle tiến trình do `Child` giữ.
        let ok = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) != 0
                && AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) != 0
        };
        if ok {
            Some(job)
        } else {
            // SAFETY: `job` là handle hợp lệ vừa tạo, đóng một lần.
            unsafe {
                CloseHandle(job);
            }
            None
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            if let Some(job) = self.job.take() {
                // SAFETY: đóng một lần handle hợp lệ.
                unsafe {
                    CloseHandle(job);
                }
            }
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod tree {
    use std::process::{Child, Command};

    pub(crate) struct Tree;

    pub(crate) fn prepare(_cmd: &mut Command) {}

    impl Tree {
        pub(crate) fn attach(_child: &Child) -> Self {
            Self
        }

        pub(crate) fn kill(&self) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_line_classifies_ready_response_and_noise() {
        assert_eq!(parse_line(r#"{"ready":true}"#), Line::Ready);
        assert_eq!(
            parse_line(r#"{"id":"r1","results":[]}"#),
            Line::Response {
                id: "r1".into(),
                body: json!({"id":"r1","results":[]})
            }
        );
        assert_eq!(parse_line(r#"{"event":"indexed","paths":[]}"#), Line::Other);
        assert_eq!(parse_line(r#"{"ready":false}"#), Line::Other);
        assert_eq!(parse_line("Loading model..."), Line::Other);
        assert_eq!(parse_line(r#"{"id":7}"#), Line::Other);
    }

    #[test]
    fn lines_with_invalid_utf8_are_kept_and_reading_continues_to_eof() {
        let input: &[u8] = b"a\r\n\xff\xfe bad\nlast without newline";
        let mut got = Vec::new();
        for_each_line(input, |l| got.push(l.to_owned()));
        assert_eq!(got, ["a", "\u{fffd}\u{fffd} bad", "last without newline"]);
    }
}
