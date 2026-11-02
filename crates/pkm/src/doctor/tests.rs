use super::*;
use std::cell::RefCell;
use std::fs;
use std::io;

use crate::install::handshake::Handshake;

/// Hooks giả cho `doctor`: trạng thái OHS và đường dẫn dài đặt trước, ghi lại vault OHS được chạy trên.
struct Fake {
    ohs: OhsState,
    long_paths: Option<bool>,
    ohs_runs: RefCell<Vec<PathBuf>>,
}

impl Fake {
    fn new() -> Self {
        Self {
            ohs: OhsState::Ready,
            long_paths: None,
            ohs_runs: RefCell::default(),
        }
    }
}

impl Hooks for Fake {
    fn build_index(&self, _: &Path, _: &Path, _: &Path, _: &[String]) -> Result<(), String> {
        unreachable!("doctor không dựng index")
    }

    fn handshake(&self, _: &ServerEntry, _: Duration) -> Result<Handshake, String> {
        unreachable!("doctor không bắt tay")
    }

    fn desktop_running(&self) -> io::Result<bool> {
        unreachable!("doctor không hỏi Claude Desktop")
    }

    fn ohs_state(&self, _pkg: OhsPackage, vault: &Path, _wait: Duration) -> OhsState {
        self.ohs_runs.borrow_mut().push(vault.to_path_buf());
        self.ohs.clone()
    }

    fn long_paths_enabled(&self) -> Option<io::Result<bool>> {
        self.long_paths.map(Ok)
    }
}

/// Máy giả hoàn chỉnh: binary `pkm`, vault, index SQLite, gói OHS và cấu hình có `kg` cùng `ohs`.
struct Machine {
    tmp: tempfile::TempDir,
    config: PathBuf,
    vault: PathBuf,
    db: PathBuf,
    ohs: PathBuf,
}

fn machine() -> Machine {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let exe = root.join("bin").join("pkm");
    fs::create_dir_all(exe.parent().unwrap()).unwrap();
    fs::write(&exe, "").unwrap();
    let vault = root.join("Kho");
    fs::create_dir_all(&vault).unwrap();
    let db = root.join("pkm").join("kg").join("kho.db");
    fs::create_dir_all(db.parent().unwrap()).unwrap();
    fs::write(&db, b"SQLite format 3\0....").unwrap();
    let ohs = root.join("pkm").join("ohs");
    let node = ohs.join(if cfg!(windows) {
        "node/node.exe"
    } else {
        "node/bin/node"
    });
    fs::create_dir_all(node.parent().unwrap()).unwrap();
    fs::write(&node, "").unwrap();
    fs::create_dir_all(ohs.join("app/dist/src")).unwrap();
    fs::write(ohs.join("app/dist/src/cli.js"), "").unwrap();
    let entry = crate::install::server_entry(&exe, &vault, &db, &[], None).unwrap();
    let config = root.join("Claude").join(config::CONFIG_FILE);
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let doc =
        serde_json::json!({"mcpServers": {"ohs": {"command": "node"}, "kg": entry.to_value()}});
    fs::write(&config, doc.to_string()).unwrap();
    Machine {
        tmp,
        config,
        vault,
        db,
        ohs,
    }
}

impl Machine {
    fn args(&self) -> DoctorArgs {
        DoctorArgs {
            config: Some(self.config.clone()),
            ohs: Some(self.ohs.clone()),
            ohs_wait: 1,
            ..DoctorArgs::default()
        }
    }
}

fn status(report: &Report, id: &str) -> Status {
    report.checks.iter().find(|c| c.id == id).unwrap().status
}

#[test]
fn healthy_machine_passes_every_check() {
    let m = machine();
    let hooks = Fake::new();
    let report = diagnose(&m.args(), &hooks);
    let ids: Vec<_> = report.checks.iter().map(|c| (c.id, c.status)).collect();
    assert_eq!(
        ids,
        [
            ("claude_config", Status::Ok),
            ("kg_entry", Status::Ok),
            ("db", Status::Ok),
            ("ohs_package", Status::Ok),
            ("ohs_state", Status::Ok),
            ("long_paths", Status::Skip),
        ]
    );
    assert!(report.healthy());
    assert_eq!(*hooks.ohs_runs.borrow(), std::slice::from_ref(&m.vault));
    assert!(report.checks.iter().all(|c| c.action.is_none()));
}

#[test]
fn every_failure_has_an_action() {
    let m = machine();
    fs::remove_file(&m.db).unwrap();
    fs::remove_dir_all(&m.ohs).unwrap();
    let hooks = Fake {
        long_paths: Some(false),
        ..Fake::new()
    };
    let report = diagnose(&m.args(), &hooks);
    assert!(!report.healthy());
    assert_eq!(status(&report, "db"), Status::Fail);
    assert_eq!(status(&report, "ohs_package"), Status::Fail);
    assert_eq!(status(&report, "ohs_state"), Status::Skip);
    assert_eq!(status(&report, "long_paths"), Status::Warn);
    for c in &report.checks {
        assert_eq!(
            c.action.is_some(),
            matches!(c.status, Status::Warn | Status::Fail),
            "{c:?}"
        );
    }
    let text = report.to_text();
    assert!(text.contains("[FAIL] db"), "{text}");
    assert!(
        text.contains("việc cần làm: Chạy `pkm mcp install VAULT`"),
        "{text}"
    );
    assert!(text.ends_with("2 lỗi, 1 cảnh báo\n"), "{text}");
}

#[test]
fn missing_kg_entry_fails_and_skips_vault_checks() {
    let m = machine();
    fs::write(&m.config, r#"{"mcpServers": {"ohs": {"command": "node"}}}"#).unwrap();
    let hooks = Fake::new();
    let report = diagnose(&m.args(), &hooks);
    assert_eq!(status(&report, "claude_config"), Status::Ok);
    assert_eq!(status(&report, "kg_entry"), Status::Fail);
    assert_eq!(status(&report, "db"), Status::Skip);
    assert_eq!(status(&report, "ohs_state"), Status::Skip);
    assert!(hooks.ohs_runs.borrow().is_empty());
}

#[test]
fn kg_entry_with_missing_command_or_vault_fails() {
    let m = machine();
    let set = |command: &str, vault: &Path| {
        let entry = ServerEntry {
            command: command.to_owned(),
            args: vec![vault.to_string_lossy().into_owned(), "mcp".into()],
            env: vec![],
        };
        let doc = serde_json::json!({"mcpServers": {"kg": entry.to_value()}});
        fs::write(&m.config, doc.to_string()).unwrap();
        diagnose(&m.args(), &Fake::new())
    };
    let exe = m.tmp.path().join("bin").join("pkm");
    let r = set(m.tmp.path().join("không-có").to_str().unwrap(), &m.vault);
    assert_eq!(status(&r, "kg_entry"), Status::Fail);
    assert!(
        r.checks[1].detail.contains("không tồn tại"),
        "{:?}",
        r.checks[1]
    );
    let r = set("pkm", &m.vault);
    assert!(
        r.checks[1].detail.contains("không phải đường dẫn đủ"),
        "{:?}",
        r.checks[1]
    );
    let r = set(exe.to_str().unwrap(), &m.tmp.path().join("mất"));
    assert!(
        r.checks[1].detail.contains("vault không tồn tại"),
        "{:?}",
        r.checks[1]
    );
}

#[test]
fn broken_config_json_fails_with_restore_action() {
    let m = machine();
    fs::write(&m.config, "{").unwrap();
    let report = diagnose(&m.args(), &Fake::new());
    assert_eq!(status(&report, "claude_config"), Status::Fail);
    assert!(
        report.checks[0]
            .action
            .as_deref()
            .unwrap()
            .contains(".bak-")
    );
    assert_eq!(status(&report, "kg_entry"), Status::Skip);
}

#[test]
fn ohs_states_map_to_warn_or_fail() {
    let m = machine();
    let run = |ohs: OhsState| {
        let r = diagnose(&m.args(), &Fake { ohs, ..Fake::new() });
        r.checks
            .iter()
            .find(|c| c.id == "ohs_state")
            .unwrap()
            .clone()
    };
    let indexing = run(OhsState::Indexing {
        progress: 0.5,
        indexed: 1,
        pending: 1,
        eta_s: None,
    });
    assert_eq!(indexing.status, Status::Warn);
    assert!(indexing.action.unwrap().contains("đang index"));
    let broken = run(OhsState::Broken {
        action: "Làm X.".into(),
    });
    assert_eq!(
        (broken.status, broken.action.as_deref()),
        (Status::Fail, Some("Làm X."))
    );
    assert_eq!(
        run(OhsState::ModelMissing { detail: "m".into() }).status,
        Status::Fail
    );
}

#[test]
fn no_ohs_start_skips_running_ohs() {
    let m = machine();
    let hooks = Fake::new();
    let mut args = m.args();
    args.no_ohs_start = true;
    let report = diagnose(&args, &hooks);
    assert_eq!(status(&report, "ohs_state"), Status::Skip);
    assert!(hooks.ohs_runs.borrow().is_empty());
}

#[test]
fn explicit_db_overrides_entry_db() {
    let m = machine();
    let mut args = m.args();
    args.db = Some(m.tmp.path().join("khác.db"));
    let report = diagnose(&args, &Fake::new());
    assert_eq!(status(&report, "db"), Status::Fail);
    assert!(report.checks[2].detail.contains("khác.db"));
}

#[test]
fn json_output_has_stable_shape() {
    let m = machine();
    let mut args = m.args();
    args.json = true;
    let mut out = Vec::new();
    let healthy = run(&args, &Fake::new(), &mut out).unwrap();
    assert!(healthy);
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["healthy"], true);
    let first = &v["checks"][0];
    assert_eq!(first["id"], "claude_config");
    assert_eq!(first["status"], "ok");
    assert!(first["action"].is_null());
    let keys: Vec<_> = first.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["id", "status", "detail", "action"]);
}
