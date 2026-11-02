//! `pkm doctor`: chẩn đoán máy người dùng (PRD mục 10, "Thêm"): cấu hình Claude Desktop và mục `pkm` (mã kiểm vẫn là
//! `kg_entry` để giữ hợp đồng `--json`), MCP `ohs` riêng, file index, gói OHS và trạng thái OHS (D7), đường dẫn dài
//! trên Windows. Mỗi mục không ổn có câu "việc cần làm" (`action`) và,
//! khi có, một lệnh chạy được không cần hỏi gì (`fix`).
//!
//! Chỉ đọc: không ghi file nào, không dựng index, không chạy OHS. Chạy thử OHS (có thể bắt đầu index và nạp model) chỉ
//! khi có `--deep`. OHS là tuỳ chọn ở giai đoạn so khớp: mọi mục OHS không ổn chỉ là `warn`.
//!
//! Lệnh con được đăng ký ở `cli`: gọi [`run`] với [`DoctorArgs`]; mã thoát khác 0 khi [`Report::ok`] là `false`.
//! Hợp đồng `--json` (khoá ổn định qua các bản): xem [`Report::to_json`].

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use pkm_core::ohs::{OhsPackage, OhsState};
use serde_json::{Value, json};

use crate::install::config::{
    self, ConfigDoc, LEGACY_SERVER_NAME, OHS_SERVER_NAME, SERVER_NAME, ServerEntry,
};
use crate::install::{self, Hooks, shell};

/// Phiên bản của envelope `--json`.
pub const SCHEMA: u32 = 1;

/// Không có OHS thì chỉ `search` và `related` không có `hits` bị ảnh hưởng.
const WITHOUT_OHS: &str = "không có OHS thì chỉ `search` và `related` không có `hits` bị ảnh hưởng; mọi tool khác \
                           của `pkm` (gồm `read`) chạy bình thường";

/// Tuỳ chọn của `pkm doctor`.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct DoctorArgs {
    /// File cấu hình Claude Desktop; mặc định tự tìm.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// File index cần kiểm; mặc định lấy từ `--db` của mục `pkm`.
    #[arg(long, value_name = "PATH")]
    pub db: Option<PathBuf>,
    /// Thư mục gói OHS; mặc định như `pkm` tìm (biến PKM_OHS_DIR, rồi thư mục của bộ cài).
    #[arg(long, value_name = "DIR")]
    pub ohs: Option<PathBuf>,
    /// Chạy thử OHS trên vault của mục `pkm` để xem trạng thái (OHS có thể bắt đầu index và nạp model). Mặc định chỉ
    /// kiểm gói có đủ file và `node --version`, không ghi gì.
    #[arg(long)]
    pub deep: bool,
    /// Chờ OHS sẵn sàng tối đa bao nhiêu giây khi chạy thử (--deep).
    #[arg(long, default_value_t = 15, value_name = "SECS")]
    pub ohs_wait: u64,
    /// In kết quả dạng JSON: {schema, ok, data: {checks: [{id, status, detail, action, fix}]}, error, next}.
    #[arg(long)]
    pub json: bool,
}

/// Kết quả một mục kiểm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    /// Chạy được nhưng nên xử lý (hoặc đang chờ, ví dụ OHS đang index).
    Warn,
    /// Hỏng: `pkm` sẽ không chạy đúng.
    Fail,
    /// Không áp dụng hoặc không kiểm được vì mục trước đã hỏng.
    Skip,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
            Self::Skip => "skip",
        }
    }
}

/// Một mục kiểm: mã ổn định (cho `--json`), kết quả, mô tả, việc cần làm khi không ổn, và lệnh sửa chạy được.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub id: &'static str,
    pub status: Status,
    pub detail: String,
    /// Câu giải thích việc cần làm (cho người đọc).
    pub action: Option<String>,
    /// Lệnh sửa cho shell mặc định của máy (PowerShell trên Windows, `sh` nơi khác), không hỏi gì, chạy lại được.
    pub fix: Option<String>,
}

impl Check {
    fn ok(id: &'static str, detail: impl Into<String>) -> Self {
        Self {
            id,
            status: Status::Ok,
            detail: detail.into(),
            action: None,
            fix: None,
        }
    }

    fn skip(id: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status: Status::Skip,
            ..Self::ok(id, detail)
        }
    }

    fn bad(
        id: &'static str,
        status: Status,
        detail: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            id,
            status,
            detail: detail.into(),
            action: Some(action.into()),
            fix: None,
        }
    }

    fn with_fix(mut self, fix: Option<String>) -> Self {
        self.fix = fix;
        self
    }
}

/// Mọi mục kiểm, theo thứ tự.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    /// Không có mục nào `fail`.
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|c| c.status != Status::Fail)
    }

    fn count(&self, status: Status) -> usize {
        self.checks.iter().filter(|c| c.status == status).count()
    }

    fn failed(&self) -> impl Iterator<Item = &Check> {
        self.checks.iter().filter(|c| c.status == Status::Fail)
    }

    /// Lệnh sửa của các mục `fail`, theo thứ tự, bỏ trùng.
    pub fn next(&self) -> Vec<String> {
        let mut next: Vec<String> = Vec::new();
        for fix in self.failed().filter_map(|c| c.fix.clone()) {
            if !next.contains(&fix) {
                next.push(fix);
            }
        }
        next
    }

    /// Envelope chung của `--json`, khoá ổn định qua các bản:
    ///
    /// `{"schema": 1, "ok": bool, "data": {"checks": [{"id", "status", "detail", "action", "fix"}]},
    ///   "error": null | {"code": "checks_failed", "why", "fix"}, "next": [lệnh, …]}`
    ///
    /// `status` thuộc `ok`, `warn`, `fail`, `skip`; `action`, `fix` là `null` khi không có. `next` là các `fix` của
    /// mục `fail` theo thứ tự, bỏ trùng (rỗng khi `ok`); `error.fix` là lệnh đầu của `next` hoặc `null`.
    pub fn to_json(&self) -> Value {
        let next = self.next();
        let error = (!self.ok()).then(|| {
            let ids: Vec<&str> = self.failed().map(|c| c.id).collect();
            json!({
                "code": "checks_failed",
                "why": format!("{} mục lỗi: {}", ids.len(), ids.join(", ")),
                "fix": next.first(),
            })
        });
        json!({
            "schema": SCHEMA,
            "ok": self.ok(),
            "data": {
                "checks": self.checks.iter().map(|c| json!({
                    "id": c.id,
                    "status": c.status.name(),
                    "detail": c.detail,
                    "action": c.action,
                    "fix": c.fix,
                })).collect::<Vec<_>>(),
            },
            "error": error,
            "next": next,
        })
    }

    /// Bản cho người đọc: mỗi mục một dòng, việc cần làm và lệnh sửa thụt vào dòng dưới, dòng tổng kết ở cuối.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for c in &self.checks {
            let tag = match c.status {
                Status::Ok => "[ok]  ",
                Status::Warn => "[warn]",
                Status::Fail => "[FAIL]",
                Status::Skip => "[skip]",
            };
            out.push_str(&format!("{tag} {:<14} {}\n", c.id, c.detail));
            if let Some(action) = &c.action {
                out.push_str(&format!("       việc cần làm: {action}\n"));
            }
            if let Some(fix) = &c.fix {
                out.push_str(&format!("       lệnh: {fix}\n"));
            }
        }
        out.push_str(&format!(
            "{} lỗi, {} cảnh báo\n",
            self.count(Status::Fail),
            self.count(Status::Warn)
        ));
        out
    }
}

/// Chạy `pkm doctor`, in kết quả; trả [`Report::ok`].
pub fn run(args: &DoctorArgs, hooks: &dyn Hooks, out: &mut dyn Write) -> Result<bool, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("không xác định được đường dẫn của pkm: {e}"))?;
    run_with(args, hooks, &exe, out)
}

/// [`run`] với binary `pkm` truyền vào (dùng trong lệnh sửa khi mục `pkm` không chỉ tới binary nào dùng được).
pub fn run_with(
    args: &DoctorArgs,
    hooks: &dyn Hooks,
    exe: &Path,
    out: &mut dyn Write,
) -> Result<bool, String> {
    let report = diagnose(args, hooks, exe);
    let text = if args.json {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&report.to_json()).expect("Value luôn chuyển được")
        )
    } else {
        report.to_text()
    };
    out.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(report.ok())
}

/// Lệnh sửa dựng từ mục `pkm`: chạy bằng binary của mục đó nếu còn, không thì bằng `pkm` đang chạy.
struct Fixes {
    exe: String,
    /// `args` của mục `pkm` khi vault trong đó còn và `args` đúng dạng [`install::server_entry`] ghi.
    entry_args: Option<Vec<String>>,
    no_ohs: bool,
}

impl Fixes {
    fn new(entry: Option<&ServerEntry>, exe: &Path) -> Self {
        let usable = entry
            .map(|e| Path::new(&e.command))
            .filter(|c| c.is_absolute() && c.is_file());
        let entry_args = entry
            .filter(|e| install::entry_paths(&e.args).0.is_some_and(|v| v.is_dir()))
            .filter(|e| install::reinstall_args(&e.args, false).is_some())
            .map(|e| e.args.clone());
        Self {
            exe: usable.unwrap_or(exe).to_string_lossy().into_owned(),
            entry_args,
            no_ohs: entry.is_some_and(installed_without_ohs),
        }
    }

    fn line(&self, args: Vec<String>) -> String {
        shell::command_line(&self.exe, &args)
    }

    /// `pkm mcp install` lại với đúng vault và cờ của mục `pkm` (từ chối khi Claude Desktop đang chạy).
    fn reinstall(&self) -> Option<String> {
        install::reinstall_args(self.entry_args.as_deref()?, self.no_ohs).map(|a| self.line(a))
    }

    /// Dựng lại index của mục `pkm` vào `db`.
    fn rebuild(&self, db: &Path) -> Option<String> {
        let mut args = install::rebuild_args(self.entry_args.as_deref()?)?;
        let db = db.to_string_lossy().into_owned();
        match args.iter().position(|a| a == "--db") {
            Some(i) if i + 1 < args.len() => args[i + 1] = db,
            _ => args.extend(["--db".to_owned(), db]),
        }
        Some(self.line(args))
    }
}

/// Mục `pkm` được cài với `--no-ohs` (`env` có `PKM_NO_OHS=1`).
fn installed_without_ohs(entry: &ServerEntry) -> bool {
    entry
        .env
        .iter()
        .any(|(k, v)| k == install::NO_OHS_ENV && v == "1")
}

/// Chạy mọi mục kiểm. `exe` là binary `pkm` dùng trong lệnh sửa khi mục `pkm` không chỉ tới binary nào dùng được.
pub fn diagnose(args: &DoctorArgs, hooks: &dyn Hooks, exe: &Path) -> Report {
    let mut checks = Vec::new();
    let (entry, ohs_mcp) = check_config(args.config.as_deref(), &mut checks);
    let fixes = Fixes::new(entry.as_ref(), exe);
    if let Some(c) = checks
        .last_mut()
        .filter(|c| matches!(c.status, Status::Fail | Status::Warn))
    {
        c.fix = fixes.reinstall();
    }
    checks.push(ohs_mcp);
    let (vault, entry_db) = entry
        .as_ref()
        .map(|e| install::entry_paths(&e.args))
        .unwrap_or_default();
    checks.push(check_db(args.db.clone().or(entry_db), &fixes, hooks));
    let entry_ohs = entry.as_ref().and_then(|e| install::entry_ohs(&e.args));
    let pkg = check_ohs_package(
        args.ohs.as_deref().or(entry_ohs.as_deref()),
        &fixes,
        &mut checks,
    );
    checks.push(check_ohs_node(pkg.as_ref(), hooks));
    checks.push(check_ohs_state(
        pkg,
        vault.filter(|v| v.is_dir()).as_deref(),
        args,
        hooks,
    ));
    checks.push(check_long_paths(hooks));
    Report { checks }
}

/// `claude_config` và `kg_entry` (mục `pkm`, hoặc mục `kg` cũ do pkm ghi: cảnh báo đổi tên); trả mục đó khi đọc được,
/// cùng mục kiểm `ohs_mcp`.
fn check_config(explicit: Option<&Path>, checks: &mut Vec<Check>) -> (Option<ServerEntry>, Check) {
    let (entry, doc) = check_entry(explicit, checks);
    (entry, check_ohs_mcp(doc.as_ref()))
}

/// `ohs_mcp`: MCP `ohs` riêng còn đăng ký thì chỉ cảnh báo (không cấm dùng song song, không tự xoá).
fn check_ohs_mcp(doc: Option<&ConfigDoc>) -> Check {
    const ID: &str = "ohs_mcp";
    let Some(doc) = doc else {
        return Check::skip(ID, "không đọc được file cấu hình");
    };
    match doc.server(OHS_SERVER_NAME) {
        None => Check::ok(ID, format!("không có MCP `{OHS_SERVER_NAME}` riêng")),
        Some(_) => Check::bad(
            ID,
            Status::Warn,
            format!(
                "MCP `{OHS_SERVER_NAME}` riêng vẫn đăng ký cạnh `{SERVER_NAME}`: agent thấy hai tool search, OHS chạy hai \
                 tiến trình và nạp model hai lần"
            ),
            format!(
                "`{SERVER_NAME}` đã có search và read. Nếu không cần `{OHS_SERVER_NAME}` riêng, thoát hẳn Claude Desktop \
                 rồi xoá mục `{OHS_SERVER_NAME}` trong file cấu hình; giữ cũng được."
            ),
        ),
    }
}

fn check_entry(
    explicit: Option<&Path>,
    checks: &mut Vec<Check>,
) -> (Option<ServerEntry>, Option<ConfigDoc>) {
    const CONFIG: &str = "claude_config";
    const ENTRY: &str = "kg_entry";
    let path = match config::locate(explicit) {
        Ok(p) => p,
        Err(e) => {
            checks.push(Check::bad(
                CONFIG,
                Status::Fail,
                e,
                "Cài và mở Claude Desktop một lần, hoặc chỉ file cấu hình bằng --config <file>.",
            ));
            checks.push(Check::skip(ENTRY, "không có file cấu hình"));
            return (None, None);
        }
    };
    if !path.is_file() {
        checks.push(Check::bad(
            CONFIG,
            Status::Fail,
            format!("{} chưa có", path.display()),
            "Chạy `pkm mcp install VAULT` (bộ cài tạo file).",
        ));
        checks.push(Check::skip(ENTRY, "không có file cấu hình"));
        return (None, None);
    }
    let doc = match ConfigDoc::load(&path) {
        Ok(doc) => doc,
        Err(e) => {
            checks.push(Check::bad(
                CONFIG,
                Status::Fail,
                e,
                "Sửa JSON của file, hoặc chép lại bản sao lưu `.bak-*` cạnh nó (Claude Desktop đóng hẳn).",
            ));
            checks.push(Check::skip(ENTRY, "không đọc được file cấu hình"));
            return (None, None);
        }
    };
    checks.push(Check::ok(
        CONFIG,
        format!(
            "{} (servers: {})",
            path.display(),
            doc.server_names().join(", ")
        ),
    ));
    let reinstall =
        "Thoát hẳn Claude Desktop rồi chạy `pkm mcp install VAULT` bằng binary pkm cần dùng.";
    let legacy = doc
        .server(LEGACY_SERVER_NAME)
        .and_then(|v| ServerEntry::from_value(v).ok())
        .filter(install::written_by_pkm);
    let (name, value) = match (doc.server(SERVER_NAME), legacy) {
        (Some(v), _) => (SERVER_NAME, v.clone()),
        (None, Some(e)) => (LEGACY_SERVER_NAME, e.to_value()),
        (None, None) => {
            checks.push(Check::bad(
                ENTRY,
                Status::Fail,
                format!("chưa có mục `{SERVER_NAME}`"),
                reinstall,
            ));
            return (None, Some(doc));
        }
    };
    let entry = match ServerEntry::from_value(&value) {
        Ok(e) => e,
        Err(e) => {
            checks.push(Check::bad(
                ENTRY,
                Status::Fail,
                format!("mục `{name}` sai: {e}"),
                reinstall,
            ));
            return (None, Some(doc));
        }
    };
    checks.push(match entry_problem(&entry) {
        Some(problem) => Check::bad(ENTRY, Status::Fail, problem, reinstall),
        None if name == LEGACY_SERVER_NAME => Check::bad(
            ENTRY,
            Status::Warn,
            format!(
                "mục `{LEGACY_SERVER_NAME}` cũ do pkm ghi ({} {}); tên MCP nay là `{SERVER_NAME}`",
                entry.command,
                entry.args.join(" ")
            ),
            format!(
                "Thoát hẳn Claude Desktop rồi chạy lại `pkm mcp install VAULT`: mục `{LEGACY_SERVER_NAME}` được đổi tên \
                 thành `{SERVER_NAME}`. Sửa câu nhắc `{LEGACY_SERVER_NAME}` trong Project instructions nếu có."
            ),
        ),
        None => Check::ok(ENTRY, format!("{} {}", entry.command, entry.args.join(" "))),
    });
    (Some(entry), Some(doc))
}

/// Lỗi của mục `pkm` khiến Claude Desktop không chạy được server, nếu có.
fn entry_problem(entry: &ServerEntry) -> Option<String> {
    let command = Path::new(&entry.command);
    if !command.is_absolute() {
        return Some(format!(
            "`command` không phải đường dẫn đủ: {}",
            entry.command
        ));
    }
    if !command.is_file() {
        return Some(format!("`command` không tồn tại: {}", entry.command));
    }
    let (vault, _) = install::entry_paths(&entry.args);
    let Some(vault) = vault else {
        return Some("`args` không bắt đầu bằng đường dẫn vault".to_owned());
    };
    if !vault.is_dir() {
        return Some(format!("vault không tồn tại: {}", vault.display()));
    }
    if entry.args.get(1).map(String::as_str) != Some("mcp") {
        return Some("`args` thiếu lệnh `mcp` sau vault".to_owned());
    }
    None
}

fn check_db(db: Option<PathBuf>, fixes: &Fixes, hooks: &dyn Hooks) -> Check {
    const ID: &str = "db";
    let Some(db) = db else {
        return Check::skip(
            ID,
            "không biết file index nào (mục `pkm` không có --db; dùng --db)",
        );
    };
    let action =
        "Chạy `pkm mcp install VAULT` (dựng index lần đầu), hoặc `pkm VAULT build --db <file>`.";
    let detail = if db.is_file() {
        match hooks.check_db(&db) {
            Ok(detail) => return Check::ok(ID, detail),
            Err(e) => e,
        }
    } else {
        format!("{} chưa có", db.display())
    };
    Check::bad(ID, Status::Fail, detail, action).with_fix(fixes.rebuild(&db))
}

fn check_ohs_package(
    flag: Option<&Path>,
    fixes: &Fixes,
    checks: &mut Vec<Check>,
) -> Option<OhsPackage> {
    const ID: &str = "ohs_package";
    let found = match flag {
        Some(dir) => OhsPackage::from_dir(dir),
        None => OhsPackage::locate(None),
    };
    match found {
        Ok(pkg) => {
            let version = pkg.version();
            let mut detail = format!(
                "{} ({}",
                pkg.root.display(),
                version.as_deref().unwrap_or("không rõ phiên bản")
            );
            detail.push_str(match pkg.pack {
                Some(_) => ", có gói tiếng Việt)",
                None => ", không có gói tiếng Việt)",
            });
            let pinned = install::ohs_bundle::VERSION;
            let check = if pkg.pack.is_none() {
                Check::bad(
                    ID,
                    Status::Warn,
                    detail,
                    format!(
                        "Gói OHS không có lang/ohs-lang-vi nên tìm tiếng Việt kém. Cài gói của fork bản {pinned}: thoát \
                         hẳn Claude Desktop rồi chạy lại `pkm mcp install`."
                    ),
                )
            } else if install::ohs_bundle::older_than_pin(version.as_deref()) {
                // `pkm mcp install` chỉ nâng gói ở thư mục mặc định; gói chỉ bằng --ohs hay biến môi trường thì người
                // dùng tự nâng, nên lệnh sửa chỉ gợi ý cho gói mặc định.
                let default = OhsPackage::default_dir().ok();
                let fix = if default.as_deref() == Some(pkg.root.as_path()) {
                    fixes.reinstall()
                } else {
                    None
                };
                Check::bad(
                    ID,
                    Status::Warn,
                    detail,
                    format!(
                        "pkm này đi với gói OHS {pinned}. Gói ở {} thì thoát hẳn Claude Desktop rồi chạy lại `pkm mcp \
                         install` để nâng; gói chỉ bằng --ohs hay {} thì nâng gói đó.",
                        default.map_or_else(|| "thư mục mặc định".to_owned(), |d| d.display().to_string()),
                        pkm_core::ohs::OHS_DIR_ENV
                    ),
                )
                .with_fix(fix)
            } else {
                Check::ok(ID, detail)
            };
            checks.push(check);
            Some(pkg)
        }
        Err(e) if fixes.no_ohs => {
            checks.push(Check::skip(
                ID,
                format!("mục `{SERVER_NAME}` cài với --no-ohs ({e})"),
            ));
            None
        }
        Err(e) => {
            let action = format!(
                "Cài gói OHS: thoát hẳn Claude Desktop rồi chạy lại `pkm mcp install` (tự tải gói OHS {} từ GitHub \
                 Release của fork, không cần đăng nhập; hoặc từ biến {} nếu đặt: URL https của gói, kèm \
                 <URL>.sha256), hoặc thêm --ohs-package <file gói|thư mục|URL> (file tải ở {}). Không cần OHS thì \
                 cài lại với --no-ohs.",
                install::ohs_bundle::VERSION,
                install::ohs_bundle::PACKAGE_ENV,
                install::ohs_bundle::Bundle::release_url()
            );
            checks.push(
                Check::bad(ID, Status::Warn, format!("{e}; {WITHOUT_OHS}"), action)
                    .with_fix(fixes.reinstall()),
            );
            None
        }
    }
}

/// Node của gói OHS chạy được (`node --version`); không chạy OHS.
fn check_ohs_node(pkg: Option<&OhsPackage>, hooks: &dyn Hooks) -> Check {
    const ID: &str = "ohs_node";
    let Some(pkg) = pkg else {
        return Check::skip(ID, "không có gói OHS");
    };
    match hooks.node_version(&pkg.node) {
        Ok(v) => Check::ok(ID, format!("{} {v}", pkg.node.display())),
        Err(e) => Check::bad(
            ID,
            Status::Warn,
            format!("{e}; {WITHOUT_OHS}"),
            format!(
                "Gói OHS hỏng: thoát hẳn Claude Desktop, xoá thư mục {} rồi chạy lại `pkm mcp install`.",
                pkg.root.display()
            ),
        ),
    }
}

fn check_ohs_state(
    pkg: Option<OhsPackage>,
    vault: Option<&Path>,
    args: &DoctorArgs,
    hooks: &dyn Hooks,
) -> Check {
    const ID: &str = "ohs_state";
    let Some(pkg) = pkg else {
        return Check::skip(ID, "không có gói OHS");
    };
    if !args.deep {
        return Check::skip(
            ID,
            "không chạy thử OHS (thêm --deep; OHS có thể bắt đầu index và nạp model)",
        );
    }
    let Some(vault) = vault else {
        return Check::skip(ID, "không biết vault nào (mục `pkm` chưa đúng)");
    };
    let state = hooks.ohs_state(pkg, vault, Duration::from_secs(args.ohs_wait));
    let detail = format!("OHS trên {}: {}", vault.display(), state.name());
    match state {
        OhsState::Ready => Check::ok(ID, detail),
        OhsState::Starting | OhsState::Indexing { .. } => {
            Check::bad(ID, Status::Warn, detail, state.action())
        }
        OhsState::ModelMissing { .. } | OhsState::Broken { .. } => Check::bad(
            ID,
            Status::Warn,
            format!("{detail}; {WITHOUT_OHS}"),
            state.action(),
        ),
    }
}

fn check_long_paths(hooks: &dyn Hooks) -> Check {
    const ID: &str = "long_paths";
    const FIX: &str = "New-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\FileSystem' -Name \
                       LongPathsEnabled -Value 1 -PropertyType DWORD -Force";
    let bad = |detail: String| {
        Check::bad(
            ID,
            Status::Warn,
            detail,
            "Bật đường dẫn dài: chạy lệnh dưới trong PowerShell (Run as administrator), rồi đăng xuất và đăng \
             nhập lại.",
        )
        .with_fix(Some(FIX.to_owned()))
    };
    match hooks.long_paths_enabled() {
        None => Check::skip(ID, "không phải Windows"),
        Some(Ok(true)) => Check::ok(ID, "LongPathsEnabled = 1"),
        Some(Ok(false)) => bad(
            "LongPathsEnabled tắt: note có đường dẫn dài hơn 260 ký tự có thể không đọc được"
                .to_owned(),
        ),
        Some(Err(e)) => bad(format!("không đọc được LongPathsEnabled: {e}")),
    }
}

#[cfg(test)]
mod tests;
