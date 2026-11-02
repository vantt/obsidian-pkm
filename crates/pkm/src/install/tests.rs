use super::*;
use std::cell::{Cell, RefCell};
use std::fs;

use serde_json::json;

/// Hooks giả: ghi lại lời gọi, không chạy tiến trình nào, không đụng mạng.
struct Fake {
    running: io::Result<bool>,
    build: Result<(), String>,
    builds: RefCell<Vec<(PathBuf, PathBuf, Vec<String>)>>,
    handshakes: Cell<u32>,
    /// Chạy trong lúc "dựng index" (giả lập app khác sửa cấu hình).
    during_build: Option<Box<dyn Fn()>>,
    /// Claude Desktop đang chạy khi hỏi lại sau khi dựng index.
    running_after_build: Option<bool>,
    built: Cell<bool>,
    /// Nội dung trả cho từng URL khi tải gói OHS.
    served: Vec<(String, Vec<u8>)>,
}

impl Fake {
    fn new() -> Self {
        Self {
            running: Ok(false),
            build: Ok(()),
            builds: RefCell::default(),
            handshakes: Cell::new(0),
            during_build: None,
            running_after_build: None,
            built: Cell::new(false),
            served: Vec::new(),
        }
    }
}

impl Hooks for Fake {
    fn build_index(
        &self,
        _exe: &Path,
        vault: &Path,
        db: &Path,
        read: &[String],
    ) -> Result<(), String> {
        self.builds
            .borrow_mut()
            .push((vault.to_path_buf(), db.to_path_buf(), read.to_vec()));
        if let Some(f) = &self.during_build {
            f();
        }
        self.built.set(true);
        self.build.clone()
    }

    fn handshake(&self, entry: &ServerEntry, _timeout: Duration) -> Result<Handshake, String> {
        self.handshakes.set(self.handshakes.get() + 1);
        Ok(Handshake {
            server_name: "kg".into(),
            server_version: "test".into(),
            tools: vec![format!("{}-tool", entry.args.len())],
        })
    }

    fn desktop_process(&self) -> io::Result<Option<String>> {
        let found = |b: bool| b.then(|| DESKTOP_EXE.to_owned());
        if let Some(b) = self.running_after_build.filter(|_| self.built.get()) {
            return Ok(found(b));
        }
        match &self.running {
            Ok(b) => Ok(found(*b)),
            Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
        }
    }

    fn fetch(&self, url: &str, out: &Path) -> Result<(), String> {
        let (_, body) = self
            .served
            .iter()
            .find(|(u, _)| u == url)
            .ok_or_else(|| format!("404 {url}"))?;
        fs::write(out, body).map_err(|e| e.to_string())
    }
}

/// Đường dẫn Claude Desktop mà [`Fake`] báo khi app "đang chạy".
const DESKTOP_EXE: &str = r"C:\Users\u\AppData\Local\AnthropicClaude\app-1.0.0\claude.exe";

struct Setup {
    _tmp: tempfile::TempDir,
    inst: Install,
    vault: PathBuf,
    config: PathBuf,
}

/// Thư mục tạm có vault, thư mục pkm, và file cấu hình với `text` (CRLF tuỳ chọn).
fn setup(text: Option<&str>) -> Setup {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("Kho ghi chú");
    fs::create_dir_all(&vault).unwrap();
    let claude = tmp.path().join("Claude");
    fs::create_dir_all(&claude).unwrap();
    let config = claude.join(config::CONFIG_FILE);
    if let Some(text) = text {
        fs::write(&config, text).unwrap();
    }
    let bin = tmp.path().join("bin");
    let inst = Install {
        pkm_dir: tmp.path().join("pkm"),
        exe: bin.join("pkm"),
        bundle: None,
        bin_dirs: vec![bin],
        ohs_source: None,
    };
    Setup {
        _tmp: tmp,
        inst,
        vault,
        config,
    }
}

impl Setup {
    fn args(&self) -> InstallArgs {
        InstallArgs {
            vault: Some(self.vault.clone()),
            config: Some(self.config.clone()),
            no_ohs: true,
            timeout: 5,
            ..InstallArgs::default()
        }
    }

    fn run(&self, args: &InstallArgs, hooks: &Fake) -> Result<String, String> {
        let mut out = Vec::new();
        run_with(&self.inst, args, hooks, &mut out)?;
        Ok(String::from_utf8(out).unwrap())
    }

    fn read(&self) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap()
    }

    /// Các file trong thư mục cấu hình (để thấy bản sao lưu, file tạm còn sót).
    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.config.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

const EXISTING: &str = r#"{
  "globalShortcut": "Ctrl+Space",
  "mcpServers": {
    "ohs": {
      "command": "C:\\tools\\ohs\\node.exe",
      "args": ["cli.js", "serve"],
      "env": {"OBSIDIAN_VAULT_PATH": "C:\\Ghi chú"}
    },
    "kg": {"command": "python.exe", "args": ["old"]},
    "other": {"command": "x"}
  },
  "preferences": {"theme": "dark"}
}"#;

#[test]
fn install_writes_kg_and_keeps_every_other_entry() {
    let s = setup(Some(EXISTING));
    let hooks = Fake::new();
    let mut args = s.args();
    args.adapter = Some("e11".into());
    args.edge_keys = Some("links".into());
    let out = s.run(&args, &hooks).unwrap();
    let got = s.read();
    let before: serde_json::Value = serde_json::from_str(EXISTING).unwrap();
    assert_eq!(got["globalShortcut"], before["globalShortcut"]);
    assert_eq!(got["preferences"], before["preferences"]);
    assert_eq!(got["mcpServers"]["ohs"], before["mcpServers"]["ohs"]);
    assert_eq!(got["mcpServers"]["other"], before["mcpServers"]["other"]);
    let keys: Vec<_> = got["mcpServers"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, ["ohs", "kg", "other"], "kg giữ chỗ cũ");
    let db = s.inst.default_db(&s.vault);
    assert_eq!(
        got["mcpServers"]["kg"],
        json!({
            "command": s.inst.exe.to_str().unwrap(),
            "args": [s.vault.to_str().unwrap(), "mcp", "--db", db.to_str().unwrap(),
                     "--edge-keys", "links", "--adapter", "e11"],
            "env": {"PKM_NO_OHS": "1"},
        })
    );
    assert_eq!(
        *hooks.builds.borrow(),
        [(
            s.vault.clone(),
            db.clone(),
            ["--edge-keys", "links", "--adapter", "e11"]
                .map(String::from)
                .to_vec()
        )]
    );
    assert!(db.parent().unwrap().is_dir(), "thư mục index được tạo");
    assert_eq!(hooks.handshakes.get(), 1);
    assert!(out.contains("servers: ohs, kg, other"), "{out}");
    let files = s.files();
    assert_eq!(
        files.len(),
        2,
        "file cấu hình và một bản sao lưu: {files:?}"
    );
    assert!(
        files[1].starts_with("claude_desktop_config.json.bak-"),
        "{files:?}"
    );
    assert_eq!(
        fs::read_to_string(s.config.with_file_name(&files[1])).unwrap(),
        EXISTING,
        "bản sao lưu là file cũ nguyên vẹn"
    );
}

#[test]
fn install_keeps_crlf_bom_free_utf8_and_non_ascii() {
    let s = setup(Some(&format!("\u{feff}{}", EXISTING.replace('\n', "\r\n"))));
    s.run(&s.args(), &Fake::new()).unwrap();
    let text = fs::read_to_string(&s.config).unwrap();
    assert!(
        text.contains("\r\n") && !text.replace("\r\n", "").contains('\n'),
        "CRLF giữ nguyên"
    );
    assert!(
        text.contains("C:\\\\Ghi chú"),
        "không thoát ký tự tiếng Việt"
    );
    assert!(text.contains("Kho ghi chú"));
    assert!(!text.starts_with('\u{feff}'));
}

#[test]
fn install_creates_missing_config_without_backup() {
    let s = setup(None);
    s.run(&s.args(), &Fake::new()).unwrap();
    let got = s.read();
    assert_eq!(got["mcpServers"].as_object().unwrap().len(), 1);
    assert_eq!(s.files(), [config::CONFIG_FILE]);
}

#[test]
fn dry_run_writes_nothing_and_builds_nothing() {
    let s = setup(Some(EXISTING));
    let hooks = Fake {
        running: Ok(true),
        ..Fake::new()
    };
    let mut args = s.args();
    args.dry_run = true;
    let out = s.run(&args, &hooks).unwrap();
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);
    assert_eq!(s.files(), [config::CONFIG_FILE]);
    assert!(hooks.builds.borrow().is_empty());
    assert_eq!(hooks.handshakes.get(), 0);
    assert!(!s.inst.pkm_dir.exists(), "không tạo thư mục dữ liệu");
    assert!(out.contains("[dry-run]"), "{out}");
    assert!(out.contains("-   \"kg\": {"), "{out}");
    assert!(out.contains("+       \"mcp\","), "{out}");
}

#[test]
fn remove_deletes_only_kg() {
    let s = setup(Some(EXISTING));
    let mut args = s.args();
    args.vault = None;
    args.remove = true;
    let hooks = Fake::new();
    s.run(&args, &hooks).unwrap();
    let got = s.read();
    let mut want: serde_json::Value = serde_json::from_str(EXISTING).unwrap();
    want["mcpServers"]
        .as_object_mut()
        .unwrap()
        .shift_remove("kg");
    assert_eq!(got, want);
    let keys: Vec<_> = got.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["globalShortcut", "mcpServers", "preferences"]);
    assert!(hooks.builds.borrow().is_empty());
}

#[test]
fn remove_dry_run_and_remove_without_kg_write_nothing() {
    let s = setup(Some(EXISTING));
    let mut args = s.args();
    args.remove = true;
    args.dry_run = true;
    let out = s.run(&args, &Fake::new()).unwrap();
    assert!(out.contains("Sẽ xoá"), "{out}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);

    let s = setup(Some(r#"{"mcpServers": {"ohs": {"command": "x"}}}"#));
    let mut args = s.args();
    args.remove = true;
    let out = s.run(&args, &Fake::new()).unwrap();
    assert!(out.contains("không đổi gì"), "{out}");
    assert_eq!(s.files(), [config::CONFIG_FILE]);
}

#[test]
fn refuses_to_write_while_desktop_runs() {
    let s = setup(Some(EXISTING));
    let hooks = Fake {
        running: Ok(true),
        ..Fake::new()
    };
    let err = s.run(&s.args(), &hooks).unwrap_err();
    assert!(err.contains("Claude Desktop đang chạy"), "{err}");
    assert!(
        err.contains(DESKTOP_EXE),
        "in exe bị coi là Claude Desktop: {err}"
    );
    assert!(err.contains("--force"), "{err}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);
    assert!(hooks.builds.borrow().is_empty());

    // Chấp nhận chỗ đặt binary không bỏ qua kiểm Claude Desktop.
    let mut args = s.args();
    args.allow_any_location = true;
    let err = s.run(&args, &hooks).unwrap_err();
    assert!(err.contains("Claude Desktop đang chạy"), "{err}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);

    let mut args = s.args();
    args.vault = None;
    args.remove = true;
    let err = s.run(&args, &hooks).unwrap_err();
    assert!(err.contains("Claude Desktop đang chạy"), "{err}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);

    let mut args = s.args();
    args.force = true;
    s.run(&args, &hooks).unwrap();
    assert_ne!(fs::read_to_string(&s.config).unwrap(), EXISTING);
}

#[test]
fn refuses_when_process_probe_fails() {
    let s = setup(Some(EXISTING));
    let hooks = Fake {
        running: Err(io::Error::new(io::ErrorKind::NotFound, "no pgrep")),
        ..Fake::new()
    };
    let err = s.run(&s.args(), &hooks).unwrap_err();
    assert!(err.contains("--force"), "{err}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);
}

#[test]
fn failed_index_build_leaves_config_unchanged() {
    let s = setup(Some(EXISTING));
    let hooks = Fake {
        build: Err("dựng index lỗi (exit status: 2)".into()),
        ..Fake::new()
    };
    let err = s.run(&s.args(), &hooks).unwrap_err();
    assert!(err.contains("cấu hình Claude Desktop chưa đổi"), "{err}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);
    assert_eq!(hooks.handshakes.get(), 0);
}

#[test]
fn invalid_json_config_is_never_overwritten() {
    let s = setup(Some("{ not json"));
    let hooks = Fake::new();
    let err = s.run(&s.args(), &hooks).unwrap_err();
    assert!(err.contains("không phải JSON hợp lệ"), "{err}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), "{ not json");
    assert!(hooks.builds.borrow().is_empty());
}

#[test]
fn second_install_with_same_entry_does_not_rewrite() {
    let s = setup(Some(EXISTING));
    s.run(&s.args(), &Fake::new()).unwrap();
    let files = s.files();
    let out = s.run(&s.args(), &Fake::new()).unwrap();
    assert!(out.contains("đã đúng, không ghi lại"), "{out}");
    assert_eq!(s.files(), files);
}

#[test]
fn check_handshakes_with_configured_entry_and_changes_nothing() {
    let s = setup(Some(EXISTING));
    let mut args = s.args();
    args.vault = None;
    args.check = true;
    let hooks = Fake::new();
    let out = s.run(&args, &hooks).unwrap();
    assert_eq!(hooks.handshakes.get(), 1);
    assert!(out.starts_with("python.exe: kg test, 1 tool"), "{out}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);

    let s = setup(Some("{}"));
    let err = s
        .run(&args_with_config(&s, true), &Fake::new())
        .unwrap_err();
    assert!(err.contains("chưa có `kg`"), "{err}");
}

fn args_with_config(s: &Setup, check: bool) -> InstallArgs {
    InstallArgs {
        config: Some(s.config.clone()),
        check,
        ..InstallArgs::default()
    }
}

#[test]
fn missing_vault_is_an_error() {
    let s = setup(Some(EXISTING));
    let mut args = s.args();
    args.vault = Some(s.vault.join("không có"));
    let err = s.run(&args, &Fake::new()).unwrap_err();
    assert!(err.contains("không phải thư mục vault"), "{err}");
}

#[test]
fn ohs_package_file_needs_a_pinned_bundle_and_stops_install() {
    let s = setup(Some(EXISTING));
    let file = s.vault.join("bundle.zip");
    fs::write(&file, "x").unwrap();
    let mut args = s.args();
    args.no_ohs = false;
    args.ohs_package = Some(file);
    let hooks = Fake::new();
    let err = s.run(&args, &hooks).unwrap_err();
    assert!(err.contains("fork chưa có gói OHS"), "{err}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);
    assert!(hooks.builds.borrow().is_empty());
}

#[test]
fn ohs_already_installed_is_kept() {
    let s = setup(Some(EXISTING));
    let dir = s.inst.ohs_dir();
    let node = if cfg!(windows) {
        "node/node.exe"
    } else {
        "node/bin/node"
    };
    fs::create_dir_all(dir.join(node).parent().unwrap()).unwrap();
    fs::write(dir.join(node), "").unwrap();
    fs::create_dir_all(dir.join("app/dist/src")).unwrap();
    fs::write(dir.join("app/dist/src/cli.js"), "").unwrap();
    let mut args = s.args();
    args.no_ohs = false;
    let out = s.run(&args, &Fake::new()).unwrap();
    assert!(out.contains("gói OHS đã có"), "{out}");
}

#[test]
fn entry_paths_reads_back_vault_and_db() {
    let e = server_entry(
        Path::new("/bin/pkm"),
        Path::new("/v"),
        Path::new("/d.db"),
        &["--packs".into(), "capture".into()],
        Some("Kho"),
    )
    .unwrap();
    assert_eq!(
        e.args,
        [
            "/v", "mcp", "--db", "/d.db", "--packs", "capture", "--about", "Kho"
        ]
    );
    assert_eq!(
        entry_paths(&e.args),
        (Some(PathBuf::from("/v")), Some(PathBuf::from("/d.db")))
    );
    assert_eq!(entry_paths(&["--db".into()]), (None, None));
}

#[test]
fn default_db_is_outside_vault_and_stable() {
    let s = setup(None);
    let a = s.inst.default_db(&s.vault);
    assert_eq!(a, s.inst.default_db(&s.vault));
    assert!(a.starts_with(s.inst.pkm_dir.join("kg")));
    let name = a.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("Kho-ghi-chú-") && name.ends_with(".db"),
        "{name}"
    );
    assert_ne!(a, s.inst.default_db(&s.vault.with_file_name("khác")));
}

#[test]
fn default_db_ignores_trailing_separator_and_symlinks() {
    let s = setup(None);
    let a = s.inst.default_db(&s.vault);
    let mut slash = s.vault.clone().into_os_string();
    slash.push(std::path::MAIN_SEPARATOR_STR);
    assert_eq!(s.inst.default_db(Path::new(&slash)), a);
    #[cfg(unix)]
    {
        let link = s.vault.with_file_name("lối tắt");
        std::os::unix::fs::symlink(&s.vault, &link).unwrap();
        assert_eq!(s.inst.default_db(&link), a);
    }
}

#[test]
fn default_db_keeps_index_named_by_old_hash() {
    let s = setup(None);
    let mut slash = s.vault.clone().into_os_string();
    slash.push(std::path::MAIN_SEPARATOR_STR);
    let slash = PathBuf::from(slash);
    let old = s.inst.legacy_default_db(&slash);
    assert_ne!(old, s.inst.default_db(&slash), "khoá mới khác khoá cũ");
    fs::create_dir_all(old.parent().unwrap()).unwrap();
    fs::write(&old, b"SQLite format 3\0").unwrap();
    assert_eq!(s.inst.default_db(&slash), old);
}

#[test]
fn vault_key_normalizes_spelling_of_one_path() {
    assert_eq!(vault_key("/home/u/Kho/", false), "/home/u/Kho");
    assert_eq!(vault_key("/", false), "/");
    assert_eq!(vault_key(r"C:\", true), r"c:\");
    assert_eq!(vault_key(r"\\?\D:\Vault\", true), r"d:\vault");
    assert_eq!(
        vault_key(r"\\?\UNC\server\share\Vault", false),
        r"\\server\share\Vault"
    );
    // NFD (macOS) và NFC cho cùng khoá; gộp hoa thường chỉ khi được yêu cầu.
    let nfd = "/Users/u/Ghi chu\u{301}";
    assert_eq!(vault_key(nfd, false), "/Users/u/Ghi chú");
    assert_eq!(vault_key("/Users/u/GHI CHÚ", true), vault_key(nfd, true));
    assert_ne!(vault_key("/a/Kho", false), vault_key("/a/kho", false));
}

#[test]
fn read_args_follow_python_order() {
    let args = InstallArgs {
        packs: Some("capture".into()),
        no_aliases: true,
        edge_keys: Some("links".into()),
        body_edges: true,
        adapter: Some("e11".into()),
        ..InstallArgs::default()
    };
    assert_eq!(
        args.read_args(),
        [
            "--packs",
            "capture",
            "--no-aliases",
            "--edge-keys",
            "links",
            "--body-edges",
            "--adapter",
            "e11"
        ]
    );
}

#[test]
fn sqlite_header_accepts_only_sqlite_files() {
    let tmp = tempfile::tempdir().unwrap();
    let good = tmp.path().join("a.db");
    fs::write(&good, b"SQLite format 3\0rest").unwrap();
    assert!(sqlite_header(&good).is_ok());
    let bad = tmp.path().join("b.db");
    fs::write(&bad, b"not a database file").unwrap();
    assert!(
        sqlite_header(&bad)
            .unwrap_err()
            .contains("không phải file SQLite")
    );
    assert!(sqlite_header(&tmp.path().join("c.db")).is_err());
}

#[test]
fn long_paths_value_is_parsed_from_reg_output() {
    let on = "\r\nHKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\FileSystem\r\n    LongPathsEnabled    REG_DWORD    0x1\r\n";
    assert_eq!(parse_long_paths(on), Some(true));
    assert_eq!(parse_long_paths(&on.replace("0x1", "0x0")), Some(false));
    assert_eq!(parse_long_paths("nothing"), None);
}

#[test]
fn install_args_parse_from_command_line() {
    use clap::Parser;
    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        args: InstallArgs,
    }
    let cli = Cli::try_parse_from(["install", "--remove"]).unwrap();
    assert!(cli.args.remove && cli.args.vault.is_none());
    assert!(
        Cli::try_parse_from(["install"]).is_err(),
        "VAULT bắt buộc khi cài"
    );
    assert!(Cli::try_parse_from(["install", "--check", "--remove"]).is_err());
    let cli = Cli::try_parse_from(["install", "/v", "--dry-run", "--edge-keys", "links"]).unwrap();
    assert_eq!(cli.args.timeout, 120);
    assert!(Cli::try_parse_from(["install", "/v", "--edge-keys", "x"]).is_err());
}

/// Gói OHS giả dạng `.tar.gz` (thư mục gốc `ohs-x/` có Node và `cli.js`).
fn ohs_tar_gz() -> Vec<u8> {
    let node = if cfg!(windows) {
        "node/node.exe"
    } else {
        "node/bin/node"
    };
    let mut b = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    for rel in [node, "app/dist/src/cli.js"] {
        let mut h = tar::Header::new_gnu();
        h.set_size(1);
        h.set_mode(0o755);
        h.set_cksum();
        b.append_data(&mut h, format!("ohs-x/{rel}"), &b"x"[..])
            .unwrap();
    }
    b.into_inner().unwrap().finish().unwrap()
}

#[test]
fn refuses_binary_outside_install_dir_without_allow_any_location() {
    let mut s = setup(Some(EXISTING));
    s.inst.exe = s.vault.parent().unwrap().join("Downloads").join("pkm");
    let hooks = Fake::new();
    let err = s.run(&s.args(), &hooks).unwrap_err();
    assert!(err.contains("thư mục tải về hoặc thư mục tạm"), "{err}");
    assert!(
        err.contains(concat!(
            "gh release download v",
            env!("CARGO_PKG_VERSION"),
            " --repo vantt/obsidian-pkm"
        )),
        "{err}"
    );
    assert!(
        err.contains("mcp install"),
        "lệnh chạy lại từ chỗ chuẩn: {err}"
    );
    assert!(err.contains("--allow-any-location"), "{err}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);
    assert!(hooks.builds.borrow().is_empty());

    // `--force` chỉ bỏ qua kiểm Claude Desktop, không chấp nhận chỗ đặt binary.
    let mut args = s.args();
    args.force = true;
    let err = s.run(&args, &hooks).unwrap_err();
    assert!(err.contains("--allow-any-location"), "{err}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);

    let mut args = s.args();
    args.dry_run = true;
    let out = s.run(&args, &hooks).unwrap();
    assert!(
        out.contains("cảnh báo (sẽ từ chối nếu không có --allow-any-location)"),
        "{out}"
    );

    let mut args = s.args();
    args.allow_any_location = true;
    let out = s.run(&args, &hooks).unwrap();
    assert!(out.starts_with("cảnh báo: pkm đang chạy từ"), "{out}");
    assert_eq!(
        s.read()["mcpServers"]["kg"]["command"],
        s.inst.exe.to_str().unwrap()
    );
}

#[test]
fn installer_marker_or_install_dir_makes_binary_location_stable() {
    let mut s = setup(Some(EXISTING));
    let dir = s.vault.parent().unwrap().join("tools");
    fs::create_dir_all(&dir).unwrap();
    s.inst.exe = dir.join("pkm");
    s.inst.bin_dirs = Vec::new();
    let warning = s.inst.exe_location_warning(&[]).unwrap();
    assert!(warning.contains("ngoài chỗ cài chuẩn") || warning.contains("thư mục tạm"));
    fs::write(dir.join(INSTALL_MARKER), "pkm-managed-v1\n").unwrap();
    assert_eq!(s.inst.exe_location_warning(&[]), None);
    fs::remove_file(dir.join(INSTALL_MARKER)).unwrap();
    s.inst.bin_dirs = vec![dir.join("khác"), dir.join(".")];
    assert_eq!(
        s.inst.exe_location_warning(&[]),
        None,
        "so thư mục sau khi chuẩn hoá"
    );
}

#[test]
fn rereads_config_changed_while_building_and_keeps_that_change() {
    let s = setup(Some(EXISTING));
    let config = s.config.clone();
    let hooks = Fake {
        during_build: Some(Box::new(move || {
            let mut v: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
            v["mcpServers"]["mới"] = json!({"command": "y"});
            v["preferences"]["theme"] = json!("light");
            fs::write(&config, v.to_string()).unwrap();
        })),
        ..Fake::new()
    };
    let out = s.run(&s.args(), &hooks).unwrap();
    assert!(out.contains("đã bị sửa trong lúc dựng index"), "{out}");
    let got = s.read();
    assert_eq!(got["mcpServers"]["mới"], json!({"command": "y"}));
    assert_eq!(got["preferences"]["theme"], "light");
    assert_eq!(got["mcpServers"]["kg"]["args"][1], "mcp");
    assert!(out.contains("servers: ohs, kg, other, mới"), "{out}");
}

#[test]
fn refuses_to_write_when_desktop_starts_during_build() {
    let s = setup(Some(EXISTING));
    let hooks = Fake {
        running_after_build: Some(true),
        ..Fake::new()
    };
    let err = s.run(&s.args(), &hooks).unwrap_err();
    assert!(err.contains("Claude Desktop đang chạy"), "{err}");
    assert!(err.contains("Index đã dựng xong"), "{err}");
    assert_eq!(hooks.builds.borrow().len(), 1);
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);
    assert_eq!(hooks.handshakes.get(), 0);
}

#[test]
fn no_ohs_is_recorded_in_the_entry_and_dropped_on_a_later_install() {
    let mut s = setup(Some(EXISTING));
    s.run(&s.args(), &Fake::new()).unwrap();
    assert_eq!(
        s.read()["mcpServers"]["kg"]["env"],
        json!({"PKM_NO_OHS": "1"})
    );
    // Lần cài sau có OHS (nguồn không tải được: chỉ cảnh báo) thì bỏ dấu `--no-ohs`.
    s.inst.ohs_source = Some("https://example.test/ohs.tar.gz".into());
    let mut args = s.args();
    args.no_ohs = false;
    let out = s.run(&args, &Fake::new()).unwrap();
    assert!(
        out.contains("cảnh báo: chưa cài được gói OHS từ PKM_OHS_PACKAGE="),
        "{out}"
    );
    assert!(out.contains("chỉ `related` khi không có `hits`"), "{out}");
    assert!(s.read()["mcpServers"]["kg"].get("env").is_none());
}

#[test]
fn ohs_source_env_installs_package_from_https_url() {
    let mut s = setup(Some(EXISTING));
    let url = "https://example.test/ohs-linux-x64.tar.gz";
    s.inst.ohs_source = Some(url.into());
    let archive = ohs_tar_gz();
    let file = s.vault.parent().unwrap().join("a.tgz");
    fs::write(&file, &archive).unwrap();
    let sha = ohs_bundle::sha256_file(&file).unwrap();
    let hooks = Fake {
        served: vec![
            (url.to_owned(), archive),
            (
                format!("{url}.sha256"),
                format!("{sha}  ohs.tar.gz\n").into_bytes(),
            ),
        ],
        ..Fake::new()
    };
    let mut args = s.args();
    args.no_ohs = false;
    args.dry_run = true;
    let plan = s.run(&args, &hooks).unwrap();
    assert!(
        plan.contains(&format!("cài gói OHS từ {url} (PKM_OHS_PACKAGE)")),
        "{plan}"
    );
    args.dry_run = false;
    let out = s.run(&args, &hooks).unwrap();
    assert!(out.contains("đã cài gói OHS vào"), "{out}");
    assert!(OhsPackage::from_dir(&s.inst.ohs_dir()).is_ok());
}

#[test]
fn ohs_is_downloaded_from_the_pinned_release_without_login() {
    let mut s = setup(Some(EXISTING));
    let archive = ohs_tar_gz();
    let file = s.vault.parent().unwrap().join("a.tgz");
    fs::write(&file, &archive).unwrap();
    let sha = ohs_bundle::sha256_file(&file).unwrap();
    let bundle: &'static Bundle = Box::leak(Box::new(Bundle {
        platform: "linux-x64",
        sha256: Box::leak(sha.into_boxed_str()),
    }));
    s.inst.bundle = Some(bundle);
    let hooks = Fake {
        served: vec![(bundle.url(), archive)],
        ..Fake::new()
    };
    let mut args = s.args();
    args.no_ohs = false;
    args.dry_run = true;
    let plan = s.run(&args, &hooks).unwrap();
    assert!(
        plan.contains(&bundle.url()) && plan.contains("không cần đăng nhập"),
        "{plan}"
    );
    args.dry_run = false;
    // Tải hỏng (404) chỉ cảnh báo, nêu nguồn là bản phát hành ghim.
    let out = s.run(&args, &Fake::new()).unwrap();
    assert!(
        out.contains("cảnh báo: chưa cài được gói OHS từ bản phát hành v0.17.2-vantt.3"),
        "{out}"
    );
    let out = s.run(&args, &hooks).unwrap();
    assert!(out.contains("đã cài gói OHS vào"), "{out}");
    assert!(OhsPackage::from_dir(&s.inst.ohs_dir()).is_ok());
}

#[test]
fn ohs_package_flag_rejects_plain_http() {
    let s = setup(Some(EXISTING));
    let mut args = s.args();
    args.no_ohs = false;
    args.ohs_package = Some(PathBuf::from("http://example.test/ohs.tar.gz"));
    let err = s.run(&args, &Fake::new()).unwrap_err();
    assert!(err.contains("https://"), "{err}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);
}

#[test]
fn reinstall_and_rebuild_args_come_from_the_entry() {
    let e = server_entry(
        Path::new("/bin/pkm"),
        Path::new("/v"),
        Path::new("/d.db"),
        &["--packs".into(), "capture".into()],
        Some("Kho"),
    )
    .unwrap();
    assert_eq!(
        reinstall_args(&e.args, true).unwrap(),
        [
            "mcp", "install", "/v", "--db", "/d.db", "--packs", "capture", "--about", "Kho",
            "--no-ohs"
        ]
    );
    assert_eq!(
        rebuild_args(&e.args).unwrap(),
        ["/v", "build", "--db", "/d.db", "--packs", "capture"]
    );
    assert_eq!(reinstall_args(&["/v".into(), "serve".into()], false), None);
    assert_eq!(rebuild_args(&["--db".into()]), None);
}
