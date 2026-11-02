//! `pkm doctor`: chẩn đoán máy người dùng (PRD mục 10, "Thêm"): cấu hình Claude Desktop và mục `kg`, file index,
//! gói OHS và trạng thái OHS (D7), đường dẫn dài trên Windows. Mỗi mục không ổn có câu "việc cần làm".
//!
//! Lệnh con được đăng ký ở `cli`: gọi [`run`] với [`DoctorArgs`]; mã thoát khác 0 khi [`Report::healthy`] là `false`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use pkm_core::ohs::{OhsPackage, OhsState};
use serde_json::{Value, json};

use crate::install::config::{self, ConfigDoc, SERVER_NAME, ServerEntry};
use crate::install::{self, Hooks};

/// Tuỳ chọn của `pkm doctor`.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct DoctorArgs {
    /// File cấu hình Claude Desktop; mặc định tự tìm.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// File index cần kiểm; mặc định lấy từ `--db` của mục `kg`.
    #[arg(long, value_name = "PATH")]
    pub db: Option<PathBuf>,
    /// Thư mục gói OHS; mặc định như `pkm` tìm (biến PKM_OHS_DIR, rồi thư mục của bộ cài).
    #[arg(long, value_name = "DIR")]
    pub ohs: Option<PathBuf>,
    /// Không chạy thử OHS (chỉ kiểm gói có đủ file).
    #[arg(long)]
    pub no_ohs_start: bool,
    /// Chờ OHS sẵn sàng tối đa bao nhiêu giây khi chạy thử.
    #[arg(long, default_value_t = 15, value_name = "SECS")]
    pub ohs_wait: u64,
    /// In kết quả dạng JSON.
    #[arg(long)]
    pub json: bool,
}

/// Kết quả một mục kiểm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    /// Chạy được nhưng nên xử lý (hoặc đang chờ, ví dụ OHS đang index).
    Warn,
    /// Hỏng: `kg` hoặc tool cần OHS sẽ không chạy đúng.
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

/// Một mục kiểm: mã ổn định (cho `--json`), kết quả, mô tả, và việc cần làm khi không ổn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub id: &'static str,
    pub status: Status,
    pub detail: String,
    pub action: Option<String>,
}

impl Check {
    fn ok(id: &'static str, detail: impl Into<String>) -> Self {
        Self {
            id,
            status: Status::Ok,
            detail: detail.into(),
            action: None,
        }
    }

    fn skip(id: &'static str, detail: impl Into<String>) -> Self {
        Self {
            id,
            status: Status::Skip,
            detail: detail.into(),
            action: None,
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
        }
    }
}

/// Mọi mục kiểm, theo thứ tự.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    /// Không có mục nào `fail`.
    pub fn healthy(&self) -> bool {
        self.checks.iter().all(|c| c.status != Status::Fail)
    }

    fn count(&self, status: Status) -> usize {
        self.checks.iter().filter(|c| c.status == status).count()
    }

    /// `{"healthy": bool, "checks": [{"id", "status", "detail", "action"}]}`; `action` là `null` khi ổn.
    pub fn to_json(&self) -> Value {
        json!({
            "healthy": self.healthy(),
            "checks": self.checks.iter().map(|c| json!({
                "id": c.id,
                "status": c.status.name(),
                "detail": c.detail,
                "action": c.action,
            })).collect::<Vec<_>>(),
        })
    }

    /// Bản cho người đọc: mỗi mục một dòng, việc cần làm thụt vào dòng dưới, dòng tổng kết ở cuối.
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
        }
        out.push_str(&format!(
            "{} lỗi, {} cảnh báo\n",
            self.count(Status::Fail),
            self.count(Status::Warn)
        ));
        out
    }
}

/// Chạy `pkm doctor`, in kết quả; trả [`Report::healthy`].
pub fn run(args: &DoctorArgs, hooks: &dyn Hooks, out: &mut dyn Write) -> Result<bool, String> {
    let report = diagnose(args, hooks);
    let text = if args.json {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&report.to_json()).expect("Value luôn chuyển được")
        )
    } else {
        report.to_text()
    };
    out.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(report.healthy())
}

/// Chạy mọi mục kiểm.
pub fn diagnose(args: &DoctorArgs, hooks: &dyn Hooks) -> Report {
    let mut checks = Vec::new();
    let entry = check_config(args.config.as_deref(), &mut checks);
    let (vault, entry_db) = entry
        .as_ref()
        .map(|e| install::entry_paths(&e.args))
        .unwrap_or_default();
    checks.push(check_db(args.db.clone().or(entry_db), hooks));
    let pkg = check_ohs_package(args.ohs.as_deref(), &mut checks);
    checks.push(check_ohs_state(pkg, vault.as_deref(), args, hooks));
    checks.push(check_long_paths(hooks));
    Report { checks }
}

/// `claude_config` và `kg_entry`; trả mục `kg` khi đọc được.
fn check_config(explicit: Option<&Path>, checks: &mut Vec<Check>) -> Option<ServerEntry> {
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
            return None;
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
        return None;
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
            return None;
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
    let Some(value) = doc.server(SERVER_NAME) else {
        checks.push(Check::bad(
            ENTRY,
            Status::Fail,
            format!("chưa có mục `{SERVER_NAME}`"),
            reinstall,
        ));
        return None;
    };
    let entry = match ServerEntry::from_value(value) {
        Ok(e) => e,
        Err(e) => {
            checks.push(Check::bad(
                ENTRY,
                Status::Fail,
                format!("mục `{SERVER_NAME}` sai: {e}"),
                reinstall,
            ));
            return None;
        }
    };
    checks.push(match entry_problem(&entry) {
        Some(problem) => Check::bad(ENTRY, Status::Fail, problem, reinstall),
        None => Check::ok(ENTRY, format!("{} {}", entry.command, entry.args.join(" "))),
    });
    Some(entry)
}

/// Lỗi của mục `kg` khiến Claude Desktop không chạy được server, nếu có.
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

fn check_db(db: Option<PathBuf>, hooks: &dyn Hooks) -> Check {
    const ID: &str = "db";
    let Some(db) = db else {
        return Check::skip(
            ID,
            "không biết file index nào (mục `kg` không có --db; dùng --db)",
        );
    };
    let action =
        "Chạy `pkm mcp install VAULT` (dựng index lần đầu), hoặc `pkm VAULT build --db <file>`.";
    if !db.is_file() {
        return Check::bad(
            ID,
            Status::Fail,
            format!("{} chưa có", db.display()),
            action,
        );
    }
    match hooks.check_db(&db) {
        Ok(detail) => Check::ok(ID, detail),
        Err(e) => Check::bad(ID, Status::Fail, e, action),
    }
}

fn check_ohs_package(flag: Option<&Path>, checks: &mut Vec<Check>) -> Option<OhsPackage> {
    const ID: &str = "ohs_package";
    let found = match flag {
        Some(dir) => OhsPackage::from_dir(dir),
        None => OhsPackage::locate(None),
    };
    match found {
        Ok(pkg) => {
            checks.push(Check::ok(ID, pkg.root.display().to_string()));
            Some(pkg)
        }
        Err(e) => {
            let action = OhsState::missing_package(&e).action();
            checks.push(Check::bad(ID, Status::Fail, e.to_string(), action));
            None
        }
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
    if args.no_ohs_start {
        return Check::skip(ID, "không chạy thử OHS (--no-ohs-start)");
    }
    let Some(vault) = vault.filter(|v| v.is_dir()) else {
        return Check::skip(ID, "không biết vault nào (mục `kg` chưa đúng)");
    };
    let state = hooks.ohs_state(pkg, vault, Duration::from_secs(args.ohs_wait));
    let detail = format!("OHS trên {}: {}", vault.display(), state.name());
    match state {
        OhsState::Ready => Check::ok(ID, detail),
        OhsState::Starting | OhsState::Indexing { .. } => {
            Check::bad(ID, Status::Warn, detail, state.action())
        }
        OhsState::ModelMissing { .. } | OhsState::Broken { .. } => {
            Check::bad(ID, Status::Fail, detail, state.action())
        }
    }
}

fn check_long_paths(hooks: &dyn Hooks) -> Check {
    const ID: &str = "long_paths";
    let action = "Bật đường dẫn dài: PowerShell (Run as administrator) chạy `New-ItemProperty -Path \
                  'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\FileSystem' -Name LongPathsEnabled -Value 1 \
                  -PropertyType DWORD -Force`, rồi đăng xuất và đăng nhập lại.";
    match hooks.long_paths_enabled() {
        None => Check::skip(ID, "không phải Windows"),
        Some(Ok(true)) => Check::ok(ID, "LongPathsEnabled = 1"),
        Some(Ok(false)) => Check::bad(
            ID,
            Status::Warn,
            "LongPathsEnabled tắt: note có đường dẫn dài hơn 260 ký tự có thể không đọc được",
            action,
        ),
        Some(Err(e)) => Check::bad(
            ID,
            Status::Warn,
            format!("không đọc được LongPathsEnabled: {e}"),
            action,
        ),
    }
}

#[cfg(test)]
mod tests;
