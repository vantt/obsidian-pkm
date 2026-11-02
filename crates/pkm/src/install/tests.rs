use super::*;
use std::cell::{Cell, RefCell};
use std::fs;

use serde_json::json;

/// Hooks giả: ghi lại lời gọi, không chạy tiến trình nào.
struct Fake {
    running: io::Result<bool>,
    build: Result<(), String>,
    builds: RefCell<Vec<(PathBuf, PathBuf, Vec<String>)>>,
    handshakes: Cell<u32>,
}

impl Fake {
    fn new() -> Self {
        Self {
            running: Ok(false),
            build: Ok(()),
            builds: RefCell::default(),
            handshakes: Cell::new(0),
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

    fn desktop_running(&self) -> io::Result<bool> {
        match &self.running {
            Ok(b) => Ok(*b),
            Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
        }
    }
}

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
    let inst = Install {
        pkm_dir: tmp.path().join("pkm"),
        exe: tmp.path().join("bin").join("pkm"),
        bundle: None,
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
    assert_eq!(fs::read_to_string(&s.config).unwrap(), EXISTING);
    assert!(hooks.builds.borrow().is_empty());

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
