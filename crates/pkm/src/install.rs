//! `pkm mcp install`: cài MCP `pkm` trỏ vào binary `pkm` cho Claude Desktop, không cần Python (PRD mục 11).
//!
//! Một cửa (Q2, R14): chỉ ghi mục `pkm`. Mục `kg` cũ do chính `pkm` ghi được đổi thành `pkm` tại chỗ; mục `kg` của
//! Python hay của người dùng giữ nguyên (chạy song song lúc so khớp). Không bao giờ ghi, sửa hay xoá mục `ohs`: `pkm`
//! đã có `search`, `read`, nhưng người dùng muốn giữ MCP `ohs` riêng thì vẫn được. Sau khi ghi, làm ấm index tìm kiếm
//! của OHS (trừ khi `--no-warm` hoặc `--no-ohs`) để lần hỏi đầu trong Claude Desktop không gặp `ohs_not_ready`.
//!
//! Cách đọc cạnh: người dùng không chọn (`--edge-keys`, `--body-edges`, `--schema-edges-only`) mà index dựng theo
//! mặc định có note nhưng không có cạnh nào (vault Obsidian thường chỉ có wikilink trong thân note), bộ cài dựng lại
//! với `--edge-keys links --body-edges` và ghi hai cờ đó vào mục `pkm` (xem [`body_edges_fallback`]).
//!
//! Đặc tả: `scripts/kg/desktop_config.py` ở sha ghim. Khác Python ở chỗ không chép mã nào: `command` là chính binary
//! `pkm` đang chạy, index dựng vào thư mục dữ liệu của `pkm` (ngoài vault), và gói OHS cài vào `<thư mục pkm>/ohs`.
//!
//! Mọi tương tác với hệ thống mà test cần thay (dựng index, bắt tay, hỏi Claude Desktop có chạy không, kiểm DB,
//! trạng thái OHS) đi qua [`Hooks`]; [`SystemHooks`] là bản thật. Lệnh con được đăng ký ở `cli`: gọi [`run`] với
//! [`InstallArgs`].

pub mod config;
pub mod desktop;
pub mod handshake;
pub mod ohs_bundle;
pub mod shell;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use pkm_core::ohs::{Ohs, OhsConfig, OhsPackage, OhsState};
use pkm_core::platform;
use sha2::{Digest, Sha256};

use config::{ConfigDoc, LEGACY_SERVER_NAME, OHS_SERVER_NAME, SERVER_NAME, ServerEntry};
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
    /// Giữ cách đọc cạnh mặc định (chỉ khoá frontmatter theo schema) cả khi graph không có cạnh nào. Không có cờ này
    /// (và không có --edge-keys, --body-edges) thì graph rỗng làm bộ cài dựng lại với --edge-keys links --body-edges.
    #[arg(long, conflicts_with_all = ["edge_keys", "body_edges"])]
    pub schema_edges_only: bool,
    /// Adapter của vault (tên ngắn hoặc file).
    #[arg(long)]
    pub adapter: Option<String>,
    /// Câu mô tả vault mà MCP server gửi cho agent.
    #[arg(long)]
    pub about: Option<String>,
    /// File cấu hình Claude Desktop; mặc định tự tìm.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Chạy server `pkm` đã cấu hình một lần (bắt tay, lấy danh sách tool), không đổi gì.
    #[arg(long, conflicts_with_all = ["remove", "dry_run"])]
    pub check: bool,
    /// In kế hoạch và thay đổi của cấu hình, không ghi gì, không dựng index, không tải gì.
    #[arg(long)]
    pub dry_run: bool,
    /// Xoá mục `pkm` (và mục `kg` cũ do pkm ghi) khỏi cấu hình (index và gói OHS vẫn giữ).
    #[arg(long)]
    pub remove: bool,
    /// Ghi kể cả khi Claude Desktop đang chạy (app sẽ ghi đè khi thoát) hoặc không kiểm được.
    #[arg(long)]
    pub force: bool,
    /// Ghi kể cả khi binary pkm đang chạy không nằm ở chỗ cài chuẩn (Claude Desktop sẽ gọi đúng file đó).
    #[arg(long)]
    pub allow_any_location: bool,
    /// Cài gói OHS từ file gói đã tải (phải khớp sha256 ghim), thư mục gói đã giải nén, hoặc URL https của file gói
    /// (kèm `<URL>.sha256`). Không có cờ này thì tự tải từ PKM_OHS_PACKAGE nếu đặt, không thì gói ghim từ GitHub
    /// Release của fork (không cần đăng nhập).
    #[arg(long, value_name = "PATH|URL")]
    pub ohs_package: Option<PathBuf>,
    /// Không cài hay kiểm gói OHS.
    #[arg(long, conflicts_with = "ohs_package")]
    pub no_ohs: bool,
    /// Không làm ấm index tìm kiếm của OHS sau khi cài (OHS sẽ index lần đầu khi Claude Desktop mở pkm).
    #[arg(long)]
    pub no_warm: bool,
    /// Dùng gói OHS đã cài sẵn ở thư mục này (ghi `--ohs DIR` vào mục `pkm`), không cài gói vào thư mục của pkm. Máy
    /// đã có OHS cho vault (MCP `ohs`, script dự án) thì dùng cờ này để cả máy chỉ có một chỗ cài OHS.
    #[arg(long, value_name = "DIR", conflicts_with_all = ["ohs_package", "no_ohs"])]
    pub ohs: Option<PathBuf>,
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

    /// Claude Desktop có đang chạy không: `Some(mô tả tiến trình, thường là đường dẫn exe)` khi có.
    fn desktop_process(&self) -> io::Result<Option<String>> {
        desktop::desktop_process()
    }

    /// Trạng thái OHS (D7) khi chạy gói `pkg` trên `vault`, chờ tối đa `wait`.
    fn ohs_state(&self, pkg: OhsPackage, vault: &Path, wait: Duration) -> OhsState {
        let ohs = Ohs::start(pkg, OhsConfig::new(vault));
        let state = ohs.wait_ready(wait);
        ohs.shutdown();
        state
    }

    /// Làm ấm index tìm kiếm: chạy gói `pkm` trên `vault` (cùng DB mà MCP `pkm` dùng) tới khi index xong, OHS hỏng,
    /// hoặc hết `timeout`; in tiến độ ra `out` khi đổi. Trả trạng thái cuối.
    fn warm_ohs(
        &self,
        pkg: OhsPackage,
        vault: &Path,
        timeout: Duration,
        out: &mut dyn Write,
    ) -> OhsState {
        let ohs = Ohs::start(pkg, OhsConfig::new(vault));
        let deadline = Instant::now() + timeout;
        let mut last = String::new();
        let state = loop {
            let s = ohs.state();
            if s.is_ready()
                || s.is_terminal()
                || matches!(s, OhsState::ModelMissing { .. })
                || Instant::now() >= deadline
            {
                break s;
            }
            let line = match &s {
                OhsState::Indexing {
                    indexed,
                    pending,
                    eta_s,
                    ..
                } => format!(
                    "  OHS đang index: {indexed}/{} note{}",
                    indexed + pending,
                    eta_s.map_or_else(String::new, |e| format!(", còn khoảng {e} giây"))
                ),
                other => format!("  OHS: {}", other.name()),
            };
            if line != last {
                let _ = writeln!(out, "{line}");
                last = line;
            }
            std::thread::sleep(Duration::from_secs(2));
        };
        ohs.shutdown();
        state
    }

    /// DLL của Visual C++ Redistributable (x64) mà gói `pkg` cần nhưng máy không có; `None` khi không phải Windows.
    fn missing_vc_runtime(&self, pkg: &OhsPackage) -> Option<Vec<String>> {
        cfg!(windows).then(|| pkg.missing_vc_runtime())
    }

    /// Windows có bật đường dẫn dài (`LongPathsEnabled`) không; `None` khi không phải Windows.
    fn long_paths_enabled(&self) -> Option<io::Result<bool>> {
        long_paths_enabled()
    }

    /// Tải `url` (https) vào file `out`, cho gói OHS. Mặc định dùng `curl`.
    fn fetch(&self, url: &str, out: &Path) -> Result<(), String> {
        ohs_bundle::curl(url, out)
    }

    /// `node --version` của Node trong gói OHS (chỉ chạy Node, không chạy OHS).
    fn node_version(&self, node: &Path) -> Result<String, String> {
        let out = Command::new(node)
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| format!("không chạy được {}: {e}", node.display()))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
        } else {
            Err(format!("{} --version lỗi ({})", node.display(), out.status))
        }
    }
}

/// Hooks thật của máy.
pub struct SystemHooks;

impl Hooks for SystemHooks {}

/// Gốc của thư mục cài chuẩn trong [`STANDARD_BIN_DIR`].
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Mỗi nền tảng chỉ dùng một gốc.
enum BinBase {
    /// `platform::pkm_dir()` (`%LOCALAPPDATA%\pkm` trên Windows).
    PkmDir,
    /// Thư mục home của người dùng.
    Home,
}

/// Chỗ bộ cài (`install.ps1`, `install.sh`) đặt binary `pkm`: `%LOCALAPPDATA%\pkm\bin` trên Windows, `~/.local/bin`
/// nơi khác. Bộ cài đổi chỗ mặc định thì sửa ở đây.
#[cfg(windows)]
const STANDARD_BIN_DIR: (BinBase, &str) = (BinBase::PkmDir, "bin");
#[cfg(not(windows))]
const STANDARD_BIN_DIR: (BinBase, &str) = (BinBase::Home, ".local/bin");

/// Biến môi trường của bộ cài khi cài vào thư mục khác chỗ chuẩn.
pub const INSTALL_DIR_ENV: &str = "PKM_INSTALL_DIR";
/// File bộ cài đặt cạnh binary đã cài (dòng đầu `pkm-managed-v1`).
pub const INSTALL_MARKER: &str = ".pkm-managed";
/// Repo phát hành `pkm` (riêng tư: tải qua `gh` đã đăng nhập).
pub const RELEASE_REPO: &str = "vantt/obsidian-pkm";
/// Biến trong `env` của mục `pkm` ghi rằng lần cài dùng `--no-ohs`, để `doctor` không cảnh báo thiếu gói OHS.
pub const NO_OHS_ENV: &str = "PKM_NO_OHS";
/// Làm ấm index OHS khi cài chờ tối đa chừng này (vault vài nghìn note lần đầu mất khoảng 10 phút).
const WARM_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Thư mục cài chuẩn của binary trên máy này (xem [`STANDARD_BIN_DIR`]).
pub fn standard_bin_dir() -> Option<PathBuf> {
    let (base, rel) = STANDARD_BIN_DIR;
    let base = match base {
        BinBase::PkmDir => platform::pkm_dir().ok()?,
        BinBase::Home => std::env::home_dir()?,
    };
    Some(base.join(rel))
}

/// Chỗ đặt dữ liệu và binary của một lần cài. [`Install::for_this_machine`] là bản thật.
#[derive(Debug, Clone)]
pub struct Install {
    /// Thư mục của `pkm` (`platform::pkm_dir`): index ở `kg/`, gói OHS ở `ohs/`.
    pub pkm_dir: PathBuf,
    /// Binary `pkm` mà Claude Desktop sẽ chạy.
    pub exe: PathBuf,
    /// Gói OHS ghim cho máy này, nếu fork có dựng.
    pub bundle: Option<&'static Bundle>,
    /// Thư mục cài được coi là ổn định: [`INSTALL_DIR_ENV`] nếu đặt, rồi [`standard_bin_dir`]. Thư mục đầu là chỗ
    /// gợi ý khi binary nằm chỗ khác.
    pub bin_dirs: Vec<PathBuf>,
    /// Nguồn tự tải gói OHS thay cho gói ghim (biến [`ohs_bundle::PACKAGE_ENV`]).
    pub ohs_source: Option<String>,
}

impl Install {
    pub fn for_this_machine() -> Result<Self, String> {
        let env = |key: &str| std::env::var_os(key).filter(|v| !v.is_empty());
        Ok(Self {
            pkm_dir: platform::pkm_dir().map_err(|e| e.to_string())?,
            exe: std::env::current_exe()
                .map_err(|e| format!("không xác định được đường dẫn của pkm: {e}"))?,
            bundle: Bundle::current(),
            bin_dirs: env(INSTALL_DIR_ENV)
                .map(PathBuf::from)
                .into_iter()
                .chain(standard_bin_dir())
                .collect(),
            ohs_source: env(ohs_bundle::PACKAGE_ENV).map(|v| v.to_string_lossy().into_owned()),
        })
    }

    /// Lời cảnh báo khi binary đang chạy không nằm ở chỗ cài ổn định (thư mục cài chuẩn, [`INSTALL_DIR_ENV`], hay
    /// thư mục có [`INSTALL_MARKER`] của bộ cài): Claude Desktop sẽ gọi đúng file đó, xoá hay dời là `pkm` hỏng.
    /// `rerun` là đối số của `pkm` để chạy lại đúng lệnh này từ chỗ chuẩn.
    pub fn exe_location_warning(&self, rerun: &[String]) -> Option<String> {
        let dir = self.exe.parent()?;
        if dir.join(INSTALL_MARKER).is_file() || self.bin_dirs.iter().any(|d| same_dir(d, dir)) {
            return None;
        }
        let place = if is_temporary(&self.exe) {
            "thư mục tải về hoặc thư mục tạm"
        } else {
            "ngoài chỗ cài chuẩn"
        };
        let mut msg = format!(
            "pkm đang chạy từ {} ({place}); Claude Desktop sẽ gọi đúng file này, xoá hay dời nó là `{SERVER_NAME}` hỏng.",
            self.exe.display()
        );
        if let Some(bin) = self.bin_dirs.first() {
            let exe = bin.join(if cfg!(windows) { "pkm.exe" } else { "pkm" });
            msg.push_str(&format!(
                " Cài pkm vào {} bằng bộ cài: {}\nrồi chạy: {}",
                bin.display(),
                installer_command(),
                shell::command_line(&exe.to_string_lossy(), rerun)
            ));
        }
        Some(msg)
    }

    /// Thư mục gói OHS (như `OhsPackage::default_dir`).
    pub fn ohs_dir(&self) -> PathBuf {
        self.pkm_dir.join(pkm_core::ohs::DEFAULT_DIR_NAME)
    }

    /// Index mặc định của `vault`: `<thư mục pkm>/kg/<tên vault>-<8 hex sha256 của khoá vault>.db`, khoá là đường
    /// dẫn đã chuẩn hoá ([`vault_key`]) để cùng một vault viết khác nhau (dấu phân cách cuối, hoa thường trên Windows
    /// và macOS, NFC/NFD, symlink) dùng chung một index. Index đặt theo cách băm cũ (đường dẫn nguyên văn) mà đã có
    /// thì dùng tiếp, để bản cài cũ không phải dựng lại.
    pub fn default_db(&self, vault: &Path) -> PathBuf {
        let legacy = self.legacy_default_db(vault);
        if legacy.is_file() {
            return legacy;
        }
        let canon = std::fs::canonicalize(vault).unwrap_or_else(|_| vault.to_path_buf());
        let key = vault_key(
            &canon.to_string_lossy(),
            cfg!(any(windows, target_os = "macos")),
        );
        self.db_file(&pkm_core::vault::nfc(&file_name(&canon)), &key)
    }

    /// Tên index theo cách băm cũ: sha256 của đường dẫn nguyên văn.
    fn legacy_default_db(&self, vault: &Path) -> PathBuf {
        self.db_file(&file_name(vault), &vault.to_string_lossy())
    }

    /// `<thư mục pkm>/kg/<name, ký tự lạ thành -><8 hex sha256 của key>.db`.
    fn db_file(&self, name: &str, key: &str) -> PathBuf {
        let name: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let hash = Sha256::digest(key.as_bytes());
        let short: String = hash[..4].iter().map(|b| format!("{b:02x}")).collect();
        self.pkm_dir.join("kg").join(format!("{name}-{short}.db"))
    }
}

/// Khoá của một đường dẫn vault (đã giải symlink) để băm tên index: bỏ tiền tố `\\?\` của Windows, bỏ dấu phân
/// cách cuối (trừ gốc như `/`, `C:\`), chuẩn NFC, và gộp hoa thường khi `fold_case` (hệ thống file không phân biệt
/// hoa thường: Windows, macOS).
fn vault_key(path: &str, fold_case: bool) -> String {
    let path = match path.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => path.strip_prefix(r"\\?\").unwrap_or(path).to_owned(),
    };
    let mut key = pkm_core::vault::nfc(&path);
    while key.len() > 1 && key.ends_with(['/', '\\']) && !is_drive_root(&key) {
        key.pop();
    }
    if fold_case {
        key = pkm_core::vault::nfc(&pkm_core::vault::casefold(&key));
    }
    key
}

/// Tên thư mục vault, `vault` khi không có.
fn file_name(vault: &Path) -> String {
    vault
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "vault".to_owned())
}

/// `C:\` hoặc `C:/`.
fn is_drive_root(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':'
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
    let (mut doc, digest) = load_with_digest(&path)?;
    let legacy = legacy_entry(&doc);
    let before = doc
        .server(SERVER_NAME)
        .or_else(|| legacy.then(|| doc.server(LEGACY_SERVER_NAME)).flatten())
        .cloned();
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
    let mut ohs_args = Vec::new();
    if let Some(dir) = &args.ohs {
        let pkg = OhsPackage::from_dir(dir).map_err(|e| format!("--ohs: {e}"))?;
        let root = pkg.root.to_str().ok_or_else(|| {
            format!(
                "{} không phải UTF-8, không ghi được vào JSON",
                pkg.root.display()
            )
        })?;
        ohs_args.extend(["--ohs".to_owned(), root.to_owned()]);
    }
    // Cờ đọc của `read` cộng `--ohs`: dùng cho cả lần dựng index và mục `pkm`.
    let with_ohs = |read: &InstallArgs| [read.read_args(), ohs_args.clone()].concat();
    let make_entry = |read_args: &[String]| -> Result<ServerEntry, String> {
        let mut entry = server_entry(&inst.exe, &vault, &db, read_args, args.about.as_deref())?;
        if args.no_ohs {
            entry.env.push((NO_OHS_ENV.to_owned(), "1".to_owned()));
        }
        Ok(entry)
    };
    let read_args = with_ohs(args);
    let mut entry = make_entry(&read_args)?;
    let mut after = entry.to_value();
    let mut changed = put_entry(&mut doc, &after);
    let p = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    let location = inst.exe_location_warning(&rerun_args(&entry.args, args));

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
        if let Some(w) = &location {
            p(
                out,
                format!("- cảnh báo (sẽ từ chối nếu không có --allow-any-location): {w}"),
            )?;
        }
        p(out, format!("- {}", ohs_plan(inst, args)))?;
        p(out, format!("- {}", warm_plan(args)))?;
        if may_switch_edges(args) {
            p(
                out,
                format!(
                    "- nếu index dựng theo mặc định có note mà không có cạnh nào: dựng lại với {} và ghi vào mục \
                     `{SERVER_NAME}` (--schema-edges-only để giữ mặc định)",
                    BODY_EDGE_FLAGS.join(" ")
                ),
            )?;
        }
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
                "- {} mục `{SERVER_NAME}`{}:",
                if changed { "ghi" } else { "giữ nguyên" },
                if legacy {
                    format!(" (đổi tên mục `{LEGACY_SERVER_NAME}` cũ do pkm ghi)")
                } else {
                    String::new()
                }
            ),
        )?;
        if let Some(note) = ohs_note(&doc) {
            p(out, format!("- {note}"))?;
        }
        write!(
            out,
            "{}",
            config::entry_diff(SERVER_NAME, before.as_ref(), Some(&after))
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    if let Some(w) = location {
        if !args.allow_any_location {
            return Err(format!(
                "{w}\nMuốn giữ chỗ này thì chạy lại với --allow-any-location."
            ));
        }
        p(out, format!("cảnh báo: {w}"))?;
    }
    refuse_if_desktop_running(hooks, args.force)?;
    install_ohs(inst, args, hooks, out)?;
    if let Some(dir) = db.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("tạo {}: {e}", dir.display()))?;
    }
    p(out, format!("dựng index {} …", db.display()))?;
    hooks
        .build_index(&inst.exe, &vault, &db, &read_args)
        .map_err(|e| format!("{e}; cấu hình Claude Desktop chưa đổi"))?;
    if body_edges_fallback(
        &inst.exe, args, &vault, &db, &read_args, &ohs_args, hooks, out,
    )? {
        let links = InstallArgs {
            edge_keys: Some("links".to_owned()),
            body_edges: true,
            ..args.clone()
        };
        entry = make_entry(&with_ohs(&links))?;
        after = entry.to_value();
        // Đặt mục mới lên bản đọc từ file, để `changed` so với cấu hình thật chứ không với mục vừa đặt ở trên.
        doc = load_with_digest(&path)
            .map_err(|e| format!("{e}. Index đã dựng xong; cấu hình Claude Desktop chưa đổi"))?
            .0;
        changed = put_entry(&mut doc, &after);
    }
    // So rồi mới ghi: trong lúc dựng index, Claude Desktop có thể đã mở, hoặc app khác đã sửa file cấu hình.
    refuse_if_desktop_running(hooks, args.force)
        .map_err(|e| format!("{e}. Index đã dựng xong; cấu hình Claude Desktop chưa đổi"))?;
    let (fresh, now) = load_with_digest(&path)
        .map_err(|e| format!("{e}. Index đã dựng xong; cấu hình Claude Desktop chưa đổi"))?;
    if now != digest {
        doc = fresh;
        changed = put_entry(&mut doc, &after);
        p(
            out,
            format!(
                "{} đã bị sửa trong lúc dựng index; đã đọc lại và chỉ đặt mục `{SERVER_NAME}` trên bản mới, giữ \
                 thay đổi kia",
                path.display()
            ),
        )?;
    }
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
    if let Some(note) = ohs_note(&doc) {
        p(out, note)?;
    }
    let hs = hooks
        .handshake(&entry, timeout)
        .map_err(|e| format!("cấu hình đã ghi nhưng server không bắt tay được: {e}"))?;
    p(
        out,
        format!(
            "server trả lời: {} {}, {} tool.",
            hs.server_name,
            hs.server_version,
            hs.tools.len()
        ),
    )?;
    warm(inst, args, &vault, hooks, out)?;
    p(
        out,
        format!("Mở lại Claude Desktop để dùng `{SERVER_NAME}`."),
    )
}

/// Mục `kg` của `doc` do `pkm` ghi (sẽ đổi tên thành `pkm`), khi chưa có mục `pkm`.
fn legacy_entry(doc: &ConfigDoc) -> bool {
    doc.server(SERVER_NAME).is_none()
        && doc
            .server(LEGACY_SERVER_NAME)
            .and_then(|v| ServerEntry::from_value(v).ok())
            .is_some_and(|e| written_by_pkm(&e))
}

/// Đặt mục `pkm` là `entry`: đổi tên mục `kg` cũ do `pkm` ghi tại chỗ của nó, không thì đặt mục `pkm`. Không đụng mục
/// nào khác (`kg` của Python hay người dùng, `ohs`). Trả `true` khi nội dung đổi.
fn put_entry(doc: &mut ConfigDoc, entry: &serde_json::Value) -> bool {
    if legacy_entry(doc) {
        doc.replace_server(LEGACY_SERVER_NAME, SERVER_NAME, entry.clone())
    } else {
        doc.set_server(SERVER_NAME, Some(entry.clone()))
    }
}

/// Mục do `pkm mcp install` ghi: `command` là binary `pkm` (`pkm`, `pkm.exe`) và `args` là `VAULT mcp ...`. Mục `kg`
/// của bản Python chạy `python` nên không khớp.
pub fn written_by_pkm(entry: &ServerEntry) -> bool {
    // Tách cả `/` lẫn `\`: cấu hình Windows đọc được trên mọi nền (test, WSL).
    let name = entry.command.rsplit(['/', '\\']).next().unwrap_or_default();
    let is_pkm = name.eq_ignore_ascii_case("pkm") || name.eq_ignore_ascii_case("pkm.exe");
    is_pkm && reinstall_args(&entry.args, false).is_some()
}

/// Câu báo khi người dùng còn MCP `ohs` riêng: giữ nguyên, chỉ nói là không cần.
fn ohs_note(doc: &ConfigDoc) -> Option<String> {
    doc.server(OHS_SERVER_NAME).map(|_| {
        format!(
            "giữ nguyên mục `{OHS_SERVER_NAME}` bạn tự thêm; `{SERVER_NAME}` đã có search và read nên có thể bỏ \
             `{OHS_SERVER_NAME}` (không bắt buộc; để cả hai thì agent thấy hai tool search và OHS nạp model hai lần)"
        )
    })
}

/// Cờ đọc cạnh mà bộ cài thêm khi graph theo mặc định rỗng: khoá frontmatter nào chứa link cũng là cạnh, và link
/// trong thân note cũng vậy.
pub const BODY_EDGE_FLAGS: [&str; 3] = ["--edge-keys", "links", "--body-edges"];

/// Người dùng không chọn cách đọc cạnh, nên bộ cài được đổi sang [`BODY_EDGE_FLAGS`] khi graph rỗng.
fn may_switch_edges(args: &InstallArgs) -> bool {
    args.edge_keys.is_none() && !args.body_edges && !args.schema_edges_only
}

/// Số note và số cạnh (mọi status) mà lần dựng gần nhất ghi vào `meta` của `db`; `None` khi không đọc được.
pub fn graph_size(db: &Path) -> Option<(u64, u64)> {
    use pkm_core::index::try_previous;
    let notes = try_previous(db, "notes").ok()??.parse().ok()?;
    let edges: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&try_previous(db, "edges").ok()??).ok()?;
    Some((
        notes,
        edges.values().filter_map(serde_json::Value::as_u64).sum(),
    ))
}

/// Vault Obsidian thường chỉ có wikilink trong thân note: cách đọc mặc định (như Python: chỉ khoá frontmatter theo
/// schema) cho graph rỗng và mọi tool graph trả rỗng. Khi [`may_switch_edges`] và index vừa dựng có note mà không có
/// cạnh nào, dựng lại với [`BODY_EDGE_FLAGS`]. Có cạnh thì giữ và trả `true` (mục `pkm` cần ghi thêm hai cờ); vẫn không
/// có thì dựng lại theo mặc định (`--edge-keys schema`, để index không nhớ cách đọc kia) và trả `false`.
#[allow(clippy::too_many_arguments)]
fn body_edges_fallback(
    exe: &Path,
    args: &InstallArgs,
    vault: &Path,
    db: &Path,
    read_args: &[String],
    ohs_args: &[String],
    hooks: &dyn Hooks,
    out: &mut dyn Write,
) -> Result<bool, String> {
    if !may_switch_edges(args) {
        return Ok(false);
    }
    let Some((notes @ 1.., 0)) = graph_size(db) else {
        return Ok(false);
    };
    let w = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    let flags = BODY_EDGE_FLAGS.join(" ");
    w(
        out,
        format!(
            "{notes} note mà không có cạnh nào khi chỉ đọc khoá frontmatter theo schema; dựng lại với {flags} (đọc \
             wikilink trong frontmatter và thân note) …"
        ),
    )?;
    let fail = |e: String| format!("{e}; cấu hình Claude Desktop chưa đổi");
    let links = InstallArgs {
        edge_keys: Some("links".to_owned()),
        body_edges: true,
        ..args.clone()
    };
    hooks
        .build_index(
            exe,
            vault,
            db,
            &[links.read_args(), ohs_args.to_vec()].concat(),
        )
        .map_err(fail)?;
    if let Some((_, edges @ 1..)) = graph_size(db) {
        w(
            out,
            format!(
                "dùng {flags}: {edges} cạnh. Muốn chỉ đọc khoá frontmatter theo schema thì cài lại với \
                 --schema-edges-only"
            ),
        )?;
        return Ok(true);
    }
    w(
        out,
        "vẫn không có cạnh nào (vault không có wikilink); giữ cách đọc mặc định".to_owned(),
    )?;
    let schema = [
        read_args.to_vec(),
        vec!["--edge-keys".to_owned(), "schema".to_owned()],
    ]
    .concat();
    hooks.build_index(exe, vault, db, &schema).map_err(fail)?;
    Ok(false)
}

/// Mô tả việc làm ấm index OHS (cho `--dry-run`).
fn warm_plan(args: &InstallArgs) -> String {
    if args.no_ohs {
        "không làm ấm index tìm kiếm (--no-ohs)".to_owned()
    } else if args.no_warm {
        "không làm ấm index tìm kiếm (--no-warm)".to_owned()
    } else {
        "làm ấm index tìm kiếm của OHS (chạy OHS tới khi index xong, tối đa 30 phút)".to_owned()
    }
}

/// Làm ấm index tìm kiếm của OHS sau khi cài. Không bao giờ làm lệnh cài thất bại: `pkm` vẫn chạy khi index chưa
/// xong (`search` báo `ohs_not_ready` tới lúc đó).
fn warm(
    inst: &Install,
    args: &InstallArgs,
    vault: &Path,
    hooks: &dyn Hooks,
    out: &mut dyn Write,
) -> Result<(), String> {
    let w = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    if args.no_ohs {
        return Ok(());
    }
    if args.no_warm {
        return w(
            out,
            "bỏ qua làm ấm index tìm kiếm (--no-warm); OHS sẽ index lần đầu khi Claude Desktop mở pkm".to_owned(),
        );
    }
    // Gói dùng chung chỉ ra bằng `--ohs DIR`, không thì gói cài ở thư mục của pkm.
    let dir = args.ohs.clone().unwrap_or_else(|| inst.ohs_dir());
    let Ok(pkg) = OhsPackage::from_dir(&dir) else {
        return Ok(());
    };
    w(
        out,
        format!(
            "làm ấm index tìm kiếm của OHS cho {} (lần đầu có thể mất vài phút; Ctrl+C để bỏ, OHS sẽ index tiếp khi \
             Claude Desktop mở pkm)…",
            vault.display()
        ),
    )?;
    let state = hooks.warm_ohs(pkg, vault, WARM_TIMEOUT, out);
    if state.is_ready() {
        w(out, "index tìm kiếm sẵn sàng".to_owned())
    } else {
        w(
            out,
            format!(
                "cảnh báo: index tìm kiếm chưa xong ({}): {} `{SERVER_NAME}` vẫn chạy; search báo ohs_not_ready tới \
                 khi xong",
                state.name(),
                state.action()
            ),
        )
    }
}

/// Mục `pkm`: `command` là binary `pkm`, `args` là `VAULT mcp --db DB <cờ đọc> [--ohs DIR] [--about TEXT]`
/// (`read_args` mang cả `--ohs`).
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

/// Đối số `pkm` để chạy lại `mcp install` cho mục `pkm` có `entry_args` (dạng [`server_entry`] ghi):
/// `mcp install VAULT --db DB <cờ đọc> [--about TEXT] [--no-ohs]`. `None` khi `entry_args` không có dạng đó.
pub fn reinstall_args(entry_args: &[String], no_ohs: bool) -> Option<Vec<String>> {
    let (vault, rest) = entry_args.split_first()?;
    let (mcp, rest) = rest.split_first()?;
    if vault.starts_with('-') || mcp != "mcp" {
        return None;
    }
    let mut out = vec!["mcp".to_owned(), "install".to_owned(), vault.clone()];
    out.extend(rest.iter().cloned());
    if no_ohs {
        out.push("--no-ohs".to_owned());
    }
    Some(out)
}

/// Đối số `pkm` để dựng lại index của mục `pkm`: `VAULT build --db DB <cờ đọc>` (bỏ `--about` và `--ohs`, cờ riêng
/// của MCP).
pub fn rebuild_args(entry_args: &[String]) -> Option<Vec<String>> {
    let mut args = reinstall_args(entry_args, false)?.split_off(2);
    for flag in ["--about", "--ohs"] {
        if let Some(i) = args.iter().position(|a| a == flag) {
            args.drain(i..(i + 2).min(args.len()));
        }
    }
    args.insert(1, "build".to_owned());
    Some(args)
}

/// Đối số để chạy lại đúng lần cài này: như [`reinstall_args`], thêm `--config` và nguồn gói OHS khi đã chỉ ra.
fn rerun_args(entry_args: &[String], args: &InstallArgs) -> Vec<String> {
    let mut out = reinstall_args(entry_args, args.no_ohs).unwrap_or_default();
    if let Some(c) = &args.config {
        out.extend(["--config".to_owned(), c.to_string_lossy().into_owned()]);
    }
    if let Some(o) = &args.ohs_package {
        out.extend(["--ohs-package".to_owned(), o.to_string_lossy().into_owned()]);
    }
    out
}

/// Lệnh bộ cài cho shell của máy này (repo public, tải thẳng không cần `gh`). Ghim tag của chính bản đang chạy:
/// `releases/latest` không bao giờ trỏ tới bản pre-release (`v0.1.0-rc.4`).
pub fn installer_command() -> String {
    let base = concat!("releases/download/v", env!("CARGO_PKG_VERSION"));
    if cfg!(windows) {
        format!("irm https://github.com/{RELEASE_REPO}/{base}/install.ps1 | iex")
    } else {
        format!("curl -fsSL https://github.com/{RELEASE_REPO}/{base}/install.sh | sh")
    }
}

/// Hai đường dẫn chỉ cùng một thư mục (so sau khi giải symlink; không giải được thì so nguyên văn).
fn same_dir(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// `exe` nằm trong thư mục tạm của hệ thống hoặc một thư mục tên `Downloads`: chỗ hay bị dọn.
fn is_temporary(exe: &Path) -> bool {
    let exe = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    let tmp = std::env::temp_dir();
    let tmp = std::fs::canonicalize(&tmp).unwrap_or(tmp);
    exe.starts_with(&tmp)
        || exe.components().any(|c| {
            c.as_os_str()
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case("downloads"))
        })
}

/// Đọc file cấu hình cùng sha256 của bytes đã đọc (`None` khi chưa có file), để biết file có bị sửa trước khi ghi.
fn load_with_digest(path: &Path) -> Result<(ConfigDoc, Option<[u8; 32]>), String> {
    let digest = match std::fs::read(path) {
        Ok(raw) => Some(Sha256::digest(&raw).into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("đọc {}: {e}", path.display())),
    };
    Ok((ConfigDoc::load(path)?, digest))
}

/// Thư mục gói OHS mà mục `pkm` chỉ ra bằng `--ohs` (cài với `mcp install --ohs DIR`).
pub fn entry_ohs(args: &[String]) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == "--ohs")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
}

/// Vault và DB đọc ngược từ `args` của mục `pkm` (dạng [`server_entry`] ghi).
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
    let legacy = legacy_entry(&doc);
    let value = doc
        .server(SERVER_NAME)
        .or_else(|| legacy.then(|| doc.server(LEGACY_SERVER_NAME)).flatten())
        .ok_or_else(|| {
            format!(
                "chưa có `{SERVER_NAME}` trong {}; chạy `pkm mcp install VAULT`",
                path.display()
            )
        })?;
    if legacy {
        writeln!(
            out,
            "mục `{LEGACY_SERVER_NAME}` cũ do pkm ghi; chạy lại `pkm mcp install VAULT` để đổi tên thành `{SERVER_NAME}`"
        )
        .map_err(|e| e.to_string())?;
    }
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
    if legacy_entry(doc) {
        doc.set_server(LEGACY_SERVER_NAME, None);
    }
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
    match hooks.desktop_process() {
        Ok(None) => Ok(()),
        Ok(Some(found)) => Err(format!(
            "Claude Desktop đang chạy ({found}): app giữ cấu hình trong bộ nhớ và ghi đè khi thoát. Thoát hẳn \
             Claude Desktop (cả biểu tượng ở khay hệ thống) rồi chạy lại. Nếu đó không phải Claude Desktop thì chạy \
             lại với --force"
        )),
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
    if let Some(d) = &args.ohs {
        return format!(
            "dùng gói OHS có sẵn ở {} (--ohs), không cài gói",
            d.display()
        );
    }
    if let Some(p) = &args.ohs_package {
        return format!("cài gói OHS từ {} vào {}", p.display(), dir.display());
    }
    if let Some(have) = installed_ohs(&dir, inst) {
        return have;
    }
    if let Some(src) = &inst.ohs_source {
        return format!(
            "cài gói OHS từ {src} ({}) vào {}",
            ohs_bundle::PACKAGE_ENV,
            dir.display()
        );
    }
    match inst.bundle {
        Some(b) => format!(
            "tải gói OHS {} cho {} từ {} (không cần đăng nhập), kiểm sha256 ghim, cài vào {}",
            ohs_bundle::VERSION,
            b.platform,
            b.url(),
            dir.display()
        ),
        None => format!("{}; bỏ qua", ohs_bundle::unsupported_platform()),
    }
}

/// Dòng báo gói OHS ở `dir` đã dùng được, nên không cài: gói có, và không cũ hơn bản ghim
/// ([`ohs_bundle::older_than_pin`]; gói mới hơn được giữ, không hạ cấp) hoặc người dùng đã chọn nguồn riêng
/// ([`ohs_bundle::PACKAGE_ENV`]). `None`: chưa có gói, hoặc gói cũ hơn bản ghim (bộ cài nâng lên bản ghim; tải lỗi thì
/// giữ gói cũ).
fn installed_ohs(dir: &Path, inst: &Install) -> Option<String> {
    let pkg = OhsPackage::from_dir(dir).ok()?;
    let version = pkg.version();
    if !ohs_bundle::older_than_pin(version.as_deref())
        || inst.ohs_source.is_some()
        || inst.bundle.is_none()
    {
        let v = version.unwrap_or_else(|| "không rõ phiên bản".to_owned());
        return Some(format!("gói OHS đã có ở {} ({v})", dir.display()));
    }
    None
}

/// Cài gói OHS theo `args`. Gói chỉ ra bằng `--ohs-package` mà lỗi thì dừng; tự tải (từ
/// [`ohs_bundle::PACKAGE_ENV`] nếu đặt, không thì gói ghim của bản phát hành) lỗi thì chỉ cảnh báo, vì `pkm` vẫn chạy được không
/// cần OHS (chỉ `related` không có `hits` cần OHS).
fn install_ohs(
    inst: &Install,
    args: &InstallArgs,
    hooks: &dyn Hooks,
    out: &mut dyn Write,
) -> Result<(), String> {
    if args.no_ohs {
        return Ok(());
    }
    if let Some(d) = &args.ohs {
        let v = OhsPackage::from_dir(d)
            .ok()
            .and_then(|p| p.version())
            .unwrap_or_else(|| "không rõ phiên bản".to_owned());
        return writeln!(out, "dùng gói OHS có sẵn ở {} ({v})", d.display())
            .map_err(|e| e.to_string());
    }
    let dest = inst.ohs_dir();
    let fetch = |url: &str, file: &Path| hooks.fetch(url, file);
    let install = |source: &Source, out: &mut dyn Write| {
        ohs_bundle::install(source, inst.bundle, &dest, out, &fetch)
    };
    let w = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    let not_ready = "`pkm` vẫn chạy; chỉ `search` và `related` không có `hits` cần OHS và sẽ báo ohs_not_ready tới khi cài gói";
    if let Some(p) = &args.ohs_package {
        let source = match p.to_str() {
            Some(text) => Source::parse(text)?,
            None => Source::Path(p.clone()),
        };
        let pkg = install(&source, out)?;
        return w(out, format!("đã cài gói OHS vào {}", pkg.root.display()));
    }
    if let Some(have) = installed_ohs(&dest, inst) {
        return w(out, have);
    }
    let (source, origin) = match &inst.ohs_source {
        Some(text) => (
            Source::parse(text),
            format!("{}={text}", ohs_bundle::PACKAGE_ENV),
        ),
        None if inst.bundle.is_some() => (
            Ok(Source::Download),
            format!("bản phát hành {}", ohs_bundle::TAG),
        ),
        None => {
            return w(
                out,
                format!(
                    "cảnh báo: {}. {not_ready}",
                    ohs_bundle::unsupported_platform()
                ),
            );
        }
    };
    match source.and_then(|s| install(&s, out)) {
        Ok(pkg) => w(out, format!("đã cài gói OHS vào {}", pkg.root.display())),
        Err(e) => w(
            out,
            format!("cảnh báo: chưa cài được gói OHS từ {origin} ({e}). {not_ready}"),
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
