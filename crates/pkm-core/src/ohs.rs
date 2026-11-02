//! Module OHS duy nhất của `pkm` (D6, PRD mục 9): giữ một tiến trình `ohs serve --stdio` sống lâu, gửi request có `id`
//! đồng thời từ nhiều luồng, khởi động lại có giới hạn khi OHS chết, và báo trạng thái theo D7.
//!
//! Dùng: [`Ohs::launch`] khi `pkm` khởi động (không chặn: OHS khởi động ở luồng nền), [`Ohs::search`] và
//! [`Ohs::read`] trong tool cần OHS, [`Ohs::state`] để báo trạng thái. Khi OHS chưa sẵn sàng, hai hàm đó trả
//! [`OhsError::NotReady`] và tool
//! trả [`OhsError::not_ready_error`] cho agent; không có đường lùi cho ra kết quả kém (D7). Bỏ [`Ohs`] (hoặc gọi
//! [`Ohs::shutdown`]) thì đóng stdin của OHS, chờ có giới hạn rồi kill: không để lại tiến trình `node` nào.
//!
//! Module không đọc thẳng DB hay `node_modules` của OHS; mọi thứ đi qua giao thức stdio.

mod package;
mod process;
mod state;

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};
use unicode_normalization::{UnicodeNormalization, is_nfc};

pub use package::{DEFAULT_DIR_NAME, LANGUAGE_PACK_ENV, OHS_DIR_ENV, OhsPackage, PackageError};
pub use state::OhsState;

use process::{CallError, Event, Proc, lock};
use state::EtaTracker;

/// Thời gian chờ tiến trình đã đóng stdout thoát hẳn trước khi kill.
const EXIT_GRACE: Duration = Duration::from_secs(2);

/// Số dòng stderr cuối của OHS đưa vào thông báo lỗi.
const STDERR_IN_ERRORS: usize = 5;

/// Số lần liên tiếp `status` bị từ chối hoặc sai định dạng thì coi gói OHS không tương thích (`broken`). Một lần lỗi
/// lẻ không đủ.
const STATUS_ERRORS_BROKEN: u32 = 3;

/// Cấu hình chạy OHS.
#[derive(Debug, Clone)]
pub struct OhsConfig {
    /// Vault cho `OBSIDIAN_VAULT_PATH`.
    pub vault: PathBuf,
    /// `OBSIDIAN_DB_PATH` (bản vá `db-path` của fork). `None`: OHS tự chọn DB như MCP `ohs` của người dùng, nên hai
    /// bên dùng chung một index.
    pub db: Option<PathBuf>,
    /// Chờ `{"ready":true}` tối đa bao lâu cho mỗi lần khởi động.
    pub ready_timeout: Duration,
    /// Chờ phản hồi `search` tối đa bao lâu.
    pub request_timeout: Duration,
    /// Chờ phản hồi `status` tối đa bao lâu.
    pub status_timeout: Duration,
    /// Khi tắt: chờ OHS tự thoát sau khi đóng stdin tối đa bao lâu, rồi kill.
    pub shutdown_timeout: Duration,
    /// Số lần khởi động lại liên tiếp tối đa; lần chết tiếp theo thì thành `broken`.
    pub max_restarts: u32,
    /// Chờ trước lần khởi động lại thứ `n` là `restart_backoff * n`.
    pub restart_backoff: Duration,
    /// Tiến trình đã sống (sau `ready`) ít nhất chừng này rồi mới chết thì đếm lại số lần khởi động lại từ đầu.
    pub stable_after: Duration,
    /// Luồng giám sát hỏi `status` và kiểm tiến trình còn sống mỗi khoảng này. Trạng thái mà `search` dùng là kết
    /// quả lần hỏi gần nhất nếu chưa cũ quá hai khoảng, nên `search` thường không tốn thêm một lượt `status`.
    pub poll_interval: Duration,
}

impl OhsConfig {
    pub fn new(vault: impl Into<PathBuf>) -> Self {
        Self {
            vault: vault.into(),
            db: None,
            ready_timeout: Duration::from_secs(120),
            request_timeout: Duration::from_secs(60),
            status_timeout: Duration::from_secs(5),
            shutdown_timeout: Duration::from_secs(10),
            max_restarts: 3,
            restart_backoff: Duration::from_secs(1),
            stable_after: Duration::from_secs(300),
            poll_interval: Duration::from_secs(2),
        }
    }
}

/// Lỗi của một request tới OHS.
#[derive(Debug, Clone, PartialEq)]
pub enum OhsError {
    /// OHS chưa sẵn sàng (D7): tool trả [`OhsError::not_ready_error`].
    NotReady(OhsState),
    /// Tham số không hợp lệ, chưa gửi đi.
    Invalid(String),
    /// OHS không trả lời trong thời hạn.
    Timeout(Duration),
    /// OHS trả `{"error": …}`.
    Remote(String),
    /// Phản hồi sai định dạng.
    Protocol(String),
}

impl OhsError {
    /// Lỗi `ohs_not_ready` theo D7 khi đây là [`OhsError::NotReady`].
    pub fn not_ready_error(&self) -> Option<Value> {
        match self {
            Self::NotReady(state) => state.not_ready_error(),
            _ => None,
        }
    }
}

impl fmt::Display for OhsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotReady(s) => write!(f, "OHS chưa sẵn sàng ({}): {}", s.name(), s.action()),
            Self::Invalid(m) => write!(f, "request OHS không hợp lệ: {m}"),
            Self::Timeout(d) => write!(f, "OHS không trả lời sau {d:?}"),
            Self::Remote(m) => write!(f, "OHS báo lỗi: {m}"),
            Self::Protocol(m) => write!(f, "phản hồi OHS sai định dạng: {m}"),
        }
    }
}

impl std::error::Error for OhsError {}

/// Tiến trình OHS hiện tại theo con mắt của luồng giám sát.
enum Phase {
    /// Đang khởi động hoặc chờ khởi động lại; có tiến trình khi đã spawn.
    Starting(Option<Arc<Proc>>),
    /// Đã `ready`: nhận request.
    Running(Arc<Proc>),
    /// Không khởi động lại nữa; chuỗi là `action`.
    Broken(String),
    /// `pkm` đang tắt hoặc đã tắt module.
    Stopped,
}

/// Kết quả lần hỏi `status` gần nhất của một tiến trình.
struct Observed {
    generation: u64,
    at: Instant,
    state: OhsState,
    /// Số lần liên tiếp `status` bị từ chối hoặc sai định dạng.
    errors: u32,
}

struct Shared {
    phase: Mutex<Phase>,
    last: Mutex<Option<Observed>>,
    eta: Mutex<EtaTracker>,
    status_timeout: Duration,
    /// Trạng thái đã lưu cũ hơn chừng này thì hỏi lại `status`.
    max_age: Duration,
}

impl Shared {
    fn new(phase: Phase, status_timeout: Duration, poll_interval: Duration) -> Self {
        Self {
            phase: Mutex::new(phase),
            last: Mutex::default(),
            eta: Mutex::default(),
            status_timeout,
            max_age: poll_interval * 2,
        }
    }

    /// Đổi pha, trừ khi module đã dừng.
    fn set(&self, phase: Phase) {
        let mut p = lock(&self.phase);
        if !matches!(*p, Phase::Stopped) {
            *p = phase;
        }
    }

    /// Tiến trình đang nhận request, hoặc trạng thái khi không có.
    fn running(&self) -> Result<Arc<Proc>, OhsState> {
        match &*lock(&self.phase) {
            Phase::Running(p) => Ok(Arc::clone(p)),
            Phase::Starting(_) => Err(OhsState::Starting),
            Phase::Broken(action) => Err(OhsState::Broken {
                action: action.clone(),
            }),
            Phase::Stopped => Err(OhsState::Broken {
                action: "pkm đang tắt module OHS. Khởi động lại pkm.".to_owned(),
            }),
        }
    }

    /// Trạng thái đã lưu của `proc` (bất kể cũ mới), không hỏi OHS.
    fn remembered(&self, proc: &Proc) -> Option<OhsState> {
        lock(&self.last)
            .as_ref()
            .filter(|o| o.generation == proc.generation())
            .map(|o| o.state.clone())
    }

    /// Trạng thái hiện tại không chặn: theo pha, hoặc lần hỏi `status` gần nhất khi tiến trình đang chạy.
    fn peek(&self) -> OhsState {
        match self.running() {
            Ok(proc) => self.remembered(&proc).unwrap_or(OhsState::Starting),
            Err(state) => state,
        }
    }

    /// Trạng thái của `proc`: lần hỏi gần nhất nếu chưa cũ quá `max_age`, ngược lại hỏi lại `status`.
    fn current(&self, proc: &Proc) -> OhsState {
        let fresh = lock(&self.last)
            .as_ref()
            .filter(|o| o.generation == proc.generation() && o.at.elapsed() <= self.max_age)
            .map(|o| o.state.clone());
        fresh.unwrap_or_else(|| self.observe(proc))
    }

    /// Hỏi `status` của tiến trình, suy trạng thái D7 (kèm `eta_s` khi đang index) và lưu lại.
    ///
    /// Lỗi tạm không đổi trạng thái đã biết: `status` hết hạn (OHS bận) thì giữ trạng thái lần trước; mất kết nối
    /// (tiến trình đang chết, luồng giám sát sẽ khởi động lại) thì `starting`. `status` bị từ chối hoặc sai định dạng
    /// [`STATUS_ERRORS_BROKEN`] lần liên tiếp mới là `broken`.
    fn observe(&self, proc: &Proc) -> OhsState {
        let mut req = Map::new();
        req.insert("action".into(), "status".into());
        let reply = proc.call(req, self.status_timeout);
        let previous = self.remembered(proc);
        let parsed = match reply {
            Ok(body) => match body.get("status") {
                Some(s) if s.is_object() => Ok(OhsState::from_status(s)),
                _ => Err("phản hồi status không có object status".to_owned()),
            },
            Err(CallError::Remote(m)) => Err(format!("OHS từ chối lệnh status: {m}")),
            Err(CallError::Timeout(_)) => return previous.unwrap_or(OhsState::Starting),
            Err(CallError::Disconnected) => return OhsState::Starting,
        };
        let mut last = lock(&self.last);
        let errors_before = last
            .as_ref()
            .filter(|o| o.generation == proc.generation())
            .map_or(0, |o| o.errors);
        let (state, errors) = match parsed {
            Ok(mut state) => {
                let mut eta = lock(&self.eta);
                match &mut state {
                    OhsState::Indexing {
                        indexed,
                        pending,
                        eta_s,
                        ..
                    } => {
                        *eta_s = eta.observe(proc.generation(), Instant::now(), *indexed, *pending)
                    }
                    _ => eta.reset(),
                }
                (state, 0)
            }
            Err(detail) => {
                let errors = errors_before + 1;
                let state = if errors >= STATUS_ERRORS_BROKEN {
                    OhsState::Broken {
                        action: incompatible_package_action(&detail),
                    }
                } else {
                    previous.unwrap_or(OhsState::Starting)
                };
                (state, errors)
            }
        };
        *last = Some(Observed {
            generation: proc.generation(),
            at: Instant::now(),
            state: state.clone(),
            errors,
        });
        state
    }
}

fn incompatible_package_action(detail: &str) -> String {
    format!(
        "Gói OHS không nói đúng giao thức pkm cần ({detail}). Cài lại gói OHS đúng phiên bản bằng bộ cài của pkm, \
         rồi khởi động lại pkm."
    )
}

/// Một OHS do `pkm` quản lý. Dùng chung giữa các luồng qua `&Ohs` (hoặc `Arc<Ohs>`).
pub struct Ohs {
    shared: Arc<Shared>,
    events: Option<Sender<Event>>,
    supervisor: Option<JoinHandle<()>>,
    request_timeout: Duration,
}

impl Ohs {
    /// Tìm gói (xem [`OhsPackage::locate`]) rồi khởi động OHS ở nền. Không thấy gói thì OHS ở trạng thái `broken`
    /// với `action` chỉ cách cài; tool không cần OHS vẫn chạy bình thường.
    pub fn launch(flag: Option<&Path>, cfg: OhsConfig) -> Self {
        match OhsPackage::locate(flag) {
            Ok(pkg) => Self::start(pkg, cfg),
            Err(e) => Self::unavailable(OhsState::missing_package(&e)),
        }
    }

    /// Khởi động OHS từ gói đã có ở luồng nền và trả ngay; trạng thái là `starting` tới khi OHS in `ready`.
    pub fn start(pkg: OhsPackage, mut cfg: OhsConfig) -> Self {
        // OHS chạy với thư mục hiện tại là thư mục gói: đường dẫn tương đối phải được giải trước.
        cfg.vault = absolute(&cfg.vault);
        cfg.db = cfg.db.as_deref().map(absolute);
        let shared = Arc::new(Shared::new(
            Phase::Starting(None),
            cfg.status_timeout,
            cfg.poll_interval,
        ));
        let (tx, rx) = mpsc::channel();
        let request_timeout = cfg.request_timeout;
        let supervisor = Supervisor {
            shared: Arc::clone(&shared),
            pkg,
            cfg,
            tx: tx.clone(),
            rx,
        };
        match thread::Builder::new()
            .name("ohs-supervisor".into())
            .spawn(move || supervisor.run())
        {
            Ok(handle) => Self {
                shared,
                events: Some(tx),
                supervisor: Some(handle),
                request_timeout,
            },
            Err(e) => Self::unavailable(OhsState::Broken {
                action: format!("pkm không tạo được luồng giám sát OHS ({e}). Khởi động lại pkm."),
            }),
        }
    }

    /// OHS không chạy được ngay từ đầu (thiếu gói): mọi tool cần OHS nhận `state` (thường là `broken`).
    pub fn unavailable(state: OhsState) -> Self {
        let phase = match state {
            OhsState::Broken { action } => Phase::Broken(action),
            other => Phase::Broken(other.action()),
        };
        Self {
            shared: Arc::new(Shared::new(phase, Duration::ZERO, Duration::ZERO)),
            events: None,
            supervisor: None,
            request_timeout: Duration::ZERO,
        }
    }

    /// Trạng thái hiện tại. Khi tiến trình đang chạy: lần hỏi `status` gần nhất của luồng giám sát, hoặc hỏi lại
    /// (chặn tối đa `status_timeout`) nếu lần đó đã cũ.
    pub fn state(&self) -> OhsState {
        match self.shared.running() {
            Ok(proc) => self.shared.current(&proc),
            Err(state) => state,
        }
    }

    /// Chờ tới khi OHS `ready`, thành `broken`, hoặc hết `timeout`; trả trạng thái cuối. Không tự hỏi `status` (dựa
    /// vào lần hỏi định kỳ của luồng giám sát), nên không chờ quá `timeout` đáng kể. Cho lệnh chạy một lần (CLI);
    /// tool MCP không chờ mà trả `ohs_not_ready` ngay (D7).
    pub fn wait_ready(&self, timeout: Duration) -> OhsState {
        let deadline = Instant::now() + timeout;
        loop {
            let state = self.shared.peek();
            let left = deadline.saturating_duration_since(Instant::now());
            if state.is_ready() || state.is_terminal() || left.is_zero() {
                return state;
            }
            thread::sleep(left.min(Duration::from_millis(200)));
        }
    }

    /// Tìm bằng OHS. `options` là object tuỳ chọn của OHS (`limit`, `mode`, `snippetLength`, `scope`, …) hoặc `null`.
    /// Đường dẫn trong mỗi kết quả được chuẩn hoá NFC (OHS trả NFD) để tra được trong index, xem
    /// [`normalize_result_paths`].
    ///
    /// Kiểm `status` trước: OHS in `ready` trước khi index xong và lúc đó tìm trả `[]` không báo lỗi, nên chỉ tìm khi
    /// trạng thái là `ready`; ngược lại trả [`OhsError::NotReady`].
    pub fn search(&self, query: &str, options: Value) -> Result<Vec<Value>, OhsError> {
        if query.trim().is_empty() {
            return Err(OhsError::Invalid("câu tìm rỗng".into()));
        }
        let options = options_object(options)?;
        let mut req = Map::new();
        req.insert("query".into(), query.into());
        req.insert("options".into(), Value::Object(options));
        self.request(req, "results")
    }

    /// Đọc note bằng action `read` của OHS: mỗi phần tử của `paths` cho đúng một phần tử kết quả, cùng thứ tự. Note có
    /// trong index là `{path, found: true, title, aliases, tags, content, links, backlinks, markdownLinks,
    /// markdownBacklinks, urls}`, không có là `{path, found: false, suggestions}`. `options` là các khoá còn lại của
    /// request (`snippetLength`, `related`) hoặc `null`; OHS bỏ qua khoá nó không biết. Đường dẫn được chuẩn hoá NFC như
    /// [`Ohs::search`]. OHS tự đưa `paths` về NFD khi tra, nên gửi NFC được.
    ///
    /// Chỉ đọc khi trạng thái là `ready`, như [`Ohs::search`]: lúc đang index, note chưa vào index sẽ bị báo `found:
    /// false` sai.
    pub fn read(&self, paths: &[String], options: Value) -> Result<Vec<Value>, OhsError> {
        if paths.is_empty() {
            return Err(OhsError::Invalid("paths rỗng".into()));
        }
        if paths.iter().any(|p| p.trim().is_empty()) {
            return Err(OhsError::Invalid("paths có đường dẫn rỗng".into()));
        }
        let options = options_object(options)?;
        if let Some(key) = ["id", "action", "paths"]
            .into_iter()
            .find(|k| options.contains_key(*k))
        {
            return Err(OhsError::Invalid(format!(
                "options của read không được có khoá {key}"
            )));
        }
        let mut req = Map::new();
        req.insert("action".into(), "read".into());
        req.insert("paths".into(), paths.into());
        req.extend(options);
        self.request(req, "notes")
    }

    /// Gửi `req` khi OHS `ready` và lấy mảng `field` của phản hồi, đường dẫn đã chuẩn hoá NFC.
    fn request(&self, req: Map<String, Value>, field: &str) -> Result<Vec<Value>, OhsError> {
        let proc = self.shared.running().map_err(OhsError::NotReady)?;
        let state = self.shared.current(&proc);
        if !state.is_ready() {
            return Err(OhsError::NotReady(state));
        }
        let body = match proc.call(req, self.request_timeout) {
            Ok(body) => body,
            Err(CallError::Disconnected) => return Err(OhsError::NotReady(self.state())),
            Err(CallError::Timeout(d)) => return Err(OhsError::Timeout(d)),
            Err(CallError::Remote(m)) => return Err(OhsError::Remote(m)),
        };
        let mut items = match body {
            Value::Object(mut m) => match m.remove(field) {
                Some(Value::Array(a)) => a,
                other => {
                    return Err(OhsError::Protocol(format!(
                        "phản hồi không có mảng {field}: {other:?}"
                    )));
                }
            },
            other => {
                return Err(OhsError::Protocol(format!(
                    "phản hồi không phải object: {other}"
                )));
            }
        };
        normalize_result_paths(&mut items);
        Ok(items)
    }

    /// PID của tiến trình OHS hiện tại, nếu có (để chẩn đoán và kiểm không mồ côi).
    pub fn pid(&self) -> Option<u32> {
        match &*lock(&self.shared.phase) {
            Phase::Starting(Some(p)) | Phase::Running(p) => Some(p.pid()),
            _ => None,
        }
    }

    /// Tắt OHS: đóng stdin, chờ OHS tự thoát tối đa `shutdown_timeout`, quá hạn thì kill. Bỏ [`Ohs`] cũng làm vậy.
    pub fn shutdown(self) {}

    fn stop(&mut self) {
        *lock(&self.shared.phase) = Phase::Stopped;
        if let Some(tx) = self.events.take() {
            let _ = tx.send(Event::Shutdown);
        }
        if let Some(handle) = self.supervisor.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Ohs {
    fn drop(&mut self) {
        self.stop();
    }
}

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Object tuỳ chọn của request: object giữ nguyên, `null` là object rỗng, kiểu khác là lỗi.
fn options_object(options: Value) -> Result<Map<String, Value>, OhsError> {
    match options {
        Value::Object(m) => Ok(m),
        Value::Null => Ok(Map::new()),
        other => Err(OhsError::Invalid(format!(
            "options phải là object, nhận {other}"
        ))),
    }
}

/// Khoá chứa danh sách đường dẫn note trong kết quả `search` và `read` của OHS.
const PATH_LIST_FIELDS: [&str; 5] = [
    "links",
    "backlinks",
    "markdownLinks",
    "markdownBacklinks",
    "suggestions",
];

/// Chuẩn hoá sang NFC, tại chỗ, `path` và các danh sách đường dẫn ([`PATH_LIST_FIELDS`]) của mỗi kết quả; trường
/// khác giữ nguyên.
fn normalize_result_paths(results: &mut [Value]) {
    fn nfc(v: &mut Value) {
        if let Value::String(p) = v
            && !is_nfc(p)
        {
            *p = p.nfc().collect();
        }
    }
    for r in results {
        if let Some(p) = r.get_mut("path") {
            nfc(p);
        }
        for key in PATH_LIST_FIELDS {
            if let Some(Value::Array(list)) = r.get_mut(key) {
                list.iter_mut().for_each(nfc);
            }
        }
    }
}

/// Kết thúc một lần chạy tiến trình.
enum Run {
    /// Nhận lệnh tắt; tiến trình đã đóng.
    Stopped,
    /// Tiến trình chết hoặc không khởi động được. `ran`: thời gian sống sau `ready`.
    Failed { detail: String, ran: Duration },
}

/// Luồng giám sát: khởi động, theo dõi, khởi động lại có giới hạn, và tắt tiến trình OHS. Là nơi duy nhất spawn và
/// đóng tiến trình, nên mỗi lúc có nhiều nhất một tiến trình OHS.
struct Supervisor {
    shared: Arc<Shared>,
    pkg: OhsPackage,
    cfg: OhsConfig,
    tx: Sender<Event>,
    rx: Receiver<Event>,
}

impl Supervisor {
    fn run(self) {
        let mut failures = 0u32;
        let mut generation = 0u64;
        loop {
            generation += 1;
            let (detail, ran) = match self.run_once(generation) {
                Run::Stopped => return,
                Run::Failed { detail, ran } => (detail, ran),
            };
            if ran >= self.cfg.stable_after {
                failures = 0;
            }
            failures += 1;
            if failures > self.cfg.max_restarts {
                self.shared.set(Phase::Broken(format!(
                    "OHS dừng {failures} lần liên tiếp nên pkm không khởi động lại nữa. Lần cuối: {detail}. Sửa \
                     nguyên nhân rồi khởi động lại pkm."
                )));
                self.wait_shutdown();
                return;
            }
            self.shared.set(Phase::Starting(None));
            if self.pause(self.cfg.restart_backoff * failures) {
                return;
            }
        }
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(&self.pkg.node);
        cmd.arg(&self.pkg.cli)
            .args(["serve", "--stdio"])
            .current_dir(&self.pkg.root)
            .env("OBSIDIAN_VAULT_PATH", &self.cfg.vault)
            .env("OHS_DISABLE_UPDATE_CHECK", "1")
            // Như `bin/ohs` của gói: NODE_OPTIONS của máy không được lọt vào Node đi kèm.
            .env_remove("NODE_OPTIONS");
        if let Some(db) = &self.cfg.db {
            cmd.env("OBSIDIAN_DB_PATH", db);
        }
        // Gói tiếng Việt đi kèm: đặt cho mọi lần chạy, để index không bị dựng lại bảng view qua lại giữa các bên
        // gọi. Gói không có thì để nguyên môi trường.
        if let Some(pack) = &self.pkg.pack {
            cmd.env(LANGUAGE_PACK_ENV, pack);
        }
        #[cfg(windows)]
        {
            // `pkm` chạy từ Claude Desktop hay Obsidian (không có console): không mở cửa sổ console cho node.
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd
    }

    fn run_once(&self, generation: u64) -> Run {
        let proc = match Proc::spawn(self.command(), generation, self.tx.clone()) {
            Ok(p) => Arc::new(p),
            Err(e) => {
                return Run::Failed {
                    detail: format!("không chạy được {}: {e}", self.pkg.node.display()),
                    ran: Duration::ZERO,
                };
            }
        };
        self.shared.set(Phase::Starting(Some(Arc::clone(&proc))));

        let deadline = Instant::now() + self.cfg.ready_timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left.min(self.cfg.poll_interval)) {
                Ok(Event::Ready(g)) if g == generation => break,
                Ok(Event::Exited(g)) if g == generation => {
                    let (status, _) = proc.close(EXIT_GRACE);
                    return Run::Failed {
                        detail: format!(
                            "OHS thoát trước khi sẵn sàng ({}); stderr: {}",
                            exit_text(status),
                            proc.stderr_tail(STDERR_IN_ERRORS)
                        ),
                        ran: Duration::ZERO,
                    };
                }
                Ok(Event::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                    proc.close(self.cfg.shutdown_timeout);
                    return Run::Stopped;
                }
                Ok(_) => {}
                // Tiến trình con của OHS có thể giữ stdout mở sau khi OHS chết: kiểm trực tiếp.
                Err(RecvTimeoutError::Timeout) if proc.try_exit().is_some() => {
                    let (status, _) = proc.close(EXIT_GRACE);
                    return Run::Failed {
                        detail: format!(
                            "OHS thoát trước khi sẵn sàng ({}); stderr: {}",
                            exit_text(status),
                            proc.stderr_tail(STDERR_IN_ERRORS)
                        ),
                        ran: Duration::ZERO,
                    };
                }
                Err(RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                Err(RecvTimeoutError::Timeout) => {
                    proc.close(Duration::ZERO);
                    return Run::Failed {
                        detail: format!(
                            "OHS chưa sẵn sàng sau {:?}; stderr: {}",
                            self.cfg.ready_timeout,
                            proc.stderr_tail(STDERR_IN_ERRORS)
                        ),
                        ran: Duration::ZERO,
                    };
                }
            }
        }

        let ready_at = Instant::now();
        self.shared.set(Phase::Running(Arc::clone(&proc)));
        // Hỏi `status` ngay rồi định kỳ: `search` dùng kết quả này, và `eta_s` có số đo từ lần tool hỏi đầu tiên.
        self.shared.observe(&proc);
        loop {
            let died = match self.rx.recv_timeout(self.cfg.poll_interval) {
                Ok(Event::Exited(g)) => g == generation,
                Ok(Event::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                    proc.close(self.cfg.shutdown_timeout);
                    return Run::Stopped;
                }
                Ok(Event::Ready(_)) => false,
                // Tiến trình con của OHS có thể giữ stdout mở sau khi OHS chết: kiểm trực tiếp.
                Err(RecvTimeoutError::Timeout) => {
                    let died = proc.try_exit().is_some();
                    if !died {
                        self.shared.observe(&proc);
                    }
                    died
                }
            };
            if died {
                self.shared.set(Phase::Starting(None));
                let (status, _) = proc.close(EXIT_GRACE);
                return Run::Failed {
                    detail: format!(
                        "OHS thoát ({}); stderr: {}",
                        exit_text(status),
                        proc.stderr_tail(STDERR_IN_ERRORS)
                    ),
                    ran: ready_at.elapsed(),
                };
            }
        }
    }

    /// Chờ `wait`; trả `true` nếu nhận lệnh tắt trong lúc chờ.
    fn pause(&self, wait: Duration) -> bool {
        let deadline = Instant::now() + wait;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(Event::Shutdown) | Err(RecvTimeoutError::Disconnected) => return true,
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => return false,
            }
        }
    }

    fn wait_shutdown(&self) {
        while let Ok(event) = self.rx.recv() {
            if event == Event::Shutdown {
                return;
            }
        }
    }
}

fn exit_text(status: Option<ExitStatus>) -> String {
    status.map_or_else(|| "không rõ mã thoát".to_owned(), |s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn result_paths_are_normalized_to_nfc_and_other_fields_kept() {
        let nfd: String = "Khách hàng/Đà Nẵng.md".nfd().collect();
        let mut results = vec![
            json!({"path": nfd, "score": 0.5}),
            json!({"score": 1}),
            json!({"path": "ascii.md"}),
        ];
        normalize_result_paths(&mut results);
        assert_eq!(results[0]["path"], "Khách hàng/Đà Nẵng.md");
        assert_eq!(results[0]["score"], 0.5);
        assert_eq!(results[1], json!({"score": 1}));
        assert_eq!(results[2]["path"], "ascii.md");
    }

    #[test]
    fn path_lists_of_read_and_search_results_are_normalized_to_nfc() {
        let n = |s: &str| -> String { s.nfd().collect() };
        let mut results = vec![
            json!({"path": n("Đà Nẵng.md"), "found": true, "content": n("Đà"),
                   "links": [n("Huế.md"), "a.md"], "backlinks": [n("Hà Nội.md")],
                   "markdownLinks": [n("Ổn.md")], "markdownBacklinks": [], "urls": [n("https://ví.dụ")]}),
            json!({"path": n("Thiếu.md"), "found": false, "suggestions": [n("Thiếu sót.md")]}),
        ];
        normalize_result_paths(&mut results);
        assert_eq!(
            results[0],
            json!({"path": "Đà Nẵng.md", "found": true, "content": n("Đà"),
                   "links": ["Huế.md", "a.md"], "backlinks": ["Hà Nội.md"],
                   "markdownLinks": ["Ổn.md"], "markdownBacklinks": [], "urls": [n("https://ví.dụ")]})
        );
        assert_eq!(
            results[1],
            json!({"path": "Thiếu.md", "found": false, "suggestions": ["Thiếu sót.md"]})
        );
    }

    #[test]
    fn invalid_read_input_is_rejected_before_anything_is_sent() {
        let ohs = Ohs::unavailable(OhsState::Starting);
        let one = vec!["a.md".to_owned()];
        for (paths, options) in [
            (vec![], Value::Null),
            (vec![" ".to_owned()], Value::Null),
            (one.clone(), json!("x")),
            (one.clone(), json!({"paths": ["b.md"]})),
            (one.clone(), json!({"action": "search"})),
            (one.clone(), json!({"id": "r1"})),
        ] {
            assert!(
                matches!(ohs.read(&paths, options.clone()), Err(OhsError::Invalid(_))),
                "{paths:?} {options}"
            );
        }
        let err = ohs.read(&one, json!({"related": false})).unwrap_err();
        assert_eq!(err.not_ready_error().unwrap()["state"], "broken");
    }

    #[test]
    fn unavailable_ohs_reports_broken_and_refuses_search() {
        let ohs = Ohs::unavailable(OhsState::missing_package(&PackageError("x".into())));
        assert_eq!(ohs.state().name(), "broken");
        assert_eq!(ohs.pid(), None);
        let err = ohs.search("khách hàng", Value::Null).unwrap_err();
        let e = err.not_ready_error().unwrap();
        assert_eq!(
            (&e["error"], &e["state"]),
            (&json!("ohs_not_ready"), &json!("broken"))
        );
    }

    #[test]
    fn invalid_search_input_is_rejected_before_anything_is_sent() {
        let ohs = Ohs::unavailable(OhsState::Starting);
        assert!(matches!(
            ohs.search("  ", Value::Null),
            Err(OhsError::Invalid(_))
        ));
        assert!(matches!(
            ohs.search("x", json!([1])),
            Err(OhsError::Invalid(_))
        ));
    }

    #[test]
    fn launch_without_package_is_broken_with_install_action() {
        let tmp = tempfile::tempdir().unwrap();
        let ohs = Ohs::launch(
            Some(&tmp.path().join("không có")),
            OhsConfig::new(tmp.path()),
        );
        let e = ohs.state().not_ready_error().unwrap();
        assert_eq!(e["state"], "broken");
        assert!(e["action"].as_str().unwrap().contains("--ohs"), "{e}");
    }
}
