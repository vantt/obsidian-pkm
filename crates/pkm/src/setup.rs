//! `pkm setup VAULT`: dựng bản B trên một vault bằng preset nhúng (`presets/b/`), thay `setup_vault.py` của repo
//! nghiên cứu. Mặc định chỉ in kế hoạch, `--apply` mới ghi; `--remove` gỡ những gì setup đã thêm (theo manifest
//! [`files::MANIFEST`]); `--json` in một báo cáo JSON cho vỏ GUI.
//!
//! Các bước, theo thứ tự, bước nào đã xong thì bỏ qua nên chạy lại an toàn:
//! 1. `plugins`: Homepage, Templater, Agent Client ghim phiên bản (sha256), bật trong `community-plugins.json`; core
//!    plugin Bases bật trong `core-plugins.json`. Thư mục plugin có sẵn (khác phiên bản) chỉ báo, không thay.
//! 2. `pkm-plugin`: plugin `pkm` của repo này, cùng tag với binary đang chạy (asset [`PLUGIN_ASSET`] kiểm theo
//!    `checksums.txt` của Release), hoặc từ bản dựng ở `--plugin-dir`; `data.json` (đường dẫn `pkm`, index) chỉ ghi khi
//!    chưa có.
//! 3. `theme`: snippet theme vào `.obsidian/snippets/`, bật trong `appearance.json`, cỡ chữ thân, tắt plugin ghi đè
//!    cỡ chữ. Theme Primary và plugin Style Settings không cài, chỉ cảnh báo khi thiếu. CSS lưới thư mục và nút phóng
//!    bảng nằm trong `styles.css` của plugin `pkm`, không còn là snippet hay plugin riêng.
//! 4. `agent`: chat trong Obsidian qua Agent Client: `.claude/CLAUDE.md`, `.claude/settings.local.json`, MCP `pkm` cho
//!    Claude Code (scope local), adapter ACP (npm, ghim phiên bản) và preset Claude Code của Agent Client.
//! 5. `mcp`: như `pkm mcp install VAULT` với cờ đọc của bản B: mục `pkm` của Claude Desktop, gói OHS, index, làm ấm.
//! 6. `writing`: cặp note gần nghĩa, đề xuất, vùng gợi ý bài viết; chỉ khi OHS sẵn sàng.
//!
//! "Dựng xưởng" là lệnh của plugin `pkm`, không phải của setup. Ghi file nguyên cả file (plugin, snippet, mẫu) luôn
//! so mã băm đã ghi: file người dùng đã sửa giữ nguyên ([`files::put_owned`]).
//!
//! Mọi việc chạm hệ thống mà test cần thay (tải, chạy `claude`, `npm`, `pkm`, tìm chương trình trên PATH, trạng thái
//! OHS) đi qua [`SetupHooks`].

pub mod files;
pub mod preset;

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use pkm_core::ohs::OhsPackage;
use serde_json::{Map, Value, json};

use crate::install::config::{self, ConfigDoc, SERVER_NAME, ServerEntry};
use crate::install::{self, Hooks, Install, InstallArgs, NO_OHS_ENV, RELEASE_REPO};
use files::{Manifest, Put, Removed, put_owned, read_json, remove_owned, write_file, write_json};
use preset::{PinnedPlugin, Preset};

/// Asset plugin `pkm` trong mỗi Release: zip phẳng có [`PLUGIN_FILES`].
pub const PLUGIN_ASSET: &str = "pkm-obsidian-plugin.zip";
/// Ba file Obsidian nạp từ `.obsidian/plugins/pkm/`.
pub const PLUGIN_FILES: [&str; 3] = ["main.js", "manifest.json", "styles.css"];
/// Id của plugin `pkm` (thư mục trong `.obsidian/plugins/`).
pub const PLUGIN_ID: &str = "pkm";
/// Phiên bản của binary; plugin phải cùng phiên bản.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Giới hạn mỗi file đọc từ zip plugin.
const MAX_PLUGIN_FILE: u64 = 64 * 1024 * 1024;
/// Chờ OHS sẵn sàng trước bước viết bài tối đa chừng này.
const OHS_READY_WAIT: Duration = Duration::from_secs(60);
/// Thư mục adapter ACP trong thư mục của pkm (ngoài vault).
const ACP_DIR: &str = "claude-agent-acp";
/// Trang vùng gợi ý bài viết, cặp gần nghĩa vào hàng chờ, tập rộng cho graph.
const PAIRS_QUEUE: &str = "_E11/similar-pairs.json";
const PAIRS_WIDE: &str = ".e11/similar-pairs.json";
/// Manifest của "Dựng xưởng" (plugin `pkm`).
const WORKSHOP_MANIFEST: &str = "_E11/workshop-ui-manifest.json";

/// Tuỳ chọn của `pkm setup`.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct SetupArgs {
    /// Vault cần dựng (thư mục có `.obsidian/`).
    pub vault: PathBuf,
    /// Làm thật; không có cờ này thì chỉ in kế hoạch, không ghi gì.
    #[arg(long)]
    pub apply: bool,
    /// Gỡ những gì setup đã thêm (theo `_E11/setup-manifest.json`); cùng --apply mới gỡ thật.
    #[arg(long)]
    pub remove: bool,
    /// In báo cáo JSON (`{vault, apply, remove, ok, error, steps}`) thay cho chữ.
    #[arg(long)]
    pub json: bool,
    /// Cài plugin `pkm` từ thư mục bản dựng này (main.js, manifest.json, styles.css) thay vì tải từ Release.
    #[arg(long, value_name = "DIR")]
    pub plugin_dir: Option<PathBuf>,
    /// File cấu hình Claude Desktop; mặc định tự tìm.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Dùng gói OHS đã cài ở thư mục này thay vì cài gói vào thư mục của pkm.
    #[arg(long, value_name = "DIR", conflicts_with = "no_ohs")]
    pub ohs: Option<PathBuf>,
    /// Không cài, không chạy OHS (bỏ cả bước viết bài).
    #[arg(long)]
    pub no_ohs: bool,
    /// Ghi cấu hình Claude Desktop kể cả khi app đang chạy hoặc không kiểm được.
    #[arg(long)]
    pub force: bool,
    /// Ghi kể cả khi binary pkm đang chạy không nằm ở chỗ cài chuẩn.
    #[arg(long)]
    pub allow_any_location: bool,
}

/// Tương tác với hệ thống của setup, thêm vào [`Hooks`] của `mcp install`.
pub trait SetupHooks: Hooks {
    /// Chương trình `name` trên PATH (Windows: thử cả đuôi trong PATHEXT).
    fn which(&self, name: &str) -> Option<PathBuf> {
        which(name)
    }

    /// Chạy `program args` trong `cwd`. stdout của nó đi sang stderr của pkm (stdout của `--json` chỉ có báo cáo);
    /// stderr được giữ lại: trả về khi thành công, nằm trong lỗi khi thất bại.
    fn run(&self, program: &Path, args: &[OsString], cwd: Option<&Path>) -> Result<String, String> {
        run_program(program, args, cwd)
    }
}

/// Hooks thật của máy.
pub struct SystemSetupHooks;

impl Hooks for SystemSetupHooks {
    /// Như bản mặc định, nhưng tóm tắt của `build` ra stderr để không lẫn vào JSON.
    fn build_index(
        &self,
        exe: &Path,
        vault: &Path,
        db: &Path,
        read_args: &[String],
    ) -> Result<(), String> {
        let mut args: Vec<OsString> = vec![vault.into(), "build".into(), "--db".into(), db.into()];
        args.extend(read_args.iter().map(OsString::from));
        self.run(exe, &args, None)
            .map(drop)
            .map_err(|e| format!("dựng index lỗi: {e}"))
    }
}

impl SetupHooks for SystemSetupHooks {}

/// Tìm `name` trên PATH.
pub fn which(name: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(str::to_owned)
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .find(|p| p.is_file())
    })
}

fn run_program(program: &Path, args: &[OsString], cwd: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(io::stderr()))
        .stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("không chạy được {}: {e}", program.display()))?;
    let mut err = Vec::new();
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_end(&mut err);
    }
    let status = child
        .wait()
        .map_err(|e| format!("{}: {e}", program.display()))?;
    let err = String::from_utf8_lossy(&err).trim().to_owned();
    if status.success() {
        Ok(err)
    } else {
        let tail: Vec<&str> = err.lines().rev().take(5).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        Err(format!(
            "{} thoát với {status}{}",
            program.display(),
            if tail.is_empty() {
                String::new()
            } else {
                format!(": {}", tail.join(" | "))
            }
        ))
    }
}

/// Trạng thái của một dòng báo cáo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Đã có từ trước, không làm gì.
    Ok,
    /// Sẽ làm (chạy thử).
    Todo,
    /// Đã làm.
    Done,
    /// Bỏ qua, kèm lý do và việc cần làm.
    Skip,
    /// Cảnh báo, người dùng cần làm gì đó.
    Warn,
    /// Thông tin (dòng in của `mcp install`, hướng dẫn).
    Info,
}

impl Status {
    pub fn code(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Todo => "todo",
            Status::Done => "done",
            Status::Skip => "skip",
            Status::Warn => "warn",
            Status::Info => "info",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Status::Ok => "  = ",
            Status::Todo => "  + ",
            Status::Done => "  ✓ ",
            Status::Skip => "  - bỏ qua: ",
            Status::Warn => "  ! cảnh báo: ",
            Status::Info => "    ",
        }
    }
}

/// Một bước của báo cáo: `(tên, tiêu đề, các dòng)`.
type LogStep = (String, String, Vec<(Status, String)>);

/// Báo cáo theo bước: in ngay ra `out` (chữ) hoặc gom lại in một lần (`--json`). Là [`Write`] để `mcp install` ghi
/// dòng của nó vào bước hiện tại.
pub struct Log<'a> {
    out: &'a mut dyn Write,
    json: bool,
    steps: Vec<LogStep>,
    partial: Vec<u8>,
}

impl<'a> Log<'a> {
    pub fn new(out: &'a mut dyn Write, json: bool) -> Self {
        Self {
            out,
            json,
            steps: Vec::new(),
            partial: Vec::new(),
        }
    }

    fn step(&mut self, name: &str, title: &str) -> Result<(), String> {
        self.flush_partial()?;
        self.steps
            .push((name.to_owned(), title.to_owned(), Vec::new()));
        if !self.json {
            writeln!(self.out, "[{name}] {title}").map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn add(&mut self, status: Status, text: impl Into<String>) -> Result<(), String> {
        let text = text.into();
        if !self.json {
            writeln!(self.out, "{}{text}", status.prefix()).map_err(|e| e.to_string())?;
        }
        if self.steps.is_empty() {
            self.steps
                .push(("setup".to_owned(), String::new(), Vec::new()));
        }
        if let Some((_, _, items)) = self.steps.last_mut() {
            items.push((status, text));
        }
        Ok(())
    }

    fn flush_partial(&mut self) -> Result<(), String> {
        if self.partial.is_empty() {
            return Ok(());
        }
        let line = String::from_utf8_lossy(&std::mem::take(&mut self.partial)).into_owned();
        self.add(Status::Info, line)
    }

    /// Mọi dòng đã ghi: `(bước, trạng thái, chữ)`.
    #[cfg(test)]
    pub fn items(&self) -> Vec<(String, Status, String)> {
        self.steps
            .iter()
            .flat_map(|(n, _, items)| items.iter().map(|(s, t)| (n.clone(), *s, t.clone())))
            .collect()
    }

    /// Kết thúc: `--json` thì in báo cáo, kèm lỗi nếu có.
    fn finish(&mut self, args: &SetupArgs, error: Option<&str>) -> Result<(), String> {
        self.flush_partial()?;
        if !self.json {
            return Ok(());
        }
        let steps: Vec<Value> = self
            .steps
            .iter()
            .map(|(name, title, items)| {
                json!({
                    "step": name,
                    "title": title,
                    "items": items
                        .iter()
                        .map(|(s, t)| json!({"status": s.code(), "text": t}))
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        let report = json!({
            "vault": args.vault.to_string_lossy(),
            "apply": args.apply,
            "remove": args.remove,
            "ok": error.is_none(),
            "error": error,
            "steps": steps,
        });
        let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        writeln!(self.out, "{text}").map_err(|e| e.to_string())
    }
}

impl Write for Log<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.partial.extend_from_slice(buf);
        while let Some(i) = self.partial.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.partial.drain(..=i).collect();
            let line = String::from_utf8_lossy(&line[..line.len() - 1])
                .trim_end_matches('\r')
                .to_owned();
            self.add(Status::Info, line).map_err(io::Error::other)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

/// Chạy `pkm setup` trên máy này với preset nhúng.
pub fn run(args: &SetupArgs, hooks: &dyn SetupHooks, out: &mut dyn Write) -> Result<(), String> {
    run_with(
        &Install::for_this_machine()?,
        &Preset::embedded(),
        args,
        hooks,
        out,
    )
}

/// [`run`] với chỗ đặt dữ liệu và preset truyền vào.
pub fn run_with(
    inst: &Install,
    preset: &Preset,
    args: &SetupArgs,
    hooks: &dyn SetupHooks,
    out: &mut dyn Write,
) -> Result<(), String> {
    let mut log = Log::new(out, args.json);
    let result = Ctx::new(inst, preset, args, hooks).and_then(|ctx| {
        if args.remove {
            ctx.remove(&mut log)
        } else {
            ctx.setup(&mut log)
        }
    });
    log.finish(args, result.as_ref().err().map(String::as_str))?;
    result
}

struct Ctx<'a> {
    inst: &'a Install,
    preset: &'a Preset,
    args: &'a SetupArgs,
    hooks: &'a dyn SetupHooks,
    vault: PathBuf,
    obs: PathBuf,
    db: PathBuf,
    apply: bool,
}

impl<'a> Ctx<'a> {
    fn new(
        inst: &'a Install,
        preset: &'a Preset,
        args: &'a SetupArgs,
        hooks: &'a dyn SetupHooks,
    ) -> Result<Self, String> {
        let vault = std::path::absolute(&args.vault)
            .map_err(|e| format!("{}: {e}", args.vault.display()))?;
        let obs = vault.join(".obsidian");
        if !obs.is_dir() {
            return Err(format!(
                "{} không phải vault Obsidian (không có .obsidian/); mở thư mục này bằng Obsidian một lần rồi chạy lại",
                vault.display()
            ));
        }
        Ok(Self {
            db: inst.default_db(&vault),
            inst,
            preset,
            args,
            hooks,
            vault,
            obs,
            apply: args.apply,
        })
    }

    /// Trạng thái của việc sắp làm: đã làm khi `--apply`, không thì sẽ làm.
    fn act(&self) -> Status {
        if self.apply {
            Status::Done
        } else {
            Status::Todo
        }
    }

    fn save(&self, m: &Manifest) -> Result<(), String> {
        if self.apply {
            m.save(&self.vault)
        } else {
            Ok(())
        }
    }

    fn setup(&self, log: &mut Log) -> Result<(), String> {
        log.add(
            Status::Warn,
            "đóng vault trong Obsidian trước: Obsidian ghi đè community-plugins.json khi đóng. Lần đầu mở, Obsidian \
             hỏi có tin plugin của vault không (chế độ hạn chế): chọn tin để bật plugin.",
        )?;
        if !self.apply {
            log.add(Status::Info, "chạy thử: không ghi gì; thêm --apply để làm")?;
        }
        let mut m = Manifest::load(&self.vault)?.unwrap_or_default();
        log.step("plugins", "Plugin cộng đồng và core plugin")?;
        self.plugins_step(log, &mut m)?;
        self.save(&m)?;
        log.step("pkm-plugin", "Plugin pkm")?;
        self.pkm_plugin_step(log, &mut m)?;
        self.save(&m)?;
        log.step("theme", "Theme")?;
        self.theme_step(log, &mut m)?;
        self.save(&m)?;
        log.step("agent", "Chat trong Obsidian (Agent Client, Claude Code)")?;
        self.agent_step(log, &mut m)?;
        self.save(&m)?;
        log.step("mcp", "Index, gói OHS, MCP pkm cho Claude Desktop")?;
        self.mcp_step(log, &mut m)?;
        self.save(&m)?;
        log.step(
            "writing",
            "Cặp note gần nghĩa, đề xuất, vùng gợi ý bài viết",
        )?;
        self.writing_step(log, &mut m)?;
        self.save(&m)?;
        log.step("not-ported", "Bước của setup_vault.py không chuyển")?;
        for (name, why) in [
            (
                "normalize",
                "chỉ cho vault chuyển từ Evernote, chạy tay bằng script của repo nghiên cứu",
            ),
            (
                "migrate_v3",
                "chuyển một lần sang schema v3 cho vault nghiên cứu",
            ),
            (
                "colours",
                "màu highlight từ file ENEX, chỉ cho vault chuyển từ Evernote",
            ),
            ("graph", "index graph đã dựng ở bước mcp"),
        ] {
            log.add(Status::Skip, format!("{name}: {why}"))?;
        }
        log.step("workshop", "Dựng xưởng")?;
        log.add(
            Status::Info,
            "mở vault trong Obsidian, Ctrl+P, chạy \"PKM: Dựng xưởng\" (Trang chủ, Bases, mẫu, trang chủ đề, cấu \
             hình Homepage và Templater), rồi tải lại Obsidian (Ctrl+R)",
        )?;
        log.add(
            Status::Info,
            if self.apply {
                "xong; kiểm máy bằng `pkm doctor`"
            } else {
                "chạy thử xong; thêm --apply để làm"
            },
        )
    }

    // ---- bước 1: plugin cộng đồng ---------------------------------------------------------------------------

    fn plugins_step(&self, log: &mut Log, m: &mut Manifest) -> Result<(), String> {
        for p in &self.preset.plugins {
            let dir = self.obs.join("plugins").join(&p.id);
            match files::plugin_version(&dir) {
                Some(v) if v == p.version => log.add(Status::Ok, format!("{} {v} đã có", p.id))?,
                Some(v) => log.add(
                    Status::Warn,
                    format!(
                        "{} {v} đã có, khác bản ghim {}; giữ nguyên (muốn bản ghim thì xoá thư mục \
                         .obsidian/plugins/{} rồi chạy lại)",
                        p.id, p.version, p.id
                    ),
                )?,
                None if dir.exists() => log.add(
                    Status::Warn,
                    format!(
                        "thư mục .obsidian/plugins/{} có sẵn nhưng không đọc được phiên bản; giữ nguyên",
                        p.id
                    ),
                )?,
                None => {
                    log.add(
                        self.act(),
                        format!(
                            "{} {}: tải từ github.com/{}, kiểm sha256 ghim",
                            p.id, p.version, p.repo
                        ),
                    )?;
                    if self.apply {
                        self.download_pinned(p, &dir)?;
                        m.add("plugins", std::slice::from_ref(&p.id));
                    }
                }
            }
        }
        let ids: Vec<String> = self.preset.plugins.iter().map(|p| p.id.clone()).collect();
        self.enable_plugins(log, m, &ids)?;
        self.enable_core(log, m)
    }

    /// Tải đủ file của `p` vào thư mục tạm, kiểm sha256 từng file, rồi đổi tên thành `dir`. Sai một file thì không
    /// ghi gì.
    fn download_pinned(&self, p: &PinnedPlugin, dir: &Path) -> Result<(), String> {
        let staging = self.staging(&p.id)?;
        let result = (|| {
            for (name, sha) in &p.files {
                let file = staging.join(name);
                self.hooks.fetch(&p.url(name), &file)?;
                let got = files::sha256_hex(
                    &std::fs::read(&file).map_err(|e| format!("đọc {}: {e}", file.display()))?,
                );
                if &got != sha {
                    return Err(format!(
                        "{} {name}: sha256 không khớp bản ghim (có {got}, cần {sha}); không ghi gì",
                        p.id
                    ));
                }
            }
            std::fs::rename(&staging, dir).map_err(|e| format!("ghi {}: {e}", dir.display()))
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        result
    }

    /// Thư mục tạm rỗng trong `.obsidian/plugins/` cho một lần tải.
    fn staging(&self, id: &str) -> Result<PathBuf, String> {
        let dir = self.obs.join("plugins").join(format!(".{id}.pkm-setup"));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| format!("xoá {}: {e}", dir.display()))?;
        }
        std::fs::create_dir_all(&dir).map_err(|e| format!("tạo {}: {e}", dir.display()))?;
        Ok(dir)
    }

    fn community_path(&self) -> PathBuf {
        self.obs.join("community-plugins.json")
    }

    /// Bật `ids` trong `community-plugins.json`; ghi những id đã thêm vào manifest.
    fn enable_plugins(
        &self,
        log: &mut Log,
        m: &mut Manifest,
        ids: &[String],
    ) -> Result<(), String> {
        let path = self.community_path();
        let mut list = read_array(&path)?;
        let add: Vec<String> = ids
            .iter()
            .filter(|id| !list.iter().any(|v| v.as_str() == Some(id.as_str())))
            .cloned()
            .collect();
        if add.is_empty() {
            return log.add(
                Status::Ok,
                format!("community-plugins.json: {} đã bật", ids.join(", ")),
            );
        }
        log.add(
            self.act(),
            format!("community-plugins.json: bật {}", add.join(", ")),
        )?;
        if self.apply {
            list.extend(add.iter().cloned().map(Value::String));
            write_json(&path, &Value::Array(list))?;
            m.add("community", &add);
        }
        Ok(())
    }

    fn enable_core(&self, log: &mut Log, m: &mut Manifest) -> Result<(), String> {
        let path = self.obs.join("core-plugins.json");
        let mut core = read_json(&path)?.unwrap_or_else(|| Value::Object(Map::new()));
        let mut added = Vec::new();
        let mut turned_on = Vec::new();
        for c in &self.preset.core_plugins {
            match &mut core {
                Value::Object(o) => match o.get(c) {
                    Some(Value::Bool(true)) => {}
                    Some(_) => {
                        o.insert(c.clone(), Value::Bool(true));
                        turned_on.push(c.clone());
                    }
                    None => {
                        o.insert(c.clone(), Value::Bool(true));
                        added.push(c.clone());
                    }
                },
                // Obsidian cũ ghi mảng id đang bật.
                Value::Array(a) => {
                    if !a.iter().any(|v| v.as_str() == Some(c.as_str())) {
                        a.push(Value::String(c.clone()));
                        added.push(c.clone());
                    }
                }
                _ => return Err(format!("{} không phải object hay mảng", path.display())),
            }
        }
        let all = [added.clone(), turned_on.clone()].concat();
        if all.is_empty() {
            return log.add(
                Status::Ok,
                format!(
                    "core-plugins.json: {} đã bật",
                    self.preset.core_plugins.join(", ")
                ),
            );
        }
        log.add(
            self.act(),
            format!("core-plugins.json: bật {}", all.join(", ")),
        )?;
        if self.apply {
            write_json(&path, &core)?;
            m.add("core", &added);
            m.add("core_off", &turned_on);
        }
        Ok(())
    }

    // ---- bước 2: plugin pkm ---------------------------------------------------------------------------------

    fn pkm_plugin_step(&self, log: &mut Log, m: &mut Manifest) -> Result<(), String> {
        let rel_dir = format!(".obsidian/plugins/{PLUGIN_ID}");
        let dir = self.vault.join(&rel_dir);
        let have = files::plugin_version(&dir);
        let source = match &self.args.plugin_dir {
            Some(d) => format!("bản dựng ở {}", d.display()),
            None => format!(
                "{PLUGIN_ASSET} của Release v{VERSION} (github.com/{RELEASE_REPO}), kiểm theo checksums.txt"
            ),
        };
        let files = if self.args.plugin_dir.is_none() && have.as_deref() == Some(VERSION) {
            log.add(Status::Ok, format!("pkm {VERSION} đã có"))?;
            None
        } else if self.args.plugin_dir.is_none() && !self.apply {
            log.add(Status::Todo, format!("pkm {VERSION}: cài từ {source}"))?;
            None
        } else {
            Some(self.plugin_files()?)
        };
        if let Some(files) = files {
            // Một file không do setup ghi (hay đã sửa) thì không ghi file nào, để plugin không thành nửa cũ nửa mới.
            let mut probe = m.clone();
            let mut outcomes = Vec::new();
            for (name, bytes) in &files {
                outcomes.push(put_owned(
                    &self.vault,
                    &format!("{rel_dir}/{name}"),
                    bytes,
                    &mut probe,
                    false,
                    false,
                )?);
            }
            if outcomes.contains(&Put::Kept) {
                log.add(
                    Status::Warn,
                    format!(
                        "plugin pkm ở {rel_dir} ({}) không do pkm setup cài hoặc đã bị sửa; giữ nguyên. Muốn bản \
                         {VERSION} thì xoá thư mục đó rồi chạy lại",
                        have.as_deref().unwrap_or("không rõ phiên bản")
                    ),
                )?;
            } else if outcomes.iter().all(|o| *o == Put::Same) {
                log.add(Status::Ok, format!("pkm {VERSION} đã có"))?;
            } else {
                log.add(self.act(), format!("pkm {VERSION}: cài từ {source}"))?;
                for (name, bytes) in &files {
                    put_owned(
                        &self.vault,
                        &format!("{rel_dir}/{name}"),
                        bytes,
                        m,
                        self.apply,
                        false,
                    )?;
                }
            }
        }
        let settings = self.plugin_settings()?;
        let rel = format!("{rel_dir}/data.json");
        match put_owned(&self.vault, &rel, &settings, m, self.apply, true)? {
            Put::Exists | Put::Same => log.add(
                Status::Ok,
                "cài đặt của plugin pkm (data.json) đã có, giữ nguyên",
            )?,
            _ => log.add(
                self.act(),
                format!(
                    "cài đặt của plugin pkm: đường dẫn pkm {}, index {}",
                    self.inst.exe.display(),
                    self.db.display()
                ),
            )?,
        }
        self.enable_plugins(log, m, &[PLUGIN_ID.to_owned()])
    }

    /// `data.json` của plugin `pkm` (khoá `pkmPath`, `graphDbPath` của cài đặt plugin): binary đang chạy và index mà mục
    /// MCP `pkm` dùng (plugin truyền nó làm `--db` cho material, zones, pairs).
    fn plugin_settings(&self) -> Result<Vec<u8>, String> {
        let text = |p: &Path| {
            p.to_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{} không phải UTF-8, không ghi được vào JSON", p.display()))
        };
        let v = json!({"pkmPath": text(&self.inst.exe)?, "graphDbPath": text(&self.db)?});
        Ok(serde_json::to_string_pretty(&v)
            .map_err(|e| e.to_string())?
            .into_bytes())
    }

    /// Ba file của plugin `pkm` từ `--plugin-dir` hoặc Release, đã kiểm `manifest.json` (id `pkm`, cùng phiên bản).
    fn plugin_files(&self) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
        let files = match &self.args.plugin_dir {
            Some(dir) => PLUGIN_FILES
                .iter()
                .map(|name| {
                    let p = dir.join(name);
                    std::fs::read(&p)
                        .map(|b| (*name, b))
                        .map_err(|e| format!("--plugin-dir: đọc {}: {e}", p.display()))
                })
                .collect::<Result<Vec<_>, String>>()?,
            None => self.release_plugin()?,
        };
        let manifest = files
            .iter()
            .find(|(n, _)| *n == "manifest.json")
            .map(|(_, b)| b)
            .ok_or("thiếu manifest.json của plugin pkm")?;
        let v: Value = serde_json::from_slice(manifest)
            .map_err(|e| format!("manifest.json của plugin pkm không phải JSON: {e}"))?;
        let (id, version) = (
            v.get("id").and_then(Value::as_str),
            v.get("version").and_then(Value::as_str),
        );
        if id != Some(PLUGIN_ID) || version != Some(VERSION) {
            return Err(format!(
                "plugin pkm có id {} phiên bản {}, cần id {PLUGIN_ID} phiên bản {VERSION} (cùng binary pkm)",
                id.unwrap_or("?"),
                version.unwrap_or("?")
            ));
        }
        Ok(files)
    }

    /// Tải [`PLUGIN_ASSET`] và `checksums.txt` của Release cùng tag, kiểm sha256, đọc ba file trong zip.
    fn release_plugin(&self) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
        let base = format!("https://github.com/{RELEASE_REPO}/releases/download/v{VERSION}");
        let staging = self.staging(PLUGIN_ID)?;
        let result = (|| {
            let sums = staging.join("checksums.txt");
            let zip = staging.join(PLUGIN_ASSET);
            self.hooks.fetch(&format!("{base}/checksums.txt"), &sums)?;
            self.hooks.fetch(&format!("{base}/{PLUGIN_ASSET}"), &zip)?;
            let sums =
                std::fs::read_to_string(&sums).map_err(|e| format!("đọc checksums.txt: {e}"))?;
            let want = checksum_for(&sums, PLUGIN_ASSET)?;
            let bytes = std::fs::read(&zip).map_err(|e| format!("đọc {PLUGIN_ASSET}: {e}"))?;
            let got = files::sha256_hex(&bytes);
            if got != want {
                return Err(format!(
                    "{PLUGIN_ASSET}: sha256 {got} khác checksums.txt ({want}); tải lỗi hoặc bị sửa, không ghi gì"
                ));
            }
            unzip_plugin(&bytes)
        })();
        let _ = std::fs::remove_dir_all(&staging);
        result
    }

    // ---- bước 3: theme --------------------------------------------------------------------------------------

    fn theme_step(&self, log: &mut Log, m: &mut Manifest) -> Result<(), String> {
        let p = self.preset;
        for (name, bytes) in &p.snippets {
            let rel = format!(".obsidian/snippets/{name}.css");
            match put_owned(&self.vault, &rel, bytes, m, self.apply, false)? {
                Put::Same | Put::Exists => log.add(Status::Ok, format!("{rel} đã có"))?,
                Put::Wrote => log.add(self.act(), format!("ghi {rel}"))?,
                Put::Updated => log.add(self.act(), format!("cập nhật {rel}"))?,
                Put::Kept => log.add(
                    Status::Warn,
                    format!("{rel} đã bị sửa hoặc không do pkm setup ghi; giữ nguyên"),
                )?,
            }
        }
        let path = self.obs.join("appearance.json");
        let mut cfg = match read_json(&path)? {
            None => Map::new(),
            Some(Value::Object(o)) => o,
            Some(_) => return Err(format!("{} không phải object", path.display())),
        };
        let mut changed = false;
        let mut enabled: Vec<Value> = cfg
            .get("enabledCssSnippets")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let add: Vec<String> = p
            .snippets
            .iter()
            .map(|(n, _)| n.clone())
            .filter(|n| !enabled.iter().any(|v| v.as_str() == Some(n.as_str())))
            .collect();
        if add.is_empty() {
            log.add(Status::Ok, "snippet đã bật trong appearance.json")?;
        } else {
            log.add(
                self.act(),
                format!("appearance.json: bật snippet {}", add.join(", ")),
            )?;
            enabled.extend(add.iter().cloned().map(Value::String));
            cfg.insert("enabledCssSnippets".to_owned(), Value::Array(enabled));
            m.add("snippets_on", &add);
            changed = true;
        }
        let size = Value::from(p.base_font_size);
        let old = cfg.get("baseFontSize").cloned();
        if old.as_ref() == Some(&size) {
            log.add(Status::Ok, format!("cỡ chữ thân đã là {size}"))?;
        } else {
            log.add(
                self.act(),
                format!(
                    "cỡ chữ thân {} -> {size}",
                    old.as_ref()
                        .map_or_else(|| "mặc định".to_owned(), Value::to_string)
                ),
            )?;
            if m.get("font_size").is_none() {
                m.set("font_size", old.unwrap_or(Value::Null));
            }
            cfg.insert("baseFontSize".to_owned(), size);
            changed = true;
        }
        if changed && self.apply {
            write_json(&path, &Value::Object(cfg.clone()))?;
        }
        // Plugin ghi đè cỡ chữ: tắt, nhớ để bật lại khi gỡ.
        let cp = self.community_path();
        let mut list = read_array(&cp)?;
        let off: Vec<String> = p
            .disable_plugins
            .iter()
            .filter(|id| list.iter().any(|v| v.as_str() == Some(id.as_str())))
            .cloned()
            .collect();
        if !off.is_empty() {
            log.add(
                self.act(),
                format!("tắt {} (ghi đè cỡ chữ của theme)", off.join(", ")),
            )?;
            if self.apply {
                list.retain(|v| !v.as_str().is_some_and(|s| off.iter().any(|o| o == s)));
                write_json(&cp, &Value::Array(list))?;
                m.add("plugins_off", &off);
            }
        }
        let theme = cfg.get("cssTheme").and_then(Value::as_str).unwrap_or("");
        let theme_dir = self.obs.join("themes").join(&p.css_theme);
        if theme != p.css_theme || !theme_dir.is_dir() {
            log.add(
                Status::Warn,
                format!(
                    "snippet viết cho theme {} {}, vault đang dùng {}. Cài và chọn theme đó: Cài đặt > Giao diện > \
                     Theme > Quản lý > tìm \"{}\" > Cài đặt và dùng",
                    p.css_theme,
                    p.css_theme_version,
                    if theme.is_empty() {
                        "theme mặc định"
                    } else {
                        theme
                    },
                    p.css_theme
                ),
            )?;
        }
        if !self.obs.join("plugins").join(&p.style_settings).is_dir() {
            log.add(
                Status::Warn,
                format!(
                    "chưa có plugin Style Settings ({}): cần để chọn font đọc của theme. Cài: Cài đặt > Plugin cộng \
                     đồng > Duyệt > tìm \"Style Settings\" > Cài đặt > Bật",
                    p.style_settings
                ),
            )?;
        }
        Ok(())
    }

    // ---- bước 4: agent --------------------------------------------------------------------------------------

    fn agent_step(&self, log: &mut Log, m: &mut Manifest) -> Result<(), String> {
        let p = self.preset;
        let rel = &p.guide_target;
        match put_owned(&self.vault, rel, p.guide, m, self.apply, true)? {
            Put::Exists | Put::Same => log.add(Status::Ok, format!("{rel} đã có, giữ nguyên"))?,
            _ => log.add(
                self.act(),
                format!("ghi {rel} (cách trả lời về note đang mở)"),
            )?,
        }
        self.allow_tools(log, m)?;
        let Some(claude) = self.hooks.which("claude") else {
            return log.add(
                Status::Skip,
                "không thấy Claude Code (lệnh claude): cài Claude Code, đăng nhập, rồi chạy lại để thêm MCP pkm cho \
                 Claude Code và chat trong Obsidian",
            );
        };
        self.claude_mcp(log, m, &claude)?;
        let Some(wrapper) = self.acp_adapter(log, &claude)? else {
            return Ok(());
        };
        self.agent_client_preset(log, m, &wrapper)
    }

    /// `.claude/settings.local.json`: cho phép tool chỉ đọc của `pkm` không hỏi.
    fn allow_tools(&self, log: &mut Log, m: &mut Manifest) -> Result<(), String> {
        let path = self.vault.join(".claude").join("settings.local.json");
        let mut settings = match read_json(&path)? {
            None => Map::new(),
            Some(Value::Object(o)) => o,
            Some(_) => return Err(format!("{} không phải object", path.display())),
        };
        let perms = settings
            .entry("permissions")
            .or_insert_with(|| Value::Object(Map::new()));
        let Some(perms) = perms.as_object_mut() else {
            return Err(format!("{}: permissions không phải object", path.display()));
        };
        let allow = perms
            .entry("allow")
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(allow) = allow.as_array_mut() else {
            return Err(format!(
                "{}: permissions.allow không phải mảng",
                path.display()
            ));
        };
        let add: Vec<String> = self
            .preset
            .allow
            .iter()
            .filter(|a| !allow.iter().any(|v| v.as_str() == Some(a.as_str())))
            .cloned()
            .collect();
        if add.is_empty() {
            return log.add(
                Status::Ok,
                ".claude/settings.local.json: tool của pkm đã được cho phép",
            );
        }
        log.add(
            self.act(),
            format!(".claude/settings.local.json: cho phép {}", add.join(", ")),
        )?;
        if self.apply {
            allow.extend(add.iter().cloned().map(Value::String));
            write_json(&path, &Value::Object(settings))?;
            m.add("agent_allow", &add);
        }
        Ok(())
    }

    /// Mục MCP `pkm` mà Claude Code (và Claude Desktop) chạy cho vault này.
    fn pkm_entry(&self) -> Result<ServerEntry, String> {
        let mut read = self.preset.mcp_flags.clone();
        if let Some(dir) = &self.args.ohs {
            let pkg = OhsPackage::from_dir(dir).map_err(|e| format!("--ohs: {e}"))?;
            let root = pkg
                .root
                .to_str()
                .ok_or_else(|| format!("{} không phải UTF-8", pkg.root.display()))?;
            read.extend(["--ohs".to_owned(), root.to_owned()]);
        }
        let mut entry = install::server_entry(
            &self.inst.exe,
            &self.vault,
            &self.db,
            &read,
            Some(&self.preset.about),
        )?;
        if self.args.no_ohs {
            entry.env.push((NO_OHS_ENV.to_owned(), "1".to_owned()));
        }
        Ok(entry)
    }

    /// MCP `pkm` cho Claude Code ở scope local (thư mục vault), thay các server cũ.
    fn claude_mcp(&self, log: &mut Log, m: &mut Manifest, claude: &Path) -> Result<(), String> {
        let name = &self.preset.mcp_name;
        let entry = self.pkm_entry()?;
        let mut add: Vec<String> = ["mcp", "add", "-s", "local", name]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        for (k, v) in &entry.env {
            add.extend(["-e".to_owned(), format!("{k}={v}")]);
        }
        add.push("--".to_owned());
        add.push(entry.command.clone());
        add.extend(entry.args.iter().cloned());
        let recorded = m.get("claude_mcp").cloned();
        if recorded == Some(json!(add)) {
            return log.add(
                Status::Ok,
                format!("MCP {name} cho Claude Code (scope local) đã có"),
            );
        }
        log.add(
            self.act(),
            format!(
                "MCP {name} cho Claude Code, scope local, thay {}: {}",
                self.preset.mcp_replaces.join(", "),
                install::shell::command_line(&claude.to_string_lossy(), &add)
            ),
        )?;
        if !self.apply {
            return Ok(());
        }
        let os = |v: &[&str]| v.iter().map(OsString::from).collect::<Vec<_>>();
        for old in self.preset.mcp_replaces.iter().chain(std::iter::once(name)) {
            // Server chưa có thì `claude mcp remove` báo lỗi: bỏ qua.
            let _ = self.hooks.run(
                claude,
                &os(&["mcp", "remove", "-s", "local", old]),
                Some(&self.vault),
            );
        }
        m.drop_items("mcp", &self.preset.mcp_replaces);
        let args: Vec<OsString> = add.iter().map(OsString::from).collect();
        self.hooks
            .run(claude, &args, Some(&self.vault))
            .map_err(|e| {
                format!(
                    "claude mcp add {name} lỗi: {e}; các bước trước giữ nguyên, sửa rồi chạy lại"
                )
            })?;
        m.add("mcp", std::slice::from_ref(name));
        m.set("claude_mcp", json!(add));
        Ok(())
    }

    /// Adapter ACP (npm, ghim) và script khởi động nó với Claude Code của máy này; trả đường dẫn script khi đã có
    /// (hoặc sẽ có, khi chạy thử).
    fn acp_adapter(&self, log: &mut Log, claude: &Path) -> Result<Option<PathBuf>, String> {
        let p = self.preset;
        let acp = self.inst.pkm_dir.join(ACP_DIR);
        let pkg_dir = p
            .acp_package
            .split('/')
            .fold(acp.join("node_modules"), |d, part| d.join(part));
        let have = read_json(&pkg_dir.join("package.json"))
            .ok()
            .flatten()
            .and_then(|v| v.get("version")?.as_str().map(str::to_owned));
        let spec = format!("{}@{}", p.acp_package, p.acp_version);
        if have.as_deref() == Some(p.acp_version.as_str()) {
            log.add(
                Status::Ok,
                format!(
                    "adapter {} {} đã có ở {}",
                    p.acp_package,
                    p.acp_version,
                    acp.display()
                ),
            )?;
        } else {
            let npm_args = vec![
                "install".to_owned(),
                "--prefix".to_owned(),
                acp.to_string_lossy().into_owned(),
                spec.clone(),
            ];
            let Some(npm) = self.hooks.which("npm") else {
                log.add(
                    Status::Skip,
                    format!(
                        "không có npm để cài adapter ACP cho chat trong Obsidian. Cài Node.js (có npm) rồi chạy: {} \
                         và chạy lại pkm setup",
                        install::shell::command_line("npm", &npm_args)
                    ),
                )?;
                return Ok(None);
            };
            log.add(
                self.act(),
                format!(
                    "cài adapter: {}",
                    install::shell::command_line(&npm.to_string_lossy(), &npm_args)
                ),
            )?;
            if self.apply {
                let args: Vec<OsString> = npm_args.iter().map(OsString::from).collect();
                self.hooks.run(&npm, &args, None).map_err(|e| {
                    format!(
                        "npm install {spec} lỗi: {e}; các bước trước giữ nguyên, sửa rồi chạy lại"
                    )
                })?;
            }
        }
        let entry = pkg_dir.join("dist").join("index.js");
        let node = self
            .hooks
            .which("node")
            .unwrap_or_else(|| PathBuf::from("node"));
        let (name, text) = if cfg!(windows) {
            (
                "claude-agent-acp.cmd",
                format!(
                    "@echo off\r\nset CLAUDE_CODE_EXECUTABLE={}\r\n\"{}\" \"{}\" %*\r\n",
                    claude.display(),
                    node.display(),
                    entry.display()
                ),
            )
        } else {
            (
                "claude-agent-acp.sh",
                format!(
                    "#!/bin/sh\nCLAUDE_CODE_EXECUTABLE=\"{}\" exec \"{}\" \"{}\" \"$@\"\n",
                    claude.display(),
                    node.display(),
                    entry.display()
                ),
            )
        };
        let wrapper = acp.join(name);
        if files::read_opt(&wrapper)?.as_deref() == Some(text.as_bytes()) {
            log.add(Status::Ok, format!("{} đã có", wrapper.display()))?;
        } else {
            log.add(
                self.act(),
                format!(
                    "ghi {} (chạy adapter với Claude Code của máy này)",
                    wrapper.display()
                ),
            )?;
            if self.apply {
                write_file(&wrapper, text.as_bytes())?;
                make_executable(&wrapper)?;
            }
        }
        Ok(Some(wrapper))
    }

    /// Preset Claude Code của Agent Client chạy `wrapper`, trừ khi người dùng đã tự đặt lệnh khác.
    fn agent_client_preset(
        &self,
        log: &mut Log,
        m: &mut Manifest,
        wrapper: &Path,
    ) -> Result<(), String> {
        let p = self.preset;
        let command = wrapper.to_string_lossy().into_owned();
        let path = self
            .obs
            .join("plugins")
            .join("agent-client")
            .join("data.json");
        let data = read_json(&path)?;
        let mut preset = data
            .as_ref()
            .and_then(|d| {
                d.get("presetAgents")?
                    .get(&p.agent_preset)?
                    .as_object()
                    .cloned()
            })
            .unwrap_or_default();
        let old = preset
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        if old == command {
            return log.add(
                Status::Ok,
                "Agent Client: preset Claude Code đã chạy adapter",
            );
        }
        if !old.is_empty() && old != p.agent_preset_default_command {
            return log.add(
                Status::Ok,
                format!("Agent Client: lệnh Claude Code là {old} (người dùng đặt), giữ nguyên"),
            );
        }
        log.add(
            self.act(),
            format!("Agent Client: preset Claude Code chạy {command}"),
        )?;
        if !self.apply {
            return Ok(());
        }
        for (k, v) in [
            ("id", json!(p.agent_preset)),
            ("displayName", json!("Claude Code")),
            ("apiKeySecretId", json!("")),
            ("args", json!([])),
            ("env", json!([])),
            ("enabled", json!(true)),
        ] {
            preset.entry(k).or_insert(v);
        }
        preset.insert("command".to_owned(), Value::String(command.clone()));
        let created = data.is_none();
        let mut out = match data {
            Some(Value::Object(o)) => o,
            None => Map::new(),
            Some(_) => return Err(format!("{} không phải object", path.display())),
        };
        let agents = out
            .entry("presetAgents")
            .or_insert_with(|| Value::Object(Map::new()));
        if !agents.is_object() {
            *agents = Value::Object(Map::new());
        }
        if let Some(a) = agents.as_object_mut() {
            a.insert(p.agent_preset.clone(), Value::Object(preset));
        }
        write_json(&path, &Value::Object(out))?;
        m.set(
            "agent_settings",
            json!({"old": old, "created": created, "command": command}),
        );
        Ok(())
    }

    // ---- bước 5: index, OHS, MCP cho Claude Desktop ---------------------------------------------------------

    /// Cờ của `pkm mcp install` cho bản B.
    fn install_args(&self) -> Result<InstallArgs, String> {
        #[derive(clap::Parser)]
        struct Flags {
            #[command(flatten)]
            args: InstallArgs,
        }
        use clap::Parser;
        let argv = [
            OsString::from("pkm setup"),
            self.vault.clone().into_os_string(),
        ]
        .into_iter()
        .chain(self.preset.mcp_flags.iter().map(OsString::from));
        let mut a = Flags::try_parse_from(argv)
            .map_err(|e| format!("cờ mcp.flags của setup.json không hợp lệ: {e}"))?
            .args;
        a.db = Some(self.db.clone());
        a.about = Some(self.preset.about.clone());
        a.config = self.args.config.clone();
        a.ohs = self.args.ohs.clone();
        a.no_ohs = self.args.no_ohs;
        a.force = self.args.force;
        a.allow_any_location = self.args.allow_any_location;
        a.dry_run = !self.apply;
        Ok(a)
    }

    fn mcp_step(&self, log: &mut Log, m: &mut Manifest) -> Result<(), String> {
        let a = self.install_args()?;
        let path = config::locate(a.config.as_deref())?;
        let had_entry = ConfigDoc::load(&path)?.server(SERVER_NAME).is_some();
        let hooks: &dyn Hooks = self.hooks;
        install::run_with(self.inst, &a, hooks, log)?;
        if self.apply && !had_entry {
            m.set(
                "desktop_config",
                Value::String(path.to_string_lossy().into_owned()),
            );
        }
        Ok(())
    }

    // ---- bước 6: viết bài -----------------------------------------------------------------------------------

    fn writing_step(&self, log: &mut Log, m: &mut Manifest) -> Result<(), String> {
        if self.args.no_ohs {
            return log.add(
                Status::Skip,
                "--no-ohs: cặp note, đề xuất liên quan và vùng cần OHS; chạy lại không có --no-ohs khi muốn",
            );
        }
        let dir = self.args.ohs.clone().unwrap_or_else(|| self.inst.ohs_dir());
        let ohs: Vec<OsString> = match &self.args.ohs {
            Some(d) => vec!["--ohs".into(), d.into()],
            None => Vec::new(),
        };
        let v = |s: &str| OsString::from(s);
        let vault = OsString::from(&self.vault);
        let queue = self.vault.join(PAIRS_QUEUE);
        // (việc, file nó ghi, đối số của pkm, cần hàng chờ cặp gần nghĩa)
        let mut plan: Vec<(&str, Option<&str>, Vec<OsString>, bool)> = Vec::new();
        if !queue.exists() {
            plan.push((
                "cặp gần nghĩa vào hàng chờ đề xuất",
                Some(PAIRS_QUEUE),
                [vec![vault.clone(), v("pairs"), v("--write")], ohs.clone()].concat(),
                false,
            ));
        } else {
            log.add(Status::Ok, format!("{PAIRS_QUEUE} đã có"))?;
        }
        if !self.vault.join(PAIRS_WIDE).exists() {
            plan.push((
                "cặp gần nghĩa cho graph (tập rộng)",
                Some(PAIRS_WIDE),
                [
                    vec![
                        vault.clone(),
                        v("pairs"),
                        v("--out"),
                        self.vault.join(PAIRS_WIDE).into(),
                    ],
                    ["--min", "0.9", "--max", "3000", "--per-note", "5"]
                        .iter()
                        .map(|s| v(s))
                        .collect(),
                    ohs.clone(),
                ]
                .concat(),
                false,
            ));
        } else {
            log.add(Status::Ok, format!("{PAIRS_WIDE} đã có"))?;
        }
        plan.push((
            "đề xuất mới vào _Đề xuất/ (đề xuất đã có không lặp lại)",
            None,
            vec![
                vault.clone(),
                v("propose"),
                v("--create"),
                v("--similar"),
                queue.clone().into(),
            ],
            true,
        ));
        let page = pkm_core::zones::PAGE;
        if self.vault.join(page).exists() {
            log.add(
                Status::Ok,
                format!("{page} đã có (giữ dòng Chấm của bạn); làm mới bằng tay: pkm VAULT zones --apply sau khi xoá trang"),
            )?;
        } else {
            plan.push((
                "vùng gợi ý bài viết",
                Some(page),
                [
                    vec![
                        vault.clone(),
                        v("zones"),
                        v("--apply"),
                        v("--db"),
                        self.db.clone().into(),
                    ],
                    ohs.clone(),
                ]
                .concat(),
                false,
            ));
        }
        if !self.apply {
            log.add(
                Status::Info,
                "chạy khi OHS sẵn sàng sau bước mcp (index tìm kiếm xong), không thì bỏ qua:",
            )?;
            for (what, _, args, _) in &plan {
                log.add(Status::Todo, format!("{what}: {}", self.pkm_line(args)))?;
            }
            return Ok(());
        }
        let Ok(pkg) = OhsPackage::from_dir(&dir) else {
            return log.add(
                Status::Skip,
                format!(
                    "chưa có gói OHS ở {}: cặp note, đề xuất liên quan và vùng cần OHS; cài gói rồi chạy lại",
                    dir.display()
                ),
            );
        };
        let state = self.hooks.ohs_state(pkg, &self.vault, OHS_READY_WAIT);
        if !state.is_ready() {
            return log.add(
                Status::Skip,
                format!(
                    "OHS chưa sẵn sàng ({}): {} Chạy lại pkm setup --apply khi index tìm kiếm xong",
                    state.name(),
                    state.action()
                ),
            );
        }
        for (what, out, args, needs_queue) in plan {
            if needs_queue && !queue.exists() {
                log.add(Status::Skip, format!("{what}: chưa có {PAIRS_QUEUE}"))?;
                continue;
            }
            let said = self
                .hooks
                .run(&self.inst.exe, &args, None)
                .map_err(|e| format!("{what}: {e}; các bước trước giữ nguyên, sửa rồi chạy lại"))?;
            let last = said.lines().last().unwrap_or("").trim().to_owned();
            log.add(
                Status::Done,
                if last.is_empty() {
                    what.to_owned()
                } else {
                    format!("{what}: {last}")
                },
            )?;
            if let Some(rel) = out
                && let Some(bytes) = files::read_opt(&self.vault.join(rel))?
            {
                m.set_file(rel, Some(files::sha256_hex(&bytes)));
            }
        }
        Ok(())
    }

    /// Dòng lệnh `pkm ...` để in.
    fn pkm_line(&self, args: &[OsString]) -> String {
        let args: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        install::shell::command_line(&self.inst.exe.to_string_lossy(), &args)
    }

    // ---- gỡ -------------------------------------------------------------------------------------------------

    fn remove(&self, log: &mut Log) -> Result<(), String> {
        let Some(mut m) = Manifest::load(&self.vault)? else {
            return Err(format!(
                "không có gì để gỡ: chưa có {} trong vault",
                files::MANIFEST
            ));
        };
        log.add(
            Status::Warn,
            "đóng vault trong Obsidian trước: Obsidian ghi đè community-plugins.json khi đóng",
        )?;
        if !self.apply {
            log.add(Status::Info, "chạy thử: không gỡ gì; thêm --apply để gỡ")?;
        }
        if self.vault.join(WORKSHOP_MANIFEST).exists() {
            log.add(
                Status::Warn,
                "xưởng còn đó (_E11/workshop-ui-manifest.json): gỡ bằng lệnh \"PKM: Gỡ xưởng\" trong Obsidian trước \
                 khi gỡ plugin pkm, không thì cài lại plugin để gỡ",
            )?;
        }
        log.step("files", "File setup đã ghi")?;
        for (rel, sha) in m.files() {
            match remove_owned(&self.vault, &rel, &sha, self.apply)? {
                Removed::Deleted => {
                    log.add(self.act(), format!("xoá {rel}"))?;
                    if self.apply {
                        m.set_file(&rel, None);
                    }
                }
                Removed::Edited => {
                    log.add(Status::Ok, format!("giữ {rel} (đã sửa từ lần setup)"))?;
                    if self.apply {
                        m.set_file(&rel, None);
                    }
                }
                Removed::Missing => {
                    if self.apply {
                        m.set_file(&rel, None);
                    }
                }
            }
        }
        // Manifest của setup_vault.py: mã băm CLAUDE.md ở khoá riêng.
        if let Some(sha) = m
            .get("agent_guide")
            .and_then(Value::as_str)
            .map(str::to_owned)
        {
            let rel = self.preset.guide_target.clone();
            match remove_owned(&self.vault, &rel, &sha, self.apply)? {
                Removed::Deleted => log.add(self.act(), format!("xoá {rel}"))?,
                Removed::Edited => {
                    log.add(Status::Ok, format!("giữ {rel} (đã sửa từ lần setup)"))?
                }
                Removed::Missing => {}
            }
        }
        if self.apply {
            let _ = files::remove_dir_if_empty(&self.obs.join("plugins").join(PLUGIN_ID));
        }

        log.step("mcp", "MCP pkm")?;
        if let Some(cfg) = m.get("desktop_config").and_then(Value::as_str) {
            let a = InstallArgs {
                config: Some(PathBuf::from(cfg)),
                remove: true,
                dry_run: !self.apply,
                force: self.args.force,
                timeout: 120,
                ..InstallArgs::default()
            };
            let hooks: &dyn Hooks = self.hooks;
            install::run_with(self.inst, &a, hooks, log)?;
        }
        let names = m.list("mcp");
        if !names.is_empty() {
            let claude = self.hooks.which("claude");
            for name in &names {
                let line = format!("claude mcp remove -s local {name}");
                match &claude {
                    Some(c) => {
                        log.add(self.act(), format!("{line} (trong thư mục vault)"))?;
                        if self.apply {
                            let args: Vec<OsString> = ["mcp", "remove", "-s", "local", name]
                                .iter()
                                .map(OsString::from)
                                .collect();
                            if let Err(e) = self.hooks.run(c, &args, Some(&self.vault)) {
                                log.add(Status::Warn, format!("{line}: {e}"))?;
                            }
                        }
                    }
                    None => log.add(
                        Status::Warn,
                        format!("không thấy Claude Code: gỡ tay bằng `{line}` trong thư mục vault"),
                    )?,
                }
            }
        }

        log.step("agent", "Cài đặt của chat trong Obsidian")?;
        self.remove_agent(log, &m)?;

        log.step("theme", "Theme")?;
        self.remove_theme(log, &m)?;

        log.step("plugins", "Plugin")?;
        for pid in m.list("plugins") {
            let dir = self.obs.join("plugins").join(&pid);
            let names: Vec<String> = self
                .preset
                .plugins
                .iter()
                .find(|p| p.id == pid)
                .map(|p| p.files.iter().map(|(n, _)| n.clone()).collect())
                .unwrap_or_else(|| PLUGIN_FILES.iter().map(|s| (*s).to_owned()).collect());
            log.add(self.act(), format!("gỡ plugin {pid}"))?;
            if self.apply {
                for n in &names {
                    let _ = std::fs::remove_file(dir.join(n));
                }
                let rest = files::remove_dir_if_empty(&dir);
                if !rest.is_empty() {
                    log.add(
                        Status::Info,
                        format!("giữ .obsidian/plugins/{pid}/ ({})", rest.join(", ")),
                    )?;
                }
            }
        }
        let community = m.list("community");
        if !community.is_empty() {
            log.add(
                self.act(),
                format!("community-plugins.json: bỏ {}", community.join(", ")),
            )?;
            let cp = self.community_path();
            if self.apply && cp.exists() {
                let mut list = read_array(&cp)?;
                list.retain(|v| !v.as_str().is_some_and(|s| community.iter().any(|c| c == s)));
                write_json(&cp, &Value::Array(list))?;
            }
        }
        let (core, core_off) = (m.list("core"), m.list("core_off"));
        if !core.is_empty() || !core_off.is_empty() {
            log.add(
                self.act(),
                format!(
                    "core-plugins.json: trả {} về như trước",
                    [core.clone(), core_off.clone()].concat().join(", ")
                ),
            )?;
            let path = self.obs.join("core-plugins.json");
            if self.apply && path.exists() {
                let mut v = read_json(&path)?.unwrap_or_else(|| Value::Object(Map::new()));
                match &mut v {
                    Value::Object(o) => {
                        for c in &core {
                            o.remove(c);
                        }
                        for c in &core_off {
                            o.insert(c.clone(), Value::Bool(false));
                        }
                    }
                    Value::Array(a) => {
                        a.retain(|x| !x.as_str().is_some_and(|s| core.iter().any(|c| c == s)))
                    }
                    _ => {}
                }
                write_json(&path, &v)?;
            }
        }
        log.step("done", "Xong")?;
        if self.apply {
            std::fs::remove_file(Manifest::path(&self.vault))
                .map_err(|e| format!("xoá {}: {e}", files::MANIFEST))?;
            log.add(Status::Done, format!("xoá {}", files::MANIFEST))?;
            for dir in [self.vault.join("_E11"), self.obs.join("snippets")] {
                files::remove_dir_if_empty(&dir);
            }
        }
        log.add(
            Status::Info,
            "note không đổi: đề xuất trong _Đề xuất/ giữ nguyên (xoá tay nếu không cần); index, gói OHS và adapter \
             ACP ở thư mục của pkm giữ nguyên",
        )
    }

    fn remove_agent(&self, log: &mut Log, m: &Manifest) -> Result<(), String> {
        if let Some(st) = m.get("agent_settings").and_then(Value::as_object) {
            let path = self
                .obs
                .join("plugins")
                .join("agent-client")
                .join("data.json");
            let created = st.get("created").and_then(Value::as_bool).unwrap_or(false);
            let command = st.get("command").and_then(Value::as_str).unwrap_or("");
            let old = st.get("old").and_then(Value::as_str).unwrap_or("");
            if path.exists() {
                log.add(
                    self.act(),
                    "Agent Client: trả preset Claude Code về như trước",
                )?;
                if self.apply {
                    if created {
                        std::fs::remove_file(&path)
                            .map_err(|e| format!("xoá {}: {e}", path.display()))?;
                    } else if let Some(mut data) = read_json(&path)? {
                        let preset = data
                            .get_mut("presetAgents")
                            .and_then(|a| a.get_mut(&self.preset.agent_preset))
                            .and_then(Value::as_object_mut);
                        if let Some(p) = preset
                            && p.get("command").and_then(Value::as_str) == Some(command)
                        {
                            p.insert("command".to_owned(), Value::String(old.to_owned()));
                            write_json(&path, &data)?;
                        }
                    }
                }
            }
        }
        let allow = m.list("agent_allow");
        let path = self.vault.join(".claude").join("settings.local.json");
        if !allow.is_empty() && path.exists() {
            log.add(
                self.act(),
                format!(".claude/settings.local.json: bỏ {}", allow.join(", ")),
            )?;
            if self.apply
                && let Some(mut s) = read_json(&path)?
            {
                if let Some(list) = s
                    .get_mut("permissions")
                    .and_then(|p| p.get_mut("allow"))
                    .and_then(Value::as_array_mut)
                {
                    list.retain(|v| !v.as_str().is_some_and(|x| allow.iter().any(|a| a == x)));
                }
                write_json(&path, &s)?;
            }
        }
        Ok(())
    }

    fn remove_theme(&self, log: &mut Log, m: &Manifest) -> Result<(), String> {
        let path = self.obs.join("appearance.json");
        let snippets = m.list("snippets_on");
        let font = m.get("font_size").cloned();
        if (!snippets.is_empty() || font.is_some()) && path.exists() {
            log.add(
                self.act(),
                "appearance.json: tắt snippet đã bật, trả cỡ chữ về như trước",
            )?;
            if self.apply
                && let Some(Value::Object(mut cfg)) = read_json(&path)?
            {
                if let Some(list) = cfg
                    .get_mut("enabledCssSnippets")
                    .and_then(Value::as_array_mut)
                {
                    list.retain(|v| !v.as_str().is_some_and(|x| snippets.iter().any(|s| s == x)));
                }
                // Chỉ trả lại khi cỡ chữ vẫn là cỡ setup đặt (người dùng đổi sau đó thì giữ).
                if let Some(old) = font
                    && cfg.get("baseFontSize") == Some(&Value::from(self.preset.base_font_size))
                {
                    match old {
                        Value::Null => {
                            cfg.remove("baseFontSize");
                        }
                        v => {
                            cfg.insert("baseFontSize".to_owned(), v);
                        }
                    }
                }
                write_json(&path, &Value::Object(cfg))?;
            }
        }
        let off = m.list("plugins_off");
        if !off.is_empty() {
            log.add(self.act(), format!("bật lại {}", off.join(", ")))?;
            let cp = self.community_path();
            if self.apply {
                let mut list = read_array(&cp)?;
                for id in &off {
                    if !list.iter().any(|v| v.as_str() == Some(id.as_str())) {
                        list.push(Value::String(id.clone()));
                    }
                }
                write_json(&cp, &Value::Array(list))?;
            }
        }
        Ok(())
    }
}

/// Mảng JSON trong `path` (rỗng khi chưa có file).
fn read_array(path: &Path) -> Result<Vec<Value>, String> {
    match read_json(path)? {
        None => Ok(Vec::new()),
        Some(Value::Array(a)) => Ok(a),
        Some(_) => Err(format!("{} không phải mảng JSON", path.display())),
    }
}

/// sha256 của `name` trong `checksums.txt` (dạng `sha256sum`: `<hex>  <tên>` hoặc `<hex> *<tên>`), đúng một dòng.
pub fn checksum_for(sums: &str, name: &str) -> Result<String, String> {
    let hits: Vec<&str> = sums
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let (sha, file) = (it.next()?, it.next()?);
            (file.trim_start_matches('*') == name).then_some(sha)
        })
        .collect();
    match hits.as_slice() {
        [sha] if sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Ok(sha.to_ascii_lowercase())
        }
        [_] => Err(format!("checksums.txt: dòng của {name} không phải sha256")),
        [] => Err(format!(
            "checksums.txt không có {name}: Release thiếu plugin"
        )),
        _ => Err(format!("checksums.txt có nhiều dòng cho {name}")),
    }
}

/// Đọc [`PLUGIN_FILES`] từ zip plugin (phẳng, không thư mục).
pub fn unzip_plugin(bytes: &[u8]) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let mut zip = zip::ZipArchive::new(io::Cursor::new(bytes))
        .map_err(|e| format!("{PLUGIN_ASSET} không phải zip: {e}"))?;
    PLUGIN_FILES
        .iter()
        .map(|name| {
            let f = zip
                .by_name(name)
                .map_err(|e| format!("{PLUGIN_ASSET} thiếu {name}: {e}"))?;
            let mut out = Vec::new();
            f.take(MAX_PLUGIN_FILE + 1)
                .read_to_end(&mut out)
                .map_err(|e| format!("{PLUGIN_ASSET}: đọc {name}: {e}"))?;
            if out.len() as u64 > MAX_PLUGIN_FILE {
                return Err(format!("{PLUGIN_ASSET}: {name} quá lớn"));
            }
            Ok((*name, out))
        })
        .collect()
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("chmod {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests;
