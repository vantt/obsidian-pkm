//! Một tiến trình `serve --stdio` của OHS và giao thức JSON Lines của nó (`src/transports/stdio-server.ts` của fork):
//! - khởi động xong OHS in `{"ready":true}`; index chạy nền sau đó;
//! - request `{"id":"…","query":"…","options":{…}}` (tìm) hoặc `{"id":"…","action":"status"}`;
//! - phản hồi mang cùng `id`: `{"id","results":[…]}`, `{"id","status":{…}}` hoặc `{"id","error":"…"}`;
//! - request chạy đồng thời, phản hồi có thể về khác thứ tự;
//! - đóng stdin: OHS chờ request đang chạy (tối đa 60 s) rồi tự thoát.
//!
//! Dòng không có `id` (sự kiện index khi bật `OHS_STDIO_EVENTS=1`) và dòng không phải JSON bị bỏ qua.

use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

/// Số dòng stderr cuối của OHS giữ lại để báo lỗi.
const STDERR_TAIL_LINES: usize = 40;

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
    /// Tiến trình đã thoát, đóng stdout hoặc stdin (ghi lỗi).
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

/// Request đang chờ phản hồi. `closed` bật khi stdout đóng: request gửi sau đó báo lỗi ngay thay vì chờ hết hạn.
#[derive(Default)]
struct Waiters {
    map: HashMap<String, Sender<Value>>,
    closed: bool,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Một tiến trình OHS. `call` dùng được từ nhiều luồng cùng lúc.
pub(crate) struct Proc {
    generation: u64,
    pid: u32,
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    waiters: Arc<Mutex<Waiters>>,
    next_id: AtomicU64,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
}

impl Proc {
    /// Chạy `cmd` (stdin, stdout, stderr được nối pipe ở đây) và bắt đầu đọc stdout, stderr ở hai luồng nền.
    /// Không chờ `ready`: luồng đọc gửi [`Event::Ready`] rồi [`Event::Exited`] vào `events`.
    pub(crate) fn spawn(
        mut cmd: Command,
        generation: u64,
        events: Sender<Event>,
    ) -> io::Result<Self> {
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("không lấy được stdout, stderr của OHS"));
        };
        let stdin = child.stdin.take();
        let waiters: Arc<Mutex<Waiters>> = Arc::default();
        let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::default();

        // Hai luồng đọc tự kết thúc khi pipe đóng (tiến trình thoát hoặc bị kill); không cần join.
        let tail = Arc::clone(&stderr_tail);
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                let mut t = lock(&tail);
                if t.len() == STDERR_TAIL_LINES {
                    t.pop_front();
                }
                t.push_back(line);
            }
        });
        let w = Arc::clone(&waiters);
        thread::spawn(move || {
            let mut ready_sent = false;
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                match parse_line(&line) {
                    Line::Ready if !ready_sent => {
                        ready_sent = true;
                        let _ = events.send(Event::Ready(generation));
                    }
                    Line::Response { id, body } => {
                        if let Some(tx) = lock(&w).map.remove(&id) {
                            let _ = tx.send(body);
                        }
                    }
                    Line::Ready | Line::Other => {}
                }
            }
            // Huỷ mọi sender: request còn chờ nhận `Disconnected` ngay.
            {
                let mut w = lock(&w);
                w.closed = true;
                w.map.clear();
            }
            let _ = events.send(Event::Exited(generation));
        });

        Ok(Self {
            generation,
            pid: child.id(),
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            waiters,
            next_id: AtomicU64::new(1),
            stderr_tail,
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

    /// Gửi `req` (object, chưa có `id`) và chờ phản hồi cùng `id` tối đa `timeout`.
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
        let written = match lock(&self.stdin).as_mut() {
            Some(w) => w.write_all(line.as_bytes()).and_then(|()| w.flush()),
            None => Err(io::ErrorKind::BrokenPipe.into()),
        };
        if written.is_err() {
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

    /// Đóng stdin để OHS tự thoát, chờ tối đa `timeout`; quá hạn thì kill. Luôn reap tiến trình (không để zombie).
    /// Trả mã thoát, kèm `true` nếu tiến trình tự thoát.
    pub(crate) fn close(&self, timeout: Duration) -> (Option<ExitStatus>, bool) {
        drop(lock(&self.stdin).take());
        let mut child = lock(&self.child);
        let deadline = Instant::now() + timeout;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return (Some(status), true),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                _ => break,
            }
        }
        let _ = child.kill();
        (child.wait().ok(), false)
    }
}

impl Drop for Proc {
    /// Lưới an toàn: tiến trình chưa được `close` thì bị kill và reap, không bao giờ mồ côi.
    fn drop(&mut self) {
        let child = self.child.get_mut().unwrap_or_else(PoisonError::into_inner);
        if let Ok(None) = child.try_wait() {
            let _ = child.kill();
            let _ = child.wait();
        }
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
}
