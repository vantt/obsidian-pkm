use super::*;
use std::cell::RefCell;
use std::fs;
use std::io;

use crate::install::handshake::Handshake;

/// Hooks giả cho `doctor`: trạng thái OHS, `node --version` và đường dẫn dài đặt trước, ghi lại vault OHS được chạy
/// trên. Mọi hook ghi hoặc khởi chạy thứ khác đều `unreachable!`: doctor chỉ đọc.
struct Fake {
    ohs: OhsState,
    node: Result<String, String>,
    long_paths: Option<bool>,
    ohs_runs: RefCell<Vec<PathBuf>>,
}

impl Fake {
    fn new() -> Self {
        Self {
            ohs: OhsState::Ready,
            node: Ok("v22.0.0".into()),
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

    fn desktop_process(&self) -> io::Result<Option<String>> {
        unreachable!("doctor không hỏi Claude Desktop")
    }

    fn fetch(&self, _: &str, _: &Path) -> Result<(), String> {
        unreachable!("doctor không tải gì")
    }

    fn ohs_state(&self, _pkg: OhsPackage, vault: &Path, _wait: Duration) -> OhsState {
        self.ohs_runs.borrow_mut().push(vault.to_path_buf());
        self.ohs.clone()
    }

    fn long_paths_enabled(&self) -> Option<io::Result<bool>> {
        self.long_paths.map(Ok)
    }

    fn node_version(&self, _node: &Path) -> Result<String, String> {
        self.node.clone()
    }
}

/// Máy giả hoàn chỉnh: binary `pkm`, vault, index SQLite, gói OHS và cấu hình có `kg` cùng `ohs`.
struct Machine {
    tmp: tempfile::TempDir,
    exe: PathBuf,
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
    let config = root.join("Claude").join(config::CONFIG_FILE);
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let m = Machine {
        tmp,
        exe,
        config,
        vault,
        db,
        ohs,
    };
    m.write_entry(&[]);
    m
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

    /// Ghi cấu hình có `ohs` và mục `kg` như `pkm mcp install` ghi, với `env` cho mục `kg`.
    fn write_entry(&self, env: &[(&str, &str)]) {
        let mut entry = install::server_entry(
            &self.exe,
            &self.vault,
            &self.db,
            &["--adapter".into(), "e11".into()],
            Some("Kho"),
        )
        .unwrap();
        entry.env = env
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let doc =
            serde_json::json!({"mcpServers": {"ohs": {"command": "node"}, "kg": entry.to_value()}});
        fs::write(&self.config, doc.to_string()).unwrap();
    }

    /// Binary `pkm` đang chạy doctor (khác binary của mục `kg`).
    fn doctor_exe(&self) -> PathBuf {
        self.tmp.path().join("khác").join("pkm")
    }

    fn diagnose(&self, args: &DoctorArgs, hooks: &Fake) -> Report {
        diagnose(args, hooks, &self.doctor_exe())
    }
}

fn check<'a>(report: &'a Report, id: &str) -> &'a Check {
    report.checks.iter().find(|c| c.id == id).unwrap()
}

fn status(report: &Report, id: &str) -> Status {
    check(report, id).status
}

#[test]
fn healthy_machine_passes_every_check_without_starting_ohs() {
    let m = machine();
    let hooks = Fake::new();
    let report = m.diagnose(&m.args(), &hooks);
    let ids: Vec<_> = report.checks.iter().map(|c| (c.id, c.status)).collect();
    assert_eq!(
        ids,
        [
            ("claude_config", Status::Ok),
            ("kg_entry", Status::Ok),
            ("db", Status::Ok),
            ("ohs_package", Status::Ok),
            ("ohs_node", Status::Ok),
            ("ohs_state", Status::Skip),
            ("long_paths", Status::Skip),
        ]
    );
    assert!(report.ok());
    assert!(
        hooks.ohs_runs.borrow().is_empty(),
        "không --deep thì không chạy OHS"
    );
    assert!(check(&report, "ohs_state").detail.contains("--deep"));
    assert!(
        report
            .checks
            .iter()
            .all(|c| c.action.is_none() && c.fix.is_none())
    );
}

#[test]
fn deep_runs_ohs_on_the_entry_vault() {
    let m = machine();
    let hooks = Fake::new();
    let mut args = m.args();
    args.deep = true;
    let report = m.diagnose(&args, &hooks);
    assert_eq!(status(&report, "ohs_state"), Status::Ok);
    assert_eq!(*hooks.ohs_runs.borrow(), std::slice::from_ref(&m.vault));
}

#[test]
fn missing_ohs_package_only_warns_and_explains_the_impact() {
    let m = machine();
    fs::remove_dir_all(&m.ohs).unwrap();
    let report = m.diagnose(&m.args(), &Fake::new());
    assert!(report.ok(), "thiếu OHS không làm doctor báo lỗi");
    let c = check(&report, "ohs_package");
    assert_eq!(c.status, Status::Warn);
    assert!(
        c.detail.contains("chỉ `related` khi không có `hits`"),
        "{c:?}"
    );
    assert!(c.action.as_deref().unwrap().contains("PKM_OHS_PACKAGE"));
    let fix = c.fix.as_deref().unwrap();
    assert!(fix.contains("mcp install") && fix.contains("e11"), "{fix}");
    assert_eq!(status(&report, "ohs_node"), Status::Skip);
    assert_eq!(status(&report, "ohs_state"), Status::Skip);
}

#[test]
fn missing_ohs_package_is_skipped_when_installed_with_no_ohs() {
    let m = machine();
    fs::remove_dir_all(&m.ohs).unwrap();
    m.write_entry(&[(install::NO_OHS_ENV, "1")]);
    let report = m.diagnose(&m.args(), &Fake::new());
    let c = check(&report, "ohs_package");
    assert_eq!(c.status, Status::Skip, "{c:?}");
    assert!(c.detail.contains("--no-ohs"));
    assert_eq!(report.count(Status::Warn), 0);
}

#[test]
fn broken_node_and_every_ohs_state_only_warn() {
    let m = machine();
    let report = m.diagnose(
        &m.args(),
        &Fake {
            node: Err("node lỗi".into()),
            ..Fake::new()
        },
    );
    assert_eq!(status(&report, "ohs_node"), Status::Warn);
    assert!(report.ok());

    let mut args = m.args();
    args.deep = true;
    for ohs in [
        OhsState::Indexing {
            progress: 0.5,
            indexed: 1,
            pending: 1,
            eta_s: None,
        },
        OhsState::Broken {
            action: "Làm X.".into(),
        },
        OhsState::ModelMissing { detail: "m".into() },
    ] {
        let r = m.diagnose(&args, &Fake { ohs, ..Fake::new() });
        let c = check(&r, "ohs_state");
        assert_eq!(c.status, Status::Warn, "{c:?}");
        assert!(r.ok());
    }
}

#[test]
fn every_problem_has_an_action_and_fail_fixes_go_to_next() {
    let m = machine();
    fs::remove_file(&m.db).unwrap();
    let hooks = Fake {
        long_paths: Some(false),
        ..Fake::new()
    };
    let report = m.diagnose(&m.args(), &hooks);
    assert!(!report.ok());
    assert_eq!(status(&report, "db"), Status::Fail);
    assert_eq!(status(&report, "long_paths"), Status::Warn);
    for c in &report.checks {
        assert_eq!(
            c.action.is_some(),
            matches!(c.status, Status::Warn | Status::Fail),
            "{c:?}"
        );
    }
    let fix = check(&report, "db").fix.clone().unwrap();
    let want = shell::command_line(
        m.exe.to_str().unwrap(),
        &[
            m.vault.to_str().unwrap(),
            "build",
            "--db",
            m.db.to_str().unwrap(),
            "--adapter",
            "e11",
        ],
    );
    assert_eq!(
        fix, want,
        "dựng lại đúng index, cùng cờ đọc, bằng binary của `kg`"
    );
    assert!(
        check(&report, "long_paths")
            .fix
            .as_deref()
            .unwrap()
            .starts_with("New-ItemProperty")
    );
    assert_eq!(report.next(), [fix], "next chỉ gồm lệnh của mục fail");
    let text = report.to_text();
    assert!(text.contains("[FAIL] db"), "{text}");
    assert!(text.contains("       lệnh: "), "{text}");
    assert!(text.ends_with("1 lỗi, 1 cảnh báo\n"), "{text}");
}

#[test]
fn missing_kg_entry_fails_and_skips_vault_checks() {
    let m = machine();
    fs::write(&m.config, r#"{"mcpServers": {"ohs": {"command": "node"}}}"#).unwrap();
    let hooks = Fake::new();
    let mut args = m.args();
    args.deep = true;
    let report = m.diagnose(&args, &hooks);
    assert_eq!(status(&report, "claude_config"), Status::Ok);
    assert_eq!(status(&report, "kg_entry"), Status::Fail);
    assert_eq!(check(&report, "kg_entry").fix, None, "chưa biết vault nào");
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
        m.diagnose(&m.args(), &Fake::new())
    };
    let r = set(m.tmp.path().join("không-có").to_str().unwrap(), &m.vault);
    let c = check(&r, "kg_entry");
    assert_eq!(c.status, Status::Fail);
    assert!(c.detail.contains("không tồn tại"), "{c:?}");
    assert_eq!(
        c.fix.as_deref(),
        Some(
            shell::command_line(
                m.doctor_exe().to_str().unwrap(),
                &["mcp", "install", m.vault.to_str().unwrap()]
            )
            .as_str()
        ),
        "binary của `kg` mất thì cài lại bằng binary đang chạy doctor"
    );
    let r = set("pkm", &m.vault);
    assert!(
        check(&r, "kg_entry")
            .detail
            .contains("không phải đường dẫn đủ")
    );
    let r = set(m.exe.to_str().unwrap(), &m.tmp.path().join("mất"));
    let c = check(&r, "kg_entry");
    assert!(c.detail.contains("vault không tồn tại"), "{c:?}");
    assert_eq!(c.fix, None, "vault mất thì không đoán lệnh");
}

#[test]
fn broken_config_json_fails_with_restore_action() {
    let m = machine();
    fs::write(&m.config, "{").unwrap();
    let report = m.diagnose(&m.args(), &Fake::new());
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
fn explicit_db_overrides_entry_db() {
    let m = machine();
    let mut args = m.args();
    let other = m.tmp.path().join("khác.db");
    args.db = Some(other.clone());
    let report = m.diagnose(&args, &Fake::new());
    let c = check(&report, "db");
    assert_eq!(c.status, Status::Fail);
    assert!(c.detail.contains("khác.db"));
    assert!(c.fix.as_deref().unwrap().contains("khác.db"), "{c:?}");
}

#[test]
fn json_envelope_has_stable_keys() {
    let m = machine();
    let mut args = m.args();
    args.json = true;
    let mut out = Vec::new();
    assert!(run_with(&args, &Fake::new(), &m.doctor_exe(), &mut out).unwrap());
    let v: Value = serde_json::from_slice(&out).unwrap();
    let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["schema", "ok", "data", "error", "next"]);
    assert_eq!(v["schema"], 1);
    assert_eq!(v["ok"], true);
    assert!(v["error"].is_null());
    assert_eq!(v["next"], serde_json::json!([]));
    let first = &v["data"]["checks"][0];
    assert_eq!(first["id"], "claude_config");
    assert_eq!(first["status"], "ok");
    assert!(first["action"].is_null() && first["fix"].is_null());
    let keys: Vec<_> = first.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["id", "status", "detail", "action", "fix"]);

    fs::remove_file(&m.db).unwrap();
    let mut out = Vec::new();
    assert!(!run_with(&args, &Fake::new(), &m.doctor_exe(), &mut out).unwrap());
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "checks_failed");
    assert_eq!(v["error"]["why"], "1 mục lỗi: db");
    assert_eq!(v["error"]["fix"], v["next"][0]);
    assert!(
        v["next"][0].as_str().unwrap().contains(" build --db "),
        "{v}"
    );
}

#[test]
fn doctor_args_parse_deep_flag() {
    use clap::Parser;
    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        args: DoctorArgs,
    }
    let cli = Cli::try_parse_from(["doctor", "--deep", "--json"]).unwrap();
    assert!(cli.args.deep && cli.args.json);
    assert!(!Cli::try_parse_from(["doctor"]).unwrap().args.deep);
    assert!(Cli::try_parse_from(["doctor", "--no-ohs-start"]).is_err());
}
