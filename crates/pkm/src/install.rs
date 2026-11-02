//! `pkm mcp install`: cài MCP `kg` trỏ vào binary `pkm` cho Claude Desktop, không cần Python (PRD mục 11).
//!
//! Đặc tả: `scripts/kg/desktop_config.py` ở sha ghim. Khác Python ở chỗ không chép mã nào: `command` là chính binary
//! `pkm` đang chạy, index dựng vào thư mục dữ liệu của `pkm` (ngoài vault), và gói OHS cài vào `<thư mục pkm>/ohs`.
//!
//! Mọi tương tác với hệ thống mà test cần thay (dựng index, bắt tay, hỏi Claude Desktop có chạy không, kiểm DB,
//! trạng thái OHS) đi qua [`Hooks`]; [`SystemHooks`] là bản thật. Lệnh con được đăng ký ở `cli`: gọi [`run`] với
//! [`InstallArgs`].

// Module chạy được ngay nhưng chưa được `cli` gọi tới; bỏ dòng này khi `cli` đăng ký lệnh con.
#![allow(dead_code)]

pub mod config;
pub mod desktop;
pub mod handshake;
pub mod ohs_bundle;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use pkm_core::ohs::{Ohs, OhsConfig, OhsPackage, OhsState};
use pkm_core::platform;
use sha2::{Digest, Sha256};

use config::{ConfigDoc, SERVER_NAME, ServerEntry};
use handshake::Handshake;
use ohs_bundle::{Bundle, Source};

/// Tuỳ chọn của `pkm mcp install`.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct InstallArgs {
    /// Vault cần cài (bắt buộc trừ khi --remove hoặc --check).
    #[arg(required_unless_present_any = ["remove", "check"])]
    pub vault: Option<PathBuf>,
    /// File index; mặc định `<thư mục pkm>/kg/<tên vault>-<hash>.db`, ngoài vault.
    #[arg(long, value_name = "PATH")]
    pub db: Option<PathBuf>,
    /// Gói schema, cách nhau bằng dấu phẩy.
    #[arg(long)]
    pub packs: Option<String>,
    /// Không giải link theo alias.
    #[arg(long)]
    pub no_aliases: bool,
    /// Cách đọc cạnh: schema hoặc links.
    #[arg(long, value_parser = ["schema", "links"])]
    pub edge_keys: Option<String>,
    /// Đọc cạnh trong thân note.
    #[arg(long)]
    pub body_edges: bool,
    /// Adapter của vault (tên ngắn hoặc file).
    #[arg(long)]
    pub adapter: Option<String>,
    /// Câu mô tả vault mà MCP server gửi cho agent.
    #[arg(long)]
    pub about: Option<String>,
    /// File cấu hình Claude Desktop; mặc định tự tìm.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Chạy server `kg` đã cấu hình một lần (bắt tay, lấy danh sách tool), không đổi gì.
    #[arg(long, conflicts_with_all = ["remove", "dry_run"])]
    pub check: bool,
    /// In kế hoạch và thay đổi của cấu hình, không ghi gì, không dựng index, không tải gì.
    #[arg(long)]
    pub dry_run: bool,
    /// Xoá mục `kg` khỏi cấu hình (index và gói OHS vẫn giữ).
    #[arg(long)]
    pub remove: bool,
    /// Ghi kể cả khi Claude Desktop đang chạy (app sẽ ghi đè khi thoát).
    #[arg(long)]
    pub force: bool,
    /// Cài gói OHS từ file zip artifact, file gói hoặc thư mục gói đã giải nén.
    #[arg(long, value_name = "PATH")]
    pub ohs_package: Option<PathBuf>,
    /// Không cài hay kiểm gói OHS.
    #[arg(long, conflicts_with = "ohs_package")]
    pub no_ohs: bool,
    /// Hạn giờ bắt tay với server, giây.
    #[arg(long, default_value_t = 120, value_name = "SECS")]
    pub timeout: u64,
}

impl InstallArgs {
    /// Cờ đọc vault theo thứ tự của Python (`--packs`, `--no-aliases`, `--edge-keys`, `--body-edges`, `--adapter`).
    pub fn read_args(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(p) = &self.packs {
            out.extend(["--packs".to_owned(), p.clone()]);
        }
        if self.no_aliases {
            out.push("--no-aliases".to_owned());
        }
        if let Some(k) = &self.edge_keys {
            out.extend(["--edge-keys".to_owned(), k.clone()]);
        }
        if self.body_edges {
            out.push("--body-edges".to_owned());
        }
        if let Some(a) = &self.adapter {
            out.extend(["--adapter".to_owned(), a.clone()]);
        }
        out
    }
}

/// Tương tác với hệ thống. Mặc định là bản thật; test và `cli` thay từng phần khi cần.
pub trait Hooks {
    /// Dựng (hoặc cập nhật) index `db` của `vault` với cờ đọc `read_args`. Mặc định chạy
    /// `<exe> VAULT build --db DB <read_args>` (in tiến độ ra terminal).
    fn build_index(
        &self,
        exe: &Path,
        vault: &Path,
        db: &Path,
        read_args: &[String],
    ) -> Result<(), String> {
        let status = Command::new(exe)
            .arg(vault)
            .arg("build")
            .arg("--db")
            .arg(db)
            .args(read_args)
            .status()
            .map_err(|e| format!("không chạy được {}: {e}", exe.display()))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("dựng index lỗi ({status})"))
        }
    }

    /// Kiểm file index mở được; trả mô tả ngắn. Mặc định kiểm file có và có header SQLite.
    fn check_db(&self, db: &Path) -> Result<String, String> {
        sqlite_header(db)
    }

    /// Bắt tay với server như Claude Desktop.
    fn handshake(&self, entry: &ServerEntry, timeout: Duration) -> Result<Handshake, String> {
        handshake::handshake(entry, timeout)
    }

    /// Claude Desktop có đang chạy không.
    fn desktop_running(&self) -> io::Result<bool> {
        desktop::desktop_running()
    }

    /// Trạng thái OHS (D7) khi chạy gói `pkg` trên `vault`, chờ tối đa `wait`.
    fn ohs_state(&self, pkg: OhsPackage, vault: &Path, wait: Duration) -> OhsState {
        let ohs = Ohs::start(pkg, OhsConfig::new(vault));
        let state = ohs.wait_ready(wait);
        ohs.shutdown();
        state
    }

    /// Windows có bật đường dẫn dài (`LongPathsEnabled`) không; `None` khi không phải Windows.
    fn long_paths_enabled(&self) -> Option<io::Result<bool>> {
        long_paths_enabled()
    }
}

/// Hooks thật của máy.
pub struct SystemHooks;

impl Hooks for SystemHooks {}

/// Chỗ đặt dữ liệu và binary của một lần cài. [`Install::for_this_machine`] là bản thật.
#[derive(Debug, Clone)]
pub struct Install {
    /// Thư mục của `pkm` (`platform::pkm_dir`): index ở `kg/`, gói OHS ở `ohs/`.
    pub pkm_dir: PathBuf,
    /// Binary `pkm` mà Claude Desktop sẽ chạy.
    pub exe: PathBuf,
    /// Gói OHS ghim cho máy này, nếu fork có dựng.
    pub bundle: Option<&'static Bundle>,
}

impl Install {
    pub fn for_this_machine() -> Result<Self, String> {
        Ok(Self {
            pkm_dir: platform::pkm_dir().map_err(|e| e.to_string())?,
            exe: std::env::current_exe()
                .map_err(|e| format!("không xác định được đường dẫn của pkm: {e}"))?,
            bundle: Bundle::current(),
        })
    }

    /// Thư mục gói OHS (như `OhsPackage::default_dir`).
    pub fn ohs_dir(&self) -> PathBuf {
        self.pkm_dir.join(pkm_core::ohs::DEFAULT_DIR_NAME)
    }

    /// Index mặc định của `vault`: `<thư mục pkm>/kg/<tên vault>-<8 hex sha256 đường dẫn>.db`.
    pub fn default_db(&self, vault: &Path) -> PathBuf {
        let name: String = vault
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "vault".to_owned())
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let hash = Sha256::digest(vault.to_string_lossy().as_bytes());
        let short: String = hash[..4].iter().map(|b| format!("{b:02x}")).collect();
        self.pkm_dir.join("kg").join(format!("{name}-{short}.db"))
    }
}

/// Chạy `pkm mcp install` trên máy này.
pub fn run(args: &InstallArgs, hooks: &dyn Hooks, out: &mut dyn Write) -> Result<(), String> {
    run_with(&Install::for_this_machine()?, args, hooks, out)
}

/// [`run`] với chỗ đặt dữ liệu truyền vào.
pub fn run_with(
    inst: &Install,
    args: &InstallArgs,
    hooks: &dyn Hooks,
    out: &mut dyn Write,
) -> Result<(), String> {
    let path = config::locate(args.config.as_deref())?;
    let timeout = Duration::from_secs(args.timeout);
    if args.check {
        return check(&path, hooks, timeout, out);
    }
    let mut doc = ConfigDoc::load(&path)?;
    let before = doc.server(SERVER_NAME).cloned();
    if args.remove {
        return remove(&path, &mut doc, before, args, hooks, out);
    }
    let vault = args
        .vault
        .as_deref()
        .ok_or("thiếu VAULT (đường dẫn vault cần cài)")?;
    let vault = std::path::absolute(vault).map_err(|e| format!("{}: {e}", vault.display()))?;
    if !vault.is_dir() {
        return Err(format!("{} không phải thư mục vault", vault.display()));
    }
    let db = match &args.db {
        Some(db) => std::path::absolute(db).map_err(|e| format!("{}: {e}", db.display()))?,
        None => inst.default_db(&vault),
    };
    let read_args = args.read_args();
    let entry = server_entry(&inst.exe, &vault, &db, &read_args, args.about.as_deref())?;
    let after = entry.to_value();
    let changed = doc.set_server(SERVER_NAME, Some(after.clone()));
    let p = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());

    if args.dry_run {
        p(out, "[dry-run] không ghi gì. Kế hoạch:".to_owned())?;
        p(
            out,
            format!(
                "- cấu hình Claude Desktop: {}{}",
                path.display(),
                if doc.exists { "" } else { " (tạo mới)" }
            ),
        )?;
        p(out, format!("- {}", ohs_plan(inst, args)))?;
        p(
            out,
            format!(
                "- dựng index {} cho vault {}",
                db.display(),
                vault.display()
            ),
        )?;
        p(
            out,
            format!(
                "- {} mục `{SERVER_NAME}`:",
                if changed { "ghi" } else { "giữ nguyên" }
            ),
        )?;
        write!(
            out,
            "{}",
            config::entry_diff(SERVER_NAME, before.as_ref(), Some(&after))
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    refuse_if_desktop_running(hooks, args.force)?;
    install_ohs(inst, args, out)?;
    if let Some(dir) = db.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("tạo {}: {e}", dir.display()))?;
    }
    p(out, format!("dựng index {} …", db.display()))?;
    hooks
        .build_index(&inst.exe, &vault, &db, &read_args)
        .map_err(|e| format!("{e}; cấu hình Claude Desktop chưa đổi"))?;
    if changed {
        let backup = config::write_atomic(&path, &doc.render())?;
        p(
            out,
            format!(
                "đã ghi `{SERVER_NAME}` vào {}{}; servers: {}",
                path.display(),
                backup
                    .map(|b| format!(" (bản cũ: {})", b.display()))
                    .unwrap_or_default(),
                doc.server_names().join(", ")
            ),
        )?;
    } else {
        p(
            out,
            format!(
                "mục `{SERVER_NAME}` trong {} đã đúng, không ghi lại",
                path.display()
            ),
        )?;
    }
    let hs = hooks
        .handshake(&entry, timeout)
        .map_err(|e| format!("cấu hình đã ghi nhưng server không bắt tay được: {e}"))?;
    p(
        out,
        format!(
            "server trả lời: {} {}, {} tool. Mở lại Claude Desktop để dùng `{SERVER_NAME}`.",
            hs.server_name,
            hs.server_version,
            hs.tools.len()
        ),
    )
}

/// Mục `kg`: `command` là binary `pkm`, `args` là `VAULT mcp --db DB <cờ đọc> [--about TEXT]`.
pub fn server_entry(
    exe: &Path,
    vault: &Path,
    db: &Path,
    read_args: &[String],
    about: Option<&str>,
) -> Result<ServerEntry, String> {
    let text = |p: &Path| {
        p.to_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{} không phải UTF-8, không ghi được vào JSON", p.display()))
    };
    let mut args = vec![text(vault)?, "mcp".to_owned(), "--db".to_owned(), text(db)?];
    args.extend(read_args.iter().cloned());
    if let Some(about) = about {
        args.extend(["--about".to_owned(), about.to_owned()]);
    }
    Ok(ServerEntry {
        command: text(exe)?,
        args,
        env: Vec::new(),
    })
}

/// Vault và DB đọc ngược từ `args` của mục `kg` (dạng [`server_entry`] ghi).
pub fn entry_paths(args: &[String]) -> (Option<PathBuf>, Option<PathBuf>) {
    let vault = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .map(PathBuf::from);
    let db = args
        .iter()
        .position(|a| a == "--db")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    (vault, db)
}

fn check(
    path: &Path,
    hooks: &dyn Hooks,
    timeout: Duration,
    out: &mut dyn Write,
) -> Result<(), String> {
    let doc = ConfigDoc::load(path)?;
    let value = doc.server(SERVER_NAME).ok_or_else(|| {
        format!(
            "chưa có `{SERVER_NAME}` trong {}; chạy `pkm mcp install VAULT`",
            path.display()
        )
    })?;
    let entry = ServerEntry::from_value(value).map_err(|e| format!("mục `{SERVER_NAME}`: {e}"))?;
    let hs = hooks.handshake(&entry, timeout)?;
    writeln!(
        out,
        "{}: {} {}, {} tool: {}",
        entry.command,
        hs.server_name,
        hs.server_version,
        hs.tools.len(),
        hs.tools.join(", ")
    )
    .map_err(|e| e.to_string())
}

fn remove(
    path: &Path,
    doc: &mut ConfigDoc,
    before: Option<serde_json::Value>,
    args: &InstallArgs,
    hooks: &dyn Hooks,
    out: &mut dyn Write,
) -> Result<(), String> {
    let w = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    if before.is_none() {
        return w(
            out,
            format!(
                "không có `{SERVER_NAME}` trong {}; không đổi gì",
                path.display()
            ),
        );
    }
    doc.set_server(SERVER_NAME, None);
    if args.dry_run {
        w(
            out,
            format!(
                "[dry-run] không ghi gì. Sẽ xoá `{SERVER_NAME}` khỏi {}:",
                path.display()
            ),
        )?;
        return write!(
            out,
            "{}",
            config::entry_diff(SERVER_NAME, before.as_ref(), None)
        )
        .map_err(|e| e.to_string());
    }
    refuse_if_desktop_running(hooks, args.force)?;
    let backup = config::write_atomic(path, &doc.render())?;
    w(
        out,
        format!(
            "đã xoá `{SERVER_NAME}` khỏi {}{}; servers: {}",
            path.display(),
            backup
                .map(|b| format!(" (bản cũ: {})", b.display()))
                .unwrap_or_default(),
            doc.server_names().join(", ")
        ),
    )
}

/// Từ chối ghi khi Claude Desktop đang chạy (app ghi đè cấu hình khi thoát), trừ khi `force`.
fn refuse_if_desktop_running(hooks: &dyn Hooks, force: bool) -> Result<(), String> {
    if force {
        return Ok(());
    }
    match hooks.desktop_running() {
        Ok(false) => Ok(()),
        Ok(true) => Err("Claude Desktop đang chạy: app giữ cấu hình trong bộ nhớ và ghi đè khi thoát. Thoát hẳn \
                         Claude Desktop (cả biểu tượng ở khay hệ thống) rồi chạy lại"
            .to_owned()),
        Err(e) => Err(format!(
            "không kiểm được Claude Desktop có đang chạy không ({e}). Thoát hẳn Claude Desktop rồi chạy lại với --force"
        )),
    }
}

/// Mô tả việc OHS sẽ làm (cho `--dry-run`).
fn ohs_plan(inst: &Install, args: &InstallArgs) -> String {
    let dir = inst.ohs_dir();
    if args.no_ohs {
        return "bỏ qua gói OHS (--no-ohs)".to_owned();
    }
    if let Some(p) = &args.ohs_package {
        return format!("cài gói OHS từ {} vào {}", p.display(), dir.display());
    }
    if OhsPackage::from_dir(&dir).is_ok() {
        return format!("gói OHS đã có ở {}", dir.display());
    }
    match inst.bundle {
        Some(b) => format!(
            "tải gói OHS bundle-{} (run {}) bằng gh vào {}",
            b.platform,
            ohs_bundle::RUN_ID,
            dir.display()
        ),
        None => "fork chưa có gói OHS cho nền tảng này; bỏ qua".to_owned(),
    }
}

/// Cài gói OHS theo `args`. Gói chỉ ra bằng `--ohs-package` mà lỗi thì dừng; tự tải lỗi thì chỉ cảnh báo, vì `kg`
/// vẫn chạy được không cần OHS (tool cần OHS trả `ohs_not_ready`).
fn install_ohs(inst: &Install, args: &InstallArgs, out: &mut dyn Write) -> Result<(), String> {
    if args.no_ohs {
        return Ok(());
    }
    let dest = inst.ohs_dir();
    let w = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    if let Some(p) = &args.ohs_package {
        let pkg = ohs_bundle::install(&Source::Path(p.clone()), inst.bundle, &dest, out)?;
        return w(out, format!("đã cài gói OHS vào {}", pkg.root.display()));
    }
    if OhsPackage::from_dir(&dest).is_ok() {
        return w(out, format!("gói OHS đã có ở {}", dest.display()));
    }
    let Some(bundle) = inst.bundle else {
        return w(
            out,
            "cảnh báo: fork chưa có gói OHS cho nền tảng này; tool cần OHS sẽ báo ohs_not_ready"
                .to_owned(),
        );
    };
    match ohs_bundle::install(&Source::Download, Some(bundle), &dest, out) {
        Ok(pkg) => w(out, format!("đã cài gói OHS vào {}", pkg.root.display())),
        Err(e) => w(
            out,
            format!(
                "cảnh báo: chưa cài được gói OHS ({e}). `kg` vẫn chạy; tool cần OHS sẽ báo ohs_not_ready tới khi cài gói"
            ),
        ),
    }
}

/// Kiểm `db` là file SQLite (16 byte đầu là `SQLite format 3\0`).
pub fn sqlite_header(db: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut f =
        std::fs::File::open(db).map_err(|e| format!("không mở được {}: {e}", db.display()))?;
    let mut head = [0u8; 16];
    f.read_exact(&mut head)
        .map_err(|_| format!("{} không phải file SQLite (quá ngắn)", db.display()))?;
    if &head != b"SQLite format 3\0" {
        return Err(format!("{} không phải file SQLite", db.display()));
    }
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    Ok(format!("{} ({} KB)", db.display(), size / 1024))
}

/// `LongPathsEnabled` trong registry; `None` ngoài Windows.
fn long_paths_enabled() -> Option<io::Result<bool>> {
    if !cfg!(windows) {
        return None;
    }
    let out = Command::new("reg")
        .args([
            "query",
            r"HKLM\SYSTEM\CurrentControlSet\Control\FileSystem",
            "/v",
            "LongPathsEnabled",
        ])
        .output();
    Some(out.and_then(|o| {
        if !o.status.success() {
            // Không có giá trị: Windows coi như tắt.
            return Ok(false);
        }
        parse_long_paths(&String::from_utf8_lossy(&o.stdout))
            .ok_or_else(|| io::Error::other("không đọc được kết quả `reg query`"))
    }))
}

/// Đọc `LongPathsEnabled    REG_DWORD    0x1` từ kết quả `reg query`.
pub fn parse_long_paths(text: &str) -> Option<bool> {
    text.lines()
        .find(|l| l.contains("LongPathsEnabled"))
        .and_then(|l| l.split_whitespace().last())
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .map(|v| v == 1)
}

#[cfg(test)]
mod tests;
