//! Bài thử giữ một tiến trình OHS (`node app/dist/src/cli.js serve --stdio`) sống lâu.
//!
//! Giao thức stdio của OHS (JSON Lines, `src/transports/stdio-server.ts` của fork):
//! - khởi động xong (đã nạp model, đã mở DB) OHS in `{"ready":true}`; index chạy nền sau đó;
//! - request `{"id":"…","query":"…","options":{…}}` (tìm) hoặc `{"id":"…","action":"status"}`;
//! - phản hồi mang cùng `id`: `{"id","results":[…]}`, `{"id","status":{…}}` hoặc `{"id","error"}`;
//! - request chạy đồng thời, phản hồi có thể về khác thứ tự;
//! - đóng stdin: OHS chờ request đang chạy (tối đa 60 s) rồi thoát.
//!
//! Dòng không có `id` (ví dụ sự kiện index khi bật `OHS_STDIO_EVENTS=1`) bị bỏ qua.

use std::collections::{HashMap, VecDeque};
use std::env;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use unicode_normalization::{UnicodeNormalization, is_nfc};

/// Biến môi trường chỉ tới gói OHS dựng sẵn khi không có cờ.
pub const OHS_DIR_ENV: &str = "PKM_OHS_DIR";

/// Số dòng stderr cuối của OHS giữ lại để báo lỗi.
const STDERR_TAIL_LINES: usize = 40;

/// Lỗi của bài thử. Mỗi biến thể nói rõ việc cần làm.
#[derive(Debug)]
pub enum OhsError {
    /// Không tìm thấy gói OHS hoặc gói thiếu file.
    Package(String),
    /// Không khởi động được tiến trình node.
    Spawn(std::io::Error),
    /// OHS thoát trước khi in `ready`.
    ExitedBeforeReady {
        status: Option<ExitStatus>,
        stderr: String,
    },
    /// Hết thời gian chờ `ready`.
    ReadyTimeout { waited: Duration, stderr: String },
    /// OHS đã thoát hoặc đóng stdout khi request còn chờ.
    Disconnected { stderr: String },
    /// Hết thời gian chờ phản hồi cho một request.
    RequestTimeout { id: String, waited: Duration },
    /// OHS trả `{"error": …}` cho request.
    Remote { id: String, message: String },
    /// Ghi vào stdin của OHS lỗi.
    Io(std::io::Error),
    /// Phản hồi sai định dạng.
    Protocol(String),
}

impl fmt::Display for OhsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package(m) => write!(f, "gói OHS: {m}"),
            Self::Spawn(e) => write!(f, "không chạy được node của OHS: {e}"),
            Self::ExitedBeforeReady { status, stderr } => {
                write!(
                    f,
                    "OHS thoát trước khi sẵn sàng ({status:?}); stderr:\n{stderr}"
                )
            }
            Self::ReadyTimeout { waited, stderr } => {
                write!(f, "OHS chưa sẵn sàng sau {waited:?}; stderr:\n{stderr}")
            }
            Self::Disconnected { stderr } => write!(f, "OHS đã đóng kết nối; stderr:\n{stderr}"),
            Self::RequestTimeout { id, waited } => {
                write!(f, "request {id} không có phản hồi sau {waited:?}")
            }
            Self::Remote { id, message } => write!(f, "OHS báo lỗi cho request {id}: {message}"),
            Self::Io(e) => write!(f, "ghi vào stdin của OHS lỗi: {e}"),
            Self::Protocol(m) => write!(f, "phản hồi OHS sai định dạng: {m}"),
        }
    }
}

impl std::error::Error for OhsError {}

/// Gói OHS dựng sẵn của fork: `node/` (Node đi kèm) và `app/dist/src/cli.js`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OhsPackage {
    pub root: PathBuf,
    pub node: PathBuf,
    pub cli: PathBuf,
}

impl OhsPackage {
    /// Tìm gói: cờ (`--ohs`) trước, rồi `PKM_OHS_DIR`. Không có cả hai là lỗi.
    pub fn locate(flag: Option<&Path>) -> Result<Self, OhsError> {
        match flag {
            Some(dir) => Self::from_dir(dir),
            None => match env::var_os(OHS_DIR_ENV) {
                Some(dir) if !dir.is_empty() => Self::from_dir(Path::new(&dir)),
                _ => Err(OhsError::Package(format!(
                    "chưa chỉ gói OHS: dùng cờ --ohs <thư mục> hoặc đặt {OHS_DIR_ENV}"
                ))),
            },
        }
    }

    /// Kiểm một thư mục gói có đủ node và cli.js.
    pub fn from_dir(dir: &Path) -> Result<Self, OhsError> {
        let node_candidates: &[&str] = if cfg!(windows) {
            &["node/node.exe"]
        } else {
            &["node/bin/node", "node/node"]
        };
        let node = node_candidates
            .iter()
            .map(|rel| dir.join(rel))
            .find(|p| p.is_file())
            .ok_or_else(|| {
                OhsError::Package(format!(
                    "{} không có Node đi kèm ({})",
                    dir.display(),
                    node_candidates.join(" hoặc ")
                ))
            })?;
        let cli = dir.join("app/dist/src/cli.js");
        if !cli.is_file() {
            return Err(OhsError::Package(format!(
                "{} không có app/dist/src/cli.js",
                dir.display()
            )));
        }
        Ok(Self {
            root: dir.to_path_buf(),
            node,
            cli,
        })
    }
}

/// Cấu hình khởi động OHS.
#[derive(Debug, Clone)]
pub struct OhsConfig {
    pub vault: PathBuf,
    /// `OBSIDIAN_DB_PATH` (bản vá `db-path` của fork). `None`: OHS tự chọn DB trong vault.
    pub db: Option<PathBuf>,
    pub ready_timeout: Duration,
    pub shutdown_timeout: Duration,
}

impl OhsConfig {
    pub fn new(vault: impl Into<PathBuf>) -> Self {
        Self {
            vault: vault.into(),
            db: None,
            ready_timeout: Duration::from_secs(120),
            shutdown_timeout: Duration::from_secs(10),
        }
    }
}

/// Trạng thái OHS theo D7, đọc từ `status` của OHS.
#[derive(Debug, Clone, PartialEq)]
pub enum OhsState {
    /// Chưa in `ready`; hoặc đã `ready` nhưng index rỗng và chưa có note nào được nhúng
    /// (đang tải model lần đầu, hoặc chưa quét vault). OHS stdio không báo tiến độ tải model.
    Starting,
    /// Đang index: `pending` note còn trong hàng đợi. `progress` = indexed / (indexed + pending).
    Indexing {
        progress: f64,
        indexed: u64,
        pending: u64,
    },
    /// Không có model dùng được: không nạp hoặc tải được model, hoặc index dựng bằng model
    /// khác model tiến trình đang dùng. OHS ngừng index; nhánh nghĩa không đáng tin.
    ModelMissing {
        detail: String,
    },
    /// Không chạy được (thiếu gói, thoát trước `ready`, khởi tạo index lỗi vì lý do khác).
    Broken {
        detail: String,
    },
    Ready,
}

impl OhsState {
    /// Tên trạng thái dùng trong lỗi `ohs_not_ready` (`ready` thì không phải lỗi).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Indexing { .. } => "indexing",
            Self::ModelMissing { .. } => "model_missing",
            Self::Broken { .. } => "broken",
            Self::Ready => "ready",
        }
    }

    /// Suy trạng thái từ payload `status` của một tiến trình đã `ready`.
    ///
    /// Thứ tự kiểm (theo hành vi đo được ở OHS 0.17.2-vantt.1):
    /// 1. `model` (model đã dựng index) khác `active_model`: `model_missing`.
    /// 2. `indexing_errors` có lỗi `operation: "initialize"` (index ngừng hẳn): `model_missing`
    ///    nếu lỗi nói tới model, ngược lại `broken`.
    /// 3. `pending` > 0: `indexing`.
    /// 4. `total` = 0 và `model` = null (chưa nhúng note nào): `starting`.
    /// 5. Còn lại: `ready`.
    pub fn from_status(status: &Value) -> Self {
        let num = |k: &str| status.get(k).and_then(Value::as_u64).unwrap_or(0);
        let model = status.get("model").and_then(Value::as_str);
        let active = status.get("active_model").and_then(Value::as_str);
        if let (Some(model), Some(active)) = (model, active)
            && model != active
        {
            return Self::ModelMissing {
                detail: format!("index dựng bằng {model}, tiến trình dùng {active}"),
            };
        }
        let init_error = status
            .get("indexing_errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|e| e.get("operation").and_then(Value::as_str) == Some("initialize"))
            .and_then(|e| e.get("message").and_then(Value::as_str));
        if let Some(message) = init_error {
            let lower = message.to_lowercase();
            let detail = message.to_owned();
            return if ["model", "huggingface", "onnx"]
                .iter()
                .any(|k| lower.contains(k))
            {
                Self::ModelMissing { detail }
            } else {
                Self::Broken { detail }
            };
        }
        let (indexed, pending) = (num("indexed"), num("pending"));
        if pending > 0 {
            let progress = indexed as f64 / (indexed + pending) as f64;
            return Self::Indexing {
                progress,
                indexed,
                pending,
            };
        }
        if num("total") == 0 && model.is_none() {
            return Self::Starting;
        }
        Self::Ready
    }
}

/// Đổi chuỗi sang NFC (đường dẫn OHS trả về có thể là NFD như trên đĩa).
pub fn nfc(s: &str) -> String {
    if is_nfc(s) {
        s.to_owned()
    } else {
        s.nfc().collect()
    }
}

/// Chuẩn hoá `path` của mỗi kết quả `search` sang NFC, tại chỗ.
pub fn normalize_result_paths(results: &mut [Value]) {
    for r in results {
        if let Some(Value::String(p)) = r.get_mut("path") {
            *p = nfc(p);
        }
    }
}

/// Một dòng stdout của OHS, đã phân loại.
#[derive(Debug, PartialEq)]
pub enum Line {
    Ready,
    Response {
        id: String,
        body: Value,
    },
    /// Dòng không có `id` (sự kiện) hoặc không phải JSON.
    Other(String),
}

/// Phân loại một dòng stdout.
pub fn parse_line(line: &str) -> Line {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Line::Other(line.to_owned());
    };
    if v.get("ready") == Some(&Value::Bool(true)) {
        return Line::Ready;
    }
    match v.get("id").and_then(Value::as_str) {
        Some(id) => Line::Response {
            id: id.to_owned(),
            body: v.clone(),
        },
        None => Line::Other(line.to_owned()),
    }
}

type Waiters = Arc<Mutex<HashMap<String, Sender<Value>>>>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Một tiến trình OHS stdio sống lâu. `&self` dùng được từ nhiều luồng cùng lúc.
pub struct OhsClient {
    child: Child,
    stdin: Mutex<Option<ChildStdin>>,
    waiters: Waiters,
    next_id: AtomicU64,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    threads: Vec<JoinHandle<()>>,
    shutdown_timeout: Duration,
}

impl OhsClient {
    /// Khởi động `serve --stdio` và chờ `{"ready":true}`.
    pub fn start(pkg: &OhsPackage, cfg: &OhsConfig) -> Result<Self, OhsError> {
        let mut cmd = Command::new(&pkg.node);
        cmd.arg(&pkg.cli)
            .args(["serve", "--stdio"])
            .current_dir(&pkg.root)
            .env("OBSIDIAN_VAULT_PATH", &cfg.vault)
            .env("OHS_DISABLE_UPDATE_CHECK", "1")
            // Như `bin/ohs` của gói: NODE_OPTIONS của máy không được lọt vào Node đi kèm.
            .env_remove("NODE_OPTIONS")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(db) = &cfg.db {
            cmd.env("OBSIDIAN_DB_PATH", db);
        }
        Self::spawn(cmd, cfg.ready_timeout, cfg.shutdown_timeout)
    }

    /// Khởi động một lệnh nói giao thức stdio của OHS (tách ra để test với server giả).
    pub fn spawn(
        mut cmd: Command,
        ready_timeout: Duration,
        shutdown_timeout: Duration,
    ) -> Result<Self, OhsError> {
        let mut child = cmd.spawn().map_err(OhsError::Spawn)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| OhsError::Protocol("không có stdout".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| OhsError::Protocol("không có stderr".into()))?;
        let stdin = child.stdin.take();

        let waiters: Waiters = Arc::default();
        let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::default();
        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let tail = Arc::clone(&stderr_tail);
        let err_thread = thread::spawn(move || {
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
        let out_thread = thread::spawn(move || {
            let mut ready_tx = Some(ready_tx);
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                match parse_line(&line) {
                    Line::Ready => {
                        if let Some(tx) = ready_tx.take() {
                            let _ = tx.send(());
                        }
                    }
                    Line::Response { id, body } => {
                        if let Some(tx) = lock(&w).remove(&id) {
                            let _ = tx.send(body);
                        }
                    }
                    Line::Other(_) => {}
                }
            }
            // stdout đóng: mọi request còn chờ nhận lỗi Disconnected (sender bị huỷ).
            lock(&w).clear();
        });

        let mut client = Self {
            child,
            stdin: Mutex::new(stdin),
            waiters,
            next_id: AtomicU64::new(1),
            stderr_tail,
            threads: vec![out_thread, err_thread],
            shutdown_timeout,
        };

        let started = Instant::now();
        match ready_rx.recv_timeout(ready_timeout) {
            Ok(()) => Ok(client),
            Err(RecvTimeoutError::Disconnected) => {
                let status = client.wait_exit(Duration::from_secs(2));
                Err(OhsError::ExitedBeforeReady {
                    status,
                    stderr: client.stderr_text(),
                })
            }
            Err(RecvTimeoutError::Timeout) => Err(OhsError::ReadyTimeout {
                waited: started.elapsed(),
                stderr: client.stderr_text(),
            }),
        }
    }

    /// PID của tiến trình node (để kiểm không mồ côi).
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// stderr gần nhất của OHS.
    pub fn stderr_text(&self) -> String {
        lock(&self.stderr_tail)
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Gửi một request (gắn `id` mới) và trả kênh nhận phản hồi.
    fn send(&self, mut req: Value) -> Result<(String, Receiver<Value>), OhsError> {
        let id = format!("r{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        let obj = req
            .as_object_mut()
            .ok_or_else(|| OhsError::Protocol("request phải là object".into()))?;
        obj.insert("id".into(), Value::String(id.clone()));
        let (tx, rx) = mpsc::channel();
        lock(&self.waiters).insert(id.clone(), tx);
        let mut line = req.to_string();
        line.push('\n');
        let write = {
            let mut guard = lock(&self.stdin);
            match guard.as_mut() {
                Some(w) => w.write_all(line.as_bytes()).and_then(|()| w.flush()),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "stdin đã đóng",
                )),
            }
        };
        if let Err(e) = write {
            lock(&self.waiters).remove(&id);
            return Err(OhsError::Io(e));
        }
        Ok((id, rx))
    }

    /// Gửi request và chờ phản hồi có cùng `id`. Lỗi từ OHS thành `OhsError::Remote`.
    pub fn call(&self, req: Value, timeout: Duration) -> Result<Value, OhsError> {
        let (id, rx) = self.send(req)?;
        match rx.recv_timeout(timeout) {
            Ok(body) => match body.get("error") {
                Some(e) => Err(OhsError::Remote {
                    id,
                    message: e.as_str().map_or_else(|| e.to_string(), str::to_owned),
                }),
                None => Ok(body),
            },
            Err(RecvTimeoutError::Timeout) => {
                lock(&self.waiters).remove(&id);
                Err(OhsError::RequestTimeout {
                    id,
                    waited: timeout,
                })
            }
            Err(RecvTimeoutError::Disconnected) => Err(OhsError::Disconnected {
                stderr: self.stderr_text(),
            }),
        }
    }

    /// `search`: trả mảng kết quả, `path` đã chuẩn hoá NFC. `options` theo tên của OHS
    /// (`limit`, `mode`, `snippetLength`, `scope`, …).
    pub fn search(
        &self,
        query: &str,
        options: Value,
        timeout: Duration,
    ) -> Result<Vec<Value>, OhsError> {
        let body = self.call(json!({ "query": query, "options": options }), timeout)?;
        let mut results = match body.get("results") {
            Some(Value::Array(a)) => a.clone(),
            other => return Err(OhsError::Protocol(format!("thiếu results: {other:?}"))),
        };
        normalize_result_paths(&mut results);
        Ok(results)
    }

    /// `status` thô của OHS.
    pub fn status(&self, timeout: Duration) -> Result<Value, OhsError> {
        let body = self.call(json!({ "action": "status" }), timeout)?;
        body.get("status")
            .cloned()
            .ok_or_else(|| OhsError::Protocol("thiếu status".into()))
    }

    /// Trạng thái D7 hiện tại.
    pub fn state(&self, timeout: Duration) -> OhsState {
        match self.status(timeout) {
            Ok(s) => OhsState::from_status(&s),
            Err(e) => OhsState::Broken {
                detail: e.to_string(),
            },
        }
    }

    fn wait_exit(&mut self, timeout: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(st)) => return Some(st),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
                _ => return None,
            }
        }
    }

    /// Đóng stdin, chờ OHS tự thoát trong `shutdown_timeout`; quá hạn thì kill.
    /// Trả `true` nếu OHS tự thoát.
    pub fn shutdown(mut self) -> bool {
        self.close()
    }

    fn close(&mut self) -> bool {
        drop(lock(&self.stdin).take());
        let graceful = self.wait_exit(self.shutdown_timeout).is_some();
        if !graceful {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        graceful
    }
}

impl Drop for OhsClient {
    fn drop(&mut self) {
        if !self.threads.is_empty() {
            self.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_turns_decomposed_vietnamese_into_composed() {
        let nfd: String = "Ghi chú thử nghiệm.md".nfd().collect();
        assert_ne!(nfd, "Ghi chú thử nghiệm.md");
        assert_eq!(nfc(&nfd), "Ghi chú thử nghiệm.md");
        assert_eq!(nfc("ascii.md"), "ascii.md");
    }

    #[test]
    fn result_paths_are_normalized_and_other_fields_kept() {
        let nfd: String = "Khách hàng/Đà Nẵng.md".nfd().collect();
        let mut results = vec![json!({"path": nfd, "score": 0.5}), json!({"score": 1})];
        normalize_result_paths(&mut results);
        assert_eq!(results[0]["path"], "Khách hàng/Đà Nẵng.md");
        assert_eq!(results[0]["score"], 0.5);
        assert_eq!(results[1], json!({"score": 1}));
    }

    #[test]
    fn parse_line_classifies_ready_response_and_events() {
        assert_eq!(parse_line(r#"{"ready":true}"#), Line::Ready);
        assert_eq!(
            parse_line(r#"{"id":"r1","results":[]}"#),
            Line::Response {
                id: "r1".into(),
                body: json!({"id":"r1","results":[]})
            }
        );
        assert!(matches!(
            parse_line(r#"{"event":"indexed","paths":[]}"#),
            Line::Other(_)
        ));
        assert!(matches!(parse_line("Loading model..."), Line::Other(_)));
    }

    #[test]
    fn state_from_status_reports_indexing_progress_against_the_queue() {
        // `total` là số note đã có trong DB, không phải số note của vault.
        let s = OhsState::from_status(&json!({
            "total": 180, "indexed": 180, "pending": 540,
            "model": "local:m", "active_model": "local:m", "indexing_errors": []
        }));
        assert_eq!(
            s,
            OhsState::Indexing {
                progress: 0.25,
                indexed: 180,
                pending: 540
            }
        );
        assert_eq!(s.name(), "indexing");
    }

    #[test]
    fn state_from_status_ready_when_nothing_pending() {
        let s = OhsState::from_status(&json!({
            "total": 747, "indexed": 747, "pending": 0,
            "model": "local:m", "active_model": "local:m", "indexing_errors": []
        }));
        assert_eq!(s, OhsState::Ready);
    }

    #[test]
    fn empty_index_before_any_embedding_is_starting() {
        // Ngay sau `ready`, và suốt lúc tải model lần đầu.
        let s = OhsState::from_status(&json!({
            "total": 0, "indexed": 0, "pending": 0, "model": null,
            "active_model": "local:m", "indexing_errors": []
        }));
        assert_eq!(s, OhsState::Starting);
    }

    #[test]
    fn queue_known_but_model_not_loaded_yet_is_indexing_at_zero() {
        let s = OhsState::from_status(&json!({
            "total": 0, "indexed": 0, "pending": 737, "model": null, "active_model": "local:m"
        }));
        assert!(matches!(s, OhsState::Indexing { progress, .. } if progress == 0.0));
    }

    #[test]
    fn state_from_status_flags_model_drift_as_model_missing() {
        let s = OhsState::from_status(&json!({
            "total": 747, "indexed": 747, "pending": 0,
            "model": "local:a", "active_model": "local:b"
        }));
        assert_eq!(s.name(), "model_missing");
    }

    #[test]
    fn model_that_cannot_be_loaded_is_model_missing() {
        let s = OhsState::from_status(&json!({
            "total": 0, "indexed": 0, "pending": 0, "model": null, "active_model": "local:x",
            "indexing_errors": [{"path": "", "operation": "initialize", "code": "INDEXING_FAILED",
                "message": "Unauthorized access to file: \"https://huggingface.co/x/resolve/main/config.json\"."}]
        }));
        assert_eq!(s.name(), "model_missing");
    }

    #[test]
    fn other_initialize_failure_is_broken_and_per_note_errors_are_not() {
        let init = OhsState::from_status(&json!({
            "total": 0, "model": null,
            "indexing_errors": [{"path": "", "operation": "initialize", "message": "disk full"}]
        }));
        assert_eq!(init.name(), "broken");
        let per_note = OhsState::from_status(&json!({
            "total": 10, "model": "local:m", "active_model": "local:m",
            "indexing_errors": [{"path": "a.md", "operation": "index", "message": "bad"}]
        }));
        assert_eq!(per_note, OhsState::Ready);
    }

    #[test]
    fn package_requires_node_and_cli() {
        let dir = env::temp_dir().join(format!("s3-ohs-pkg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("app/dist/src")).unwrap();
        assert!(matches!(
            OhsPackage::from_dir(&dir),
            Err(OhsError::Package(_))
        ));
        let node = if cfg!(windows) {
            "node/node.exe"
        } else {
            "node/bin/node"
        };
        std::fs::create_dir_all(dir.join(node).parent().unwrap()).unwrap();
        std::fs::write(dir.join(node), b"").unwrap();
        assert!(matches!(
            OhsPackage::from_dir(&dir),
            Err(OhsError::Package(_))
        ));
        std::fs::write(dir.join("app/dist/src/cli.js"), b"").unwrap();
        let pkg = OhsPackage::from_dir(&dir).unwrap();
        assert_eq!(pkg.cli, dir.join("app/dist/src/cli.js"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
