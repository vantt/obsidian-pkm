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
    /// Vault đã làm ấm index OHS, và trạng thái trả về.
    warmed: RefCell<Vec<PathBuf>>,
    warm_state: OhsState,
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
            warmed: RefCell::default(),
            warm_state: OhsState::Ready,
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
            server_name: "pkm".into(),
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

    fn warm_ohs(
        &self,
        _pkg: OhsPackage,
        vault: &Path,
        _timeout: Duration,
        out: &mut dyn Write,
    ) -> OhsState {
        self.warmed.borrow_mut().push(vault.to_path_buf());
        let _ = writeln!(out, "  OHS đang index: 1/2 note");
        self.warm_state.clone()
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
    "pkm": {"command": "python.exe", "args": ["old"]},
    "other": {"command": "x"}
  },
  "preferences": {"theme": "dark"}
}"#;

#[test]
fn install_writes_pkm_and_keeps_every_other_entry() {
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
    assert_eq!(keys, ["ohs", "pkm", "other"], "pkm giữ chỗ cũ");
    let db = s.inst.default_db(&s.vault);
    assert_eq!(
        got["mcpServers"]["pkm"],
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
    assert!(out.contains("servers: ohs, pkm, other"), "{out}");
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
    assert!(out.contains("-   \"pkm\": {"), "{out}");
    assert!(out.contains("+       \"mcp\","), "{out}");
}

#[test]
fn remove_deletes_only_pkm() {
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
        .shift_remove("pkm");
    assert_eq!(got, want);
    let keys: Vec<_> = got.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["globalShortcut", "mcpServers", "preferences"]);
    assert!(hooks.builds.borrow().is_empty());
}

#[test]
fn remove_dry_run_and_remove_without_pkm_write_nothing() {
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
    assert!(out.starts_with("python.exe: pkm test, 1 tool"), "{out}");
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);

    let s = setup(Some("{}"));
    let err = s
        .run(&args_with_config(&s, true), &Fake::new())
        .unwrap_err();
    assert!(err.contains("chưa có `pkm`"), "{err}");
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
fn ohs_flag_points_pkm_at_an_existing_package_without_installing_one() {
    let s = setup(Some(EXISTING));
    let shared = s.vault.parent().unwrap().join("tools-ohs");
    let node = if cfg!(windows) {
        "node/node.exe"
    } else {
        "node/bin/node"
    };
    fs::create_dir_all(shared.join(node).parent().unwrap()).unwrap();
    fs::write(shared.join(node), "").unwrap();
    fs::create_dir_all(shared.join("app/dist/src")).unwrap();
    fs::write(shared.join("app/dist/src/cli.js"), "").unwrap();
    let manifest = r#"{"version":"0.17.2-vantt.1"}"#;
    fs::write(shared.join("MANIFEST.json"), manifest).unwrap();
    let mut args = s.args();
    args.no_ohs = false;
    args.ohs = Some(shared.clone());
    let hooks = Fake::new();
    let out = s.run(&args, &hooks).unwrap();
    assert!(
        out.contains("dùng gói OHS có sẵn") && out.contains("0.17.2-vantt.1"),
        "{out}"
    );
    // Làm ấm index bằng chính gói dùng chung.
    assert_eq!(*hooks.warmed.borrow(), std::slice::from_ref(&s.vault));
    assert!(
        !s.inst.ohs_dir().exists(),
        "không cài gói vào thư mục của pkm"
    );
    assert_eq!(
        fs::read_to_string(shared.join("MANIFEST.json")).unwrap(),
        manifest,
        "gói của người dùng giữ nguyên, kể cả khi cũ hơn bản ghim"
    );
    let entry: Vec<String> =
        serde_json::from_value(s.read()["mcpServers"]["pkm"]["args"].clone()).unwrap();
    let i = entry.iter().position(|a| a == "--ohs").unwrap();
    assert_eq!(Path::new(&entry[i + 1]), shared);
    assert_eq!(entry_ohs(&entry).as_deref(), Some(shared.as_path()));
    assert!(!rebuild_args(&entry).unwrap().contains(&"--ohs".to_owned()));
    assert!(
        reinstall_args(&entry, false)
            .unwrap()
            .contains(&"--ohs".to_owned())
    );
    args.ohs = Some(s.vault.join("không có"));
    assert!(
        s.run(&args, &Fake::new())
            .unwrap_err()
            .starts_with("--ohs:")
    );
}

#[test]
fn ohs_older_than_the_pin_is_upgraded_and_the_pin_or_newer_is_kept() {
    let mut s = setup(Some(EXISTING));
    let archive = ohs_tar_gz();
    let sha = ohs_bundle::sha256_file(&{
        let f = s.vault.parent().unwrap().join("pinned.tgz");
        fs::write(&f, &archive).unwrap();
        f
    })
    .unwrap();
    let bundle: &'static Bundle = Box::leak(Box::new(Bundle {
        platform: "linux-x64",
        sha256: Box::leak(sha.into_boxed_str()),
    }));
    s.inst.bundle = Some(bundle);
    let dir = s.inst.ohs_dir();
    let node = if cfg!(windows) {
        "node/node.exe"
    } else {
        "node/bin/node"
    };
    let old = |version: &str| {
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(node).parent().unwrap()).unwrap();
        fs::write(dir.join(node), "").unwrap();
        fs::create_dir_all(dir.join("app/dist/src")).unwrap();
        fs::write(dir.join("app/dist/src/cli.js"), "").unwrap();
        fs::write(
            dir.join("MANIFEST.json"),
            format!(r#"{{"version":"{version}"}}"#),
        )
        .unwrap();
    };
    let hooks = Fake {
        served: vec![(bundle.url(), archive)],
        ..Fake::new()
    };
    let mut args = s.args();
    args.no_ohs = false;

    old(ohs_bundle::VERSION);
    let out = s.run(&args, &hooks).unwrap();
    assert!(
        out.contains("gói OHS đã có") && out.contains(ohs_bundle::VERSION),
        "{out}"
    );

    let (core, n) = ohs_bundle::VERSION.split_once("-vantt.").unwrap();
    let newer = format!("{core}-vantt.{}", n.parse::<u64>().unwrap() + 1);
    old(&newer);
    let out = s.run(&args, &hooks).unwrap();
    assert!(
        out.contains("gói OHS đã có") && out.contains(&newer),
        "không hạ cấp gói mới hơn bản ghim: {out}"
    );
    assert!(
        fs::read_to_string(dir.join("MANIFEST.json"))
            .unwrap()
            .contains(&newer)
    );

    old("0.17.2-vantt.1");
    args.dry_run = true;
    let plan = s.run(&args, &hooks).unwrap();
    assert!(
        plan.contains(&format!("tải gói OHS {}", ohs_bundle::VERSION))
            && plan.contains(&bundle.url()),
        "{plan}"
    );
    args.dry_run = false;
    let out = s.run(&args, &hooks).unwrap();
    assert!(out.contains("đã cài gói OHS vào"), "{out}");
    assert!(!dir.join("MANIFEST.json").exists(), "gói cũ đã bị thay");
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
        s.read()["mcpServers"]["pkm"]["command"],
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
    assert_eq!(got["mcpServers"]["pkm"]["args"][1], "mcp");
    assert!(out.contains("servers: ohs, pkm, other, mới"), "{out}");
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
        s.read()["mcpServers"]["pkm"]["env"],
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
    assert!(
        out.contains("chỉ `search` và `related` không có `hits`"),
        "{out}"
    );
    assert!(s.read()["mcpServers"]["pkm"].get("env").is_none());
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
        out.contains("cảnh báo: chưa cài được gói OHS từ bản phát hành v0.17.2-vantt.4"),
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

/// Cấu hình của lần cài trước đổi tên: mục `kg` do pkm ghi, MCP `ohs` người dùng tự thêm, và mục khác.
fn two_doors(s: &Setup) -> String {
    let old = server_entry(
        &s.inst.exe,
        &s.vault,
        &s.inst.default_db(&s.vault),
        &[],
        None,
    )
    .unwrap();
    serde_json::to_string_pretty(&json!({"mcpServers": {
        "ohs": {"command": "node", "args": ["cli.js", "serve"]},
        "kg": old.to_value(),
        "other": {"command": "x"},
    }}))
    .unwrap()
}

/// Mục `kg` do pkm ghi được đổi thành `pkm` tại đúng chỗ của nó; `ohs` người dùng tự thêm giữ nguyên, kèm câu báo.
#[test]
fn install_renames_the_kg_entry_written_by_pkm_and_keeps_ohs() {
    let s = setup(None);
    fs::write(&s.config, two_doors(&s)).unwrap();
    let out = s.run(&s.args(), &Fake::new()).unwrap();
    let got = s.read();
    let keys: Vec<_> = got["mcpServers"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, ["ohs", "pkm", "other"]);
    assert_eq!(
        got["mcpServers"]["ohs"],
        json!({"command": "node", "args": ["cli.js", "serve"]})
    );
    assert_eq!(got["mcpServers"]["pkm"]["args"][1], "mcp");
    assert!(out.contains("giữ nguyên mục `ohs` bạn tự thêm"), "{out}");

    // Dry-run báo đổi tên.
    fs::write(&s.config, two_doors(&s)).unwrap();
    let mut args = s.args();
    args.dry_run = true;
    let out = s.run(&args, &Fake::new()).unwrap();
    assert!(out.contains("đổi tên mục `kg` cũ do pkm ghi"), "{out}");
}

/// Mục `kg` không do pkm ghi (bản Python chạy song song) giữ nguyên; `pkm` được thêm bên cạnh.
#[test]
fn install_keeps_a_kg_entry_not_written_by_pkm() {
    let python = r#"{"mcpServers": {"kg": {"command": "python.exe", "args": ["scripts/kg/mcp_server.py"]}}}"#;
    let s = setup(Some(python));
    s.run(&s.args(), &Fake::new()).unwrap();
    let got = s.read();
    let keys: Vec<_> = got["mcpServers"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, ["kg", "pkm"]);
    assert_eq!(got["mcpServers"]["kg"]["command"], "python.exe");
}

/// `--remove` xoá `pkm` và mục `kg` cũ do pkm ghi; không đụng `ohs`.
#[test]
fn remove_also_drops_the_old_kg_entry_written_by_pkm() {
    let s = setup(None);
    fs::write(&s.config, two_doors(&s)).unwrap();
    let mut args = s.args();
    args.vault = None;
    args.remove = true;
    s.run(&args, &Fake::new()).unwrap();
    let got = s.read();
    let keys: Vec<_> = got["mcpServers"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, ["ohs", "other"]);
}

#[test]
fn written_by_pkm_needs_the_pkm_binary_and_mcp_args() {
    let e = |command: &str, args: &[&str]| ServerEntry {
        command: command.to_owned(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        env: vec![],
    };
    assert!(written_by_pkm(&e(
        "/home/u/.local/bin/pkm",
        &["/v", "mcp", "--db", "/d"]
    )));
    assert!(written_by_pkm(&e(
        r"C:\Users\u\AppData\Local\pkm\bin\PKM.exe",
        &["C:\\v", "mcp"]
    )));
    assert!(!written_by_pkm(&e(
        "python.exe",
        &["scripts/kg/mcp_server.py"]
    )));
    assert!(!written_by_pkm(&e("/bin/pkm", &["/v", "build"])));
}

/// Gói OHS có thì làm ấm index sau khi ghi cấu hình; `--no-warm` bỏ qua; index chưa xong chỉ là cảnh báo.
#[test]
fn install_warms_the_ohs_index_unless_told_not_to() {
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
    let hooks = Fake::new();
    let out = s.run(&args, &hooks).unwrap();
    assert_eq!(*hooks.warmed.borrow(), std::slice::from_ref(&s.vault));
    assert!(out.contains("OHS đang index: 1/2 note"), "{out}");
    assert!(out.contains("index tìm kiếm sẵn sàng"), "{out}");
    assert!(
        out.trim_end()
            .ends_with("Mở lại Claude Desktop để dùng `pkm`."),
        "{out}"
    );

    let hooks = Fake {
        warm_state: OhsState::ModelMissing {
            detail: "không tải được".into(),
        },
        ..Fake::new()
    };
    let out = s.run(&args, &hooks).unwrap();
    assert!(
        out.contains("cảnh báo: index tìm kiếm chưa xong (model_missing)"),
        "{out}"
    );

    args.no_warm = true;
    let hooks = Fake::new();
    let out = s.run(&args, &hooks).unwrap();
    assert!(hooks.warmed.borrow().is_empty());
    assert!(
        out.contains("bỏ qua làm ấm index tìm kiếm (--no-warm)"),
        "{out}"
    );

    // `--no-ohs`: không làm ấm, không nhắc gì.
    let hooks = Fake::new();
    let out = s.run(&s.args(), &hooks).unwrap();
    assert!(hooks.warmed.borrow().is_empty());
    assert!(!out.contains("làm ấm"), "{out}");
}
