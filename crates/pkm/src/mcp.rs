//! `pkm VAULT mcp`: MCP server stdio `pkm` của vault (một cửa: tìm kiếm qua OHS và graph), chép
//! `scripts/kg/mcp_server.py` ở sha ghim; phần khác Python ghi trong `docs/MCP-CHANGELOG.md`.
//!
//! Giao thức (DECISIONS Q4): JSON-RPC 2.0 tự viết, mỗi dòng một bản tin, đồng bộ, không async runtime.
//! `initialize`, `ping`, `tools/list`, `tools/call`; `resources/list`, `prompts/list`, `resources/templates/list`
//! trả rỗng; phương thức lạ `-32601`; dòng không phải JSON `-32700` với `id: null`; batch trả mảng; notification
//! (không `id` hoặc `id: null`) không trả lời. stdout chỉ chứa bản tin; log ra stderr. Hết stdin thì thoát.
//!
//! Index: `VAULT/.kg/graph.db` (hoặc `--db`), dựng ở nền ngay khi server chạy. Trước mỗi lời gọi tool, nếu lần kiểm
//! trước đã quá `--refresh-every` giây thì build tăng dần (`-1`: chỉ dựng lúc khởi động). Tool đi qua
//! [`tool_layer`].

pub mod instructions;
pub mod pyjson;
pub mod tool_layer;

#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::io::{self, BufRead, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use pkm_core::adapter::{self, Adapter};
use pkm_core::index::{self, Build, EdgeKeys, Options};
use pkm_core::ohs::{Ohs, OhsConfig, OhsPackage};
use serde_json::{Map, Value, json};

use tool_layer::{ToolContext, ToolLayer, ToolSession};

/// Phiên bản giao thức nhận được, theo thứ tự ưu tiên; phần tử đầu là bản trả khi client xin bản lạ.
pub const PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
pub const SERVER_NAME: &str = "kg";
pub const SERVER_VERSION: &str = "0.1.0";

/// `initialize` và `tools/list` chờ lần dựng đầu tối đa chừng này (vault lớn trên ổ chậm: không giữ client quá lâu).
const READY_WAIT: Duration = Duration::from_secs(20);

/// `tools/call` chờ lần dựng đầu tối đa chừng này, rồi trả `index unavailable` thay vì treo client (Python chờ không
/// hạn). Test dùng hạn ngắn.
const CALL_WAIT: Duration = if cfg!(test) {
    Duration::from_secs(5)
} else {
    Duration::from_secs(120)
};

/// Câu của một panic (`panic!("...")` mang `&str` hoặc `String`).
fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".to_owned())
}

const ABOUT: &str = "MCP server (stdio) của index graph: bọc tầng tool theo bộ tool của bản Python.

Index: VAULT/.kg/graph.db (hoặc --db). Dựng ở nền ngay khi server chạy; trước mỗi lời gọi tool, nếu lần kiểm trước \
đã quá --refresh-every giây thì build tăng dần. -1 là chỉ dựng lúc khởi động. Gói schema mặc định lấy từ lần build \
trước, nên chỉ cần --packs ở lần đầu. Không cần file cấu hình: mọi thứ nằm trong `args` của client.";

/// Cờ của `pkm VAULT mcp`, như `parser()` của Python.
#[derive(Parser, Debug, Clone)]
#[command(name = "pkm VAULT mcp", about = ABOUT, infer_long_args = true)]
pub struct McpArgs {
    pub vault: PathBuf,
    #[arg(value_parser = ["mcp"], hide = true)]
    pub cmd: String,
    /// Index (mặc định VAULT/.kg/graph.db); vault chỉ đọc thì đặt ra ngoài vault.
    #[arg(long)]
    pub db: Option<PathBuf>,
    /// Gói schema, cách nhau bằng dấu phẩy (mặc định: gói của lần build trước).
    #[arg(long)]
    pub packs: Option<String>,
    /// Giây giữa hai lần build tăng dần trước lời gọi tool (0: mọi lần, -1: chỉ lúc khởi động). Mặc định 5, hoặc 30
    /// nếu vault ở ổ Windows nhìn từ WSL.
    #[arg(long, allow_negative_numbers = true)]
    pub refresh_every: Option<f64>,
    #[arg(long)]
    pub no_aliases: bool,
    /// Mặc định schema, hoặc cách đọc của lần build trước khi không ghi cờ đọc nào.
    #[arg(long, value_parser = ["schema", "links"])]
    pub edge_keys: Option<String>,
    #[arg(long)]
    pub body_edges: bool,
    /// Cạnh riêng của vault: e11 (bản B); mặc định adapter của lần build trước, none để bỏ.
    #[arg(long)]
    pub adapter: Option<String>,
    /// Một câu: vault mô tả điều gì (đưa vào instructions của server).
    #[arg(long)]
    pub about: Option<String>,
    /// Ngày "hôm nay" cho instructions (mặc định ngày máy).
    #[arg(long)]
    pub today: Option<String>,
    /// Thư mục gói OHS (có `node/` và `app/dist/`) cho `search` và `related` gieo bằng câu hỏi hay note; mặc định biến
    /// PKM_OHS_DIR, rồi thư mục gói mà `pkm mcp install` cài.
    #[arg(long)]
    pub ohs: Option<PathBuf>,
    /// Cờ cũ của bản Python (đường dẫn `cli.js` của OHS): nhận để tương thích. Trỏ vào một gói OHS (thư mục gói, hoặc
    /// `<gói>/app/dist/src/cli.js`) thì dùng gói đó khi không có --ohs; không thì bỏ qua.
    #[arg(long, env = "KG_OHS_CLI")]
    pub ohs_cli: Option<String>,
    /// Cờ cũ của bản Python (lệnh node cho OHS): nhận để tương thích, bỏ qua (gói OHS có Node đi kèm).
    #[arg(long, env = "KG_OHS_NODE")]
    pub ohs_node: Option<String>,
    /// Tên vault trong Obsidian cho link obsidian://open (mặc định tên thư mục vault; "" để tắt).
    #[arg(long)]
    pub obsidian_vault: Option<String>,
    #[arg(long, default_value_t = 30000)]
    pub max_chars: usize,
    /// File JSONL ghi mỗi lời gọi tool (tên, tham số, cỡ, ms).
    #[arg(long)]
    pub log: Option<PathBuf>,
}

/// Chạy `pkm VAULT mcp ...` (`argv` không gồm tên chương trình).
pub fn main(argv: Vec<OsString>) -> ExitCode {
    let a = McpArgs::parse_from(std::iter::once(OsString::from("pkm")).chain(argv));
    let ohs = launch_ohs(&a.vault, a.ohs.as_deref(), a.ohs_cli.as_deref());
    let server = Server::start_with(a, tool_layer::installed(), ohs);
    let served = server.serve(io::stdin().lock(), io::stdout().lock());
    // Hết stdin (hoặc lỗi ghi): tắt OHS trước khi thoát, không để lại tiến trình `node`.
    server.shutdown();
    match served {
        Ok(()) => ExitCode::SUCCESS,
        // Client đóng ống đọc trước: thoát lặng, không phải lỗi của server.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[kg-mcp] {e}");
            ExitCode::FAILURE
        }
    }
}

/// Biến môi trường mà `pkm mcp install --no-ohs` ghi vào mục `kg`: `1` là không chạy OHS.
pub const NO_OHS_ENV: &str = "PKM_NO_OHS";

/// Khởi động OHS ở nền (không chặn) cho `related` chỉ có `query`: gói theo `--ohs`, rồi gói mà cờ cũ `--ohs-cli` trỏ
/// tới (nếu là gói OHS), rồi `PKM_OHS_DIR`, rồi thư mục mặc định ([`OhsPackage::locate`]). `PKM_NO_OHS=1`: không chạy
/// (`None`; `related` chỉ có `query` trả `ohs_not_ready`). Không thấy gói thì OHS ở trạng thái `broken` kèm cách cài.
pub fn launch_ohs(vault: &Path, flag: Option<&Path>, legacy_cli: Option<&str>) -> Option<Arc<Ohs>> {
    if std::env::var_os(NO_OHS_ENV).is_some_and(|v| v == "1") {
        return None;
    }
    let legacy = match (flag, legacy_cli) {
        (None, Some(cli)) => {
            let pkg = legacy_package(Path::new(cli));
            if pkg.is_none() {
                eprintln!(
                    "[kg-mcp] bỏ qua --ohs-cli {cli}: không phải gói OHS (dùng --ohs <thư mục gói>)"
                );
            }
            pkg
        }
        _ => None,
    };
    Some(Arc::new(Ohs::launch(
        flag.or(legacy.as_deref()),
        OhsConfig::new(abspath(vault)),
    )))
}

/// Thư mục gói OHS mà giá trị cũ của `--ohs-cli` trỏ tới: chính thư mục gói, hoặc `<gói>/app/dist/src/cli.js`.
fn legacy_package(cli: &Path) -> Option<PathBuf> {
    let root = if cli.is_dir() {
        cli.to_path_buf()
    } else if cli.ends_with(Path::new("app").join("dist").join("src").join("cli.js")) {
        cli.ancestors().nth(4)?.to_path_buf()
    } else {
        return None;
    };
    OhsPackage::from_dir(&root).ok().map(|_| root)
}

/// Phiên bản giao thức trả lời: đúng bản client xin nếu nằm trong [`PROTOCOLS`], không thì bản mới nhất.
pub fn negotiate(requested: Option<&str>) -> &'static str {
    PROTOCOLS
        .iter()
        .copied()
        .find(|p| Some(*p) == requested)
        .unwrap_or(PROTOCOLS[0])
}

/// `os.path.abspath`: tuyệt đối, bỏ `.` và `..` theo chữ.
pub fn abspath(p: &Path) -> PathBuf {
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// `platform_paths.windows_drive`: vault trên ổ Windows nhìn từ WSL (`/mnt/<chữ thường>/...`).
fn windows_drive(vault: &Path) -> bool {
    if cfg!(windows) {
        return false;
    }
    let s = vault.to_string_lossy();
    let Some(rest) = s.strip_prefix("/mnt/") else {
        return false;
    };
    let mut chars = rest.chars();
    matches!(chars.next(), Some('a'..='z')) && matches!(chars.next(), None | Some('/'))
}

/// `str(x)` của Python cho một giá trị JSON (tên phương thức, tên tool trong câu báo lỗi).
pub fn py_str(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "None".to_owned(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => if *b { "True" } else { "False" }.to_owned(),
        Some(v) => pyjson::dumps(v, pyjson::LINE),
    }
}

/// Bỏ trường `null` và chuỗi rỗng trong object (ít token hơn cho model); danh sách rỗng giữ lại. Lỗi
/// `ohs_not_ready` giữ nguyên cả năm khoá (D7), kể cả `progress`, `eta_s` là `null`.
pub fn compact(v: Value) -> Value {
    match v {
        Value::Object(m) if m.get("error").and_then(Value::as_str) == Some("ohs_not_ready") => {
            Value::Object(m)
        }
        Value::Object(m) => Value::Object(
            m.into_iter()
                .filter(|(_, x)| !x.is_null() && x.as_str() != Some(""))
                .map(|(k, x)| (k, compact(x)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.into_iter().map(compact).collect()),
        x => x,
    }
}

/// Kết quả là lỗi khi là object có `error` và nhiều nhất hai khoá (lỗi kèm `did_you_mean`, `detail`).
/// `ohs_not_ready` (D7, không có trong Python) cũng là lỗi dù có năm khoá (`state`, `progress`, `eta_s`, `action`).
fn is_error(v: &Value) -> bool {
    v.as_object()
        .is_some_and(|o| o.contains_key("error") && (o.len() <= 2 || o["error"] == "ohs_not_ready"))
}

/// `round(x, 1)` cho số mili giây của log.
fn round1(d: Duration) -> Value {
    Value::from((d.as_secs_f64() * 10_000.0).round() / 10.0)
}

/// Phần được khoá: phiên tool và kết quả lần dựng gần nhất.
#[derive(Default)]
struct State {
    tools: Option<Box<dyn ToolSession>>,
    /// Lần build gần nhất (thành công hoặc lỗi khi refresh); `None`: chưa lần nào.
    last_check: Option<Instant>,
    build_error: Option<String>,
    /// Không mở được phiên tool (binary không có tầng tool, hoặc tầng tool báo lỗi).
    open_error: Option<String>,
    /// Index của phiên tool có trang hệ thống.
    hidden: bool,
}

/// Adapter nạp một lần (như `Server._adapter`): nạp lỗi thì nhớ lỗi, các lần build sau chạy không adapter.
#[derive(Default)]
struct AdapterSlot {
    adapter: Option<Arc<dyn Adapter>>,
    error: Option<String>,
}

struct Shared {
    a: McpArgs,
    vault: PathBuf,
    db: PathBuf,
    opts: Options,
    vault_name: Option<String>,
    refresh_every: f64,
    layer: Option<Arc<dyn ToolLayer>>,
    state: Mutex<State>,
    ready: Mutex<bool>,
    ready_cv: Condvar,
    adapter: Mutex<AdapterSlot>,
    /// OHS của server (một tiến trình cho cả phiên), gắn vào phiên tool; [`Server::shutdown`] lấy ra để tắt.
    ohs: Mutex<Option<Arc<Ohs>>>,
}

/// Một MCP server đang chạy. Lần dựng đầu chạy ở luồng nền từ [`Server::start`].
pub struct Server {
    s: Arc<Shared>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Server {
    /// Khởi động: tính đường dẫn và cách đọc, rồi dựng index ở nền. `layer`: tầng tool ([`tool_layer::installed`]).
    #[cfg(test)]
    pub fn start(a: McpArgs, layer: Option<Arc<dyn ToolLayer>>) -> Self {
        Self::start_with(a, layer, None)
    }

    /// [`Server::start`] với OHS đã khởi động ([`launch_ohs`]) cho `related` chỉ có `query`.
    pub fn start_with(
        a: McpArgs,
        layer: Option<Arc<dyn ToolLayer>>,
        ohs: Option<Arc<Ohs>>,
    ) -> Self {
        let vault = abspath(&a.vault);
        let db =
            a.db.clone()
                .unwrap_or_else(|| vault.join(".kg").join("graph.db"));
        let opts = Options::pick(
            a.no_aliases,
            a.edge_keys.as_deref().and_then(EdgeKeys::parse),
            a.body_edges,
            Some(&db),
        );
        let vault_name = match a.obsidian_vault.as_deref() {
            Some("") => None,
            Some(v) => Some(v.to_owned()),
            None => Some(instructions::basename(&vault)),
        };
        let refresh_every =
            a.refresh_every
                .unwrap_or(if windows_drive(&vault) { 30.0 } else { 5.0 });
        let s = Arc::new(Shared {
            a,
            vault,
            db,
            opts,
            vault_name,
            refresh_every,
            layer,
            state: Mutex::new(State::default()),
            ready: Mutex::new(false),
            ready_cv: Condvar::new(),
            adapter: Mutex::new(AdapterSlot::default()),
            ohs: Mutex::new(ohs),
        });
        // Lời gọi tool đến sớm chờ cờ `ready` của lần dựng này (`wait_ready`).
        let first = Arc::clone(&s);
        thread::spawn(move || first.first_build());
        Server { s }
    }

    /// Tắt khi hết phiên: bỏ phiên tool (chờ lần dựng đang chạy xong), rồi tắt OHS (đóng stdin của OHS, chờ có giới
    /// hạn rồi kill). Gọi sau [`Server::serve`].
    pub fn shutdown(&self) {
        // Lấy OHS ra trước: phiên mở sau lúc này không gắn OHS nữa.
        let ohs = lock(&self.s.ohs).take();
        lock(&self.s.state).tools = None;
        if let Some(ohs) = ohs {
            match Arc::try_unwrap(ohs) {
                Ok(ohs) => ohs.shutdown(),
                // Còn chỗ khác giữ (không xảy ra sau khi đã bỏ phiên): OHS tắt khi chỗ đó bỏ.
                Err(_) => self.s.log("OHS còn được giữ, tắt khi bỏ"),
            }
        }
    }

    /// Đọc bản tin tới hết stdin, trả lời từng dòng.
    pub fn serve(&self, mut input: impl BufRead, mut out: impl Write) -> io::Result<()> {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            if input.read_until(b'\n', &mut buf)? == 0 {
                return Ok(());
            }
            let text = String::from_utf8_lossy(&buf);
            let line = text.trim();
            if line.is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(line) {
                Err(_) => Some(pyjson::dumps(
                    &json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "parse error"}}),
                    pyjson::DEFAULT,
                )),
                Ok(Value::Array(batch)) => {
                    let replies: Vec<Value> = batch
                        .iter()
                        .filter_map(Value::as_object)
                        .filter_map(|m| self.handle(m))
                        .collect();
                    (!replies.is_empty())
                        .then(|| pyjson::dumps(&Value::Array(replies), pyjson::LINE))
                }
                Ok(Value::Object(m)) => self.handle(&m).map(|r| pyjson::dumps(&r, pyjson::LINE)),
                Ok(_) => None,
            };
            if let Some(r) = reply {
                out.write_all(r.as_bytes())?;
                out.write_all(b"\n")?;
                out.flush()?;
            }
        }
    }

    /// Trả lời một bản tin; `None` với notification.
    pub fn handle(&self, m: &Map<String, Value>) -> Option<Value> {
        let id = m.get("id").filter(|v| !v.is_null())?.clone();
        let method = m.get("method");
        let params = m.get("params").and_then(Value::as_object);
        let ok = |result: Value| Some(json!({"jsonrpc": "2.0", "id": id, "result": result}));
        match method.and_then(Value::as_str).unwrap_or_default() {
            "initialize" => {
                let want = params
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(Value::as_str);
                ok(json!({
                    "protocolVersion": negotiate(want),
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
                    "instructions": self.instructions(),
                }))
            }
            "ping" => ok(json!({})),
            "tools/list" => ok(json!({"tools": self.defs()})),
            "resources/list" => ok(json!({"resources": []})),
            "prompts/list" => ok(json!({"prompts": []})),
            "resources/templates/list" => ok(json!({"resourceTemplates": []})),
            "tools/call" => ok(self.s.tools_call(params)),
            _ => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": format!("method not found: {}", py_str(method))},
            })),
        }
    }

    /// `instructions` của `initialize`.
    pub fn instructions(&self) -> String {
        self.s.wait_ready(Some(READY_WAIT));
        self.s.instructions()
    }

    /// Mục của `tools/list`.
    pub fn defs(&self) -> Vec<Value> {
        self.s.wait_ready(Some(READY_WAIT));
        let Some(layer) = &self.s.layer else {
            return Vec::new();
        };
        // Sau khi sẵn sàng, chỉ luồng nền hoặc lời gọi tool giữ khoá; đang bận thì coi như chưa có phiên.
        let hidden = self
            .s
            .state
            .try_lock()
            .map(|st| st.tools.is_some() && st.hidden)
            .unwrap_or(false);
        layer.defs(hidden)
    }
}

impl Shared {
    fn log(&self, msg: &str) {
        eprintln!("[kg-mcp] {msg}");
    }

    /// Chờ lần dựng đầu xong (tối đa `timeout`); trả `true` khi đã xong.
    fn wait_ready(&self, timeout: Option<Duration>) -> bool {
        let guard = lock(&self.ready);
        match timeout {
            None => *self
                .ready_cv
                .wait_while(guard, |r| !*r)
                .unwrap_or_else(PoisonError::into_inner),
            Some(t) => {
                *self
                    .ready_cv
                    .wait_timeout_while(guard, t, |r| !*r)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0
            }
        }
    }

    fn set_ready(&self) {
        *lock(&self.ready) = true;
        self.ready_cv.notify_all();
    }

    /// Gói schema: cờ `--packs`, không thì gói của lần build trước.
    fn packs(&self) -> Vec<String> {
        let raw = match &self.a.packs {
            Some(p) => p.clone(),
            None => index::previous(&self.db, "packs", ""),
        };
        raw.split(',')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn adapter(&self) -> Result<Option<Arc<dyn Adapter>>, String> {
        let mut slot = lock(&self.adapter);
        if slot.adapter.is_none() && slot.error.is_none() {
            let spec = match &self.a.adapter {
                Some(s) => s.clone(),
                None => index::previous(&self.db, "adapter_spec", ""),
            };
            match adapter::load(&spec) {
                Ok(a) => slot.adapter = a.map(Arc::from),
                Err(e) => {
                    slot.error = Some(e.to_string());
                    return Err(e.to_string());
                }
            }
        }
        Ok(slot.adapter.clone())
    }

    fn context(&self) -> ToolContext {
        ToolContext {
            db: self.db.clone(),
            vault: self.vault.clone(),
            vault_name: self.vault_name.clone(),
            limit: None,
            ohs: lock(&self.ohs).clone(),
            max_chars: Some(self.a.max_chars),
        }
    }

    /// Mở phiên tool, hoặc mở lại graph của phiên đang có.
    fn open_tools(&self, st: &mut State) -> Result<(), String> {
        let Some(layer) = &self.layer else {
            st.open_error = Some("this pkm build has no tool layer".to_owned());
            return Ok(());
        };
        match st.tools.as_mut() {
            Some(t) => t.reopen()?,
            None => st.tools = Some(layer.open(&self.context())?),
        }
        st.open_error = None;
        st.hidden = index::connect(&self.db, true)
            .map(|c| !instructions::hidden(&c).is_empty())
            .unwrap_or(false);
        Ok(())
    }

    fn build(&self, st: &mut State) -> Result<(), String> {
        let t = Instant::now();
        let adapter = self.adapter()?;
        let info = index::build(
            &self.vault,
            &self.db,
            &Build {
                packs: self.packs(),
                opts: self.opts,
                adapter: adapter.as_deref(),
                ..Build::default()
            },
        )
        .map_err(|e| e.to_string())?;
        st.last_check = Some(Instant::now());
        let get = |k| info.get(k).unwrap_or_default();
        let changed =
            get("mode") == "full" || get("notes_read") != "0" || get("notes_removed") != "0";
        if changed || st.tools.is_none() {
            self.open_tools(st)?;
        }
        self.log(&format!(
            "build {}: {} note, đọc {}, {} ms",
            get("mode"),
            get("notes"),
            get("notes_read"),
            t.elapsed().as_millis()
        ));
        Ok(())
    }

    /// Lần dựng đầu. Luôn đặt cờ `ready`, kể cả khi dựng panic (lỗi ghi vào `build_error`): lời gọi tool không chờ
    /// một luồng đã chết.
    fn first_build(&self) {
        let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut st = lock(&self.state);
            if let Err(e) = self.build(&mut st) {
                self.log(&format!("build lỗi: {e}"));
                st.build_error = Some(e);
                // Dùng index cũ nếu có.
                if self.db.exists() {
                    let _ = self.open_tools(&mut st);
                }
            }
        }));
        if let Err(p) = built {
            let e = format!("panic: {}", panic_message(p.as_ref()));
            self.log(&format!("build lỗi: {e}"));
            lock(&self.state).build_error = Some(e);
        }
        self.set_ready();
    }

    /// Build tăng dần khi lần kiểm trước đã quá `--refresh-every` giây; lỗi thì dùng index hiện có.
    fn refresh(&self, st: &mut State) {
        let every = self.refresh_every;
        if every < 0.0
            || st
                .last_check
                .is_some_and(|t| t.elapsed().as_secs_f64() < every)
        {
            return;
        }
        if let Err(e) = self.build(st) {
            st.last_check = Some(Instant::now());
            self.log(&format!("build tăng dần lỗi, dùng index cũ: {e}"));
        }
    }

    fn instructions(&self) -> String {
        let build_error = self
            .state
            .try_lock()
            .ok()
            .and_then(|st| st.build_error.clone());
        if !self.db.exists() {
            return instructions::pending(build_error.as_deref());
        }
        let adapter = lock(&self.adapter)
            .adapter
            .as_ref()
            .and_then(|a| a.instructions().map(str::to_owned));
        let today = match self.a.today.as_deref() {
            Some(t) if !t.is_empty() => t.to_owned(),
            _ => chrono::Local::now().date_naive().to_string(),
        };
        let packs = self.packs();
        let inputs = instructions::Inputs {
            vault: &self.vault,
            packs: &packs,
            about: self.a.about.as_deref(),
            today: &today,
            vault_name: self.vault_name.as_deref(),
            adapter: adapter.as_deref(),
        };
        let composed = index::connect(&self.db, true)
            .map_err(|e| e.to_string())
            .and_then(|c| instructions::compose(&inputs, &c).map_err(|e| e.to_string()));
        composed.unwrap_or_else(|e| instructions::pending(Some(&e)))
    }

    /// `tools/call`: refresh, gọi tool, thu gọn JSON, cắt theo `--max-chars`, ghi log.
    fn tools_call(&self, params: Option<&Map<String, Value>>) -> Value {
        let raw_name = params.and_then(|p| p.get("name"));
        let raw_args = params.and_then(|p| p.get("arguments"));
        let name = py_str(raw_name);
        let args = raw_args
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let t0 = Instant::now();
        let ready = self.wait_ready(Some(CALL_WAIT));
        let (text, err, t1) = {
            let (res, t1) = if !ready {
                let why = format!(
                    "the index is still being built (waited {} s); try again shortly",
                    CALL_WAIT.as_secs()
                );
                (
                    Ok(json!({"error": format!("index unavailable: {why}")})),
                    Instant::now(),
                )
            } else {
                let mut st = lock(&self.state);
                // Panic trong refresh hay trong tool thành kết quả lỗi; server chạy tiếp.
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.refresh(&mut st);
                    let t1 = Instant::now();
                    let res = match st.tools.as_mut() {
                        Some(t) => t.call(&name, args),
                        None => {
                            let why = st
                                .build_error
                                .clone()
                                .or_else(|| st.open_error.clone())
                                .unwrap_or_else(|| "None".to_owned());
                            Ok(json!({"error": format!("index unavailable: {why}")}))
                        }
                    };
                    (res, t1)
                }));
                caught.unwrap_or_else(|p| {
                    (
                        Err(format!("internal error: {}", panic_message(p.as_ref()))),
                        Instant::now(),
                    )
                })
            };
            match res {
                Ok(v) => {
                    let err = is_error(&v);
                    (pyjson::dumps(&compact(v), pyjson::COMPACT), err, t1)
                }
                Err(e) => {
                    self.log(&format!("tool {name} lỗi: {e}"));
                    (format!("error: {e}"), true, t1)
                }
            }
        };
        let n = text.chars().count();
        let max = self.a.max_chars;
        let text = if n > max {
            let cut: String = text.chars().take(max).collect();
            format!(
                "{cut}... [truncated at {max} of {n} chars; narrow the query or use limit/more]"
            )
        } else {
            text
        };
        if let Some(path) = &self.a.log {
            let rec = json!({
                "t": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                "tool": raw_name.cloned().unwrap_or(Value::Null),
                "args": raw_args.cloned().unwrap_or(Value::Null),
                "chars": n,
                "error": if err { Value::Bool(true) } else { Value::Null },
                "refresh_ms": round1(t1 - t0),
                "ms": round1(t1.elapsed()),
            });
            let line = pyjson::dumps(&rec, pyjson::LINE) + "\n";
            let written = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut f| f.write_all(line.as_bytes()));
            if let Err(e) = written {
                self.log(&format!("không ghi được log {}: {e}", path.display()));
            }
        }
        json!({"content": [{"type": "text", "text": text}], "isError": err})
    }
}
