//! Binary `pkm` như người dùng gọi: lệnh trên vault (`build`, `stats`, `validate`, các lệnh hỏi graph qua tầng tool),
//! cách đọc nhớ trong index, `pkm mcp install --dry-run`, `pkm doctor`, trợ giúp.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn pkm(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pkm"))
        .args(args)
        .output()
        .expect("chạy được pkm")
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

struct Vault {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    vault: String,
    db: String,
}

fn vault() -> Vault {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("vault");
    for (path, fm) in [
        ("Yếu tố/Doanh thu.md", "type: factor"),
        (
            "Yếu tố/Số lead.md",
            "type: factor\nincreases: [\"[[Doanh thu]]\"]",
        ),
        (
            "Khái niệm/Tự trỏ.md",
            "type: concept\nrelated: [\"[[Tự trỏ]]\"]",
        ),
    ] {
        let p = vault.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, format!("---\n{fm}\n---\n")).unwrap();
    }
    let db = tmp.path().join("graph.db");
    Vault {
        root: tmp.path().to_path_buf(),
        vault: vault.to_string_lossy().into_owned(),
        db: db.to_string_lossy().into_owned(),
        _tmp: tmp,
    }
}

#[test]
fn build_stats_validate_like_python_cli() {
    let v = vault();
    let o = pkm(&[&v.vault, "build", "--packs", "business", "--db", &v.db]);
    assert!(o.status.success(), "{o:?}");
    let text = stdout(&o);
    assert!(
        text.starts_with(&format!("{} (full): 3 note, đọc 3, xoá 0; ms ", v.db)),
        "{text}"
    );
    assert!(text.ends_with("cạnh theo lớp: {'accepted': 2}\n"), "{text}");

    // Lần sau không ghi --packs: lấy gói của lần build trước, không đọc lại note nào.
    let o = pkm(&[&v.vault, "build", "--json", "--db", &v.db]);
    assert!(o.status.success(), "{o:?}");
    let text = stdout(&o);
    assert!(text.starts_with("{\n \"vault\": "), "thụt lề 1 như Python");
    assert!(text.ends_with('}'), "build --json không xuống dòng cuối");
    let info: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(info["packs"], "business");
    assert_eq!(info["mode"], "incremental");
    assert_eq!(info["notes_read"], "0");

    let o = pkm(&[&v.vault, "stats", "--db", &v.db]);
    assert!(o.status.success(), "{o:?}");
    let stats: Value = serde_json::from_str(&stdout(&o)).unwrap();
    let keys: Vec<&str> = stats
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "meta",
            "nodes",
            "nodes_by_type",
            "edges_by_status",
            "edges_by_origin",
            "edges_by_rel",
            "validator"
        ]
    );
    assert_eq!(stats["nodes"], 3);
    assert_eq!(stats["nodes_by_type"]["factor"], 2);
    assert_eq!(stats["validator"]["R7"]["deviations"], 1);

    let o = pkm(&[&v.vault, "validate", "--db", &v.db]);
    assert!(o.status.success(), "{o:?}");
    let text = stdout(&o);
    assert!(
        text.starts_with("Lệch schema v3 (luật | số chỗ | số note):\n  R7        1      1\n"),
        "{text}"
    );
    assert!(
        text.contains("\nR7:\n  Khái niệm/Tự trỏ.md: related trỏ về chính note\n"),
        "{text}"
    );

    let o = pkm(&[
        &v.vault, "validate", "--rule", "R7", "--json", "--db", &v.db,
    ]);
    let rep: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(rep["examples"]["R7"][0]["node"], "Khái niệm/Tự trỏ.md");
}

#[test]
fn first_query_builds_the_missing_index_quietly() {
    let v = vault();
    let o = pkm(&[&v.vault, "stats", "--packs", "business", "--db", &v.db]);
    assert!(o.status.success(), "{o:?}");
    let stats: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(stats["meta"]["packs"], "business");
    // Mặc định index nằm ở VAULT/.kg/graph.db.
    let o = pkm(&[&v.vault, "stats"]);
    assert!(o.status.success(), "{o:?}");
    assert!(Path::new(&v.vault).join(".kg/graph.db").is_file());
}

#[test]
fn graph_commands_query_the_index_like_python_cli() {
    let v = vault();
    let run = |args: &[&str]| -> Value {
        let mut all = vec![v.vault.as_str()];
        all.extend_from_slice(args);
        all.extend_from_slice(&["--db", &v.db, "--packs", "business"]);
        let o = pkm(&all);
        assert!(o.status.success(), "{args:?}: {o:?}");
        let text = stdout(&o);
        assert!(text.starts_with("{\n "), "JSON thụt lề 1: {text}");
        serde_json::from_str(&text).unwrap()
    };
    let r = run(&["find", "doanh", "thu"]);
    assert_eq!(r["count"], 1);
    assert_eq!(r["items"][0]["id"], "Yếu tố/Doanh thu.md");
    // CLI gọi thẳng `Tools.note`: không link `obsidian://`, không thân note, id sai không gợi ý.
    let r = run(&["note", "Yếu tố/Doanh thu.md"]);
    assert_eq!(r["type"], "factor");
    assert!(r.get("url").is_none() && r.get("body").is_none(), "{r}");
    let r = run(&["note", "Doanh thu"]);
    assert_eq!(
        r,
        serde_json::json!({"error": "không có note Doanh thu; dùng find"})
    );
    let r = run(&["neighbors", "Yếu tố/Doanh thu.md", "--dir", "in"]);
    assert_eq!(r["items"][0]["rel"], "increased_by");
    assert_eq!(r["items"][0]["node"], "Yếu tố/Số lead.md");
    let r = run(&["reach", "Yếu tố/Số lead.md", "--max-depth", "2"]);
    assert_eq!(
        r["items"],
        serde_json::json!([{"id": "Yếu tố/Doanh thu.md", "depth": 1}])
    );
    let r = run(&["select", "type=factor", "--limit", "1"]);
    assert_eq!((&r["count"], &r["truncated"]), (&2.into(), &true.into()));
    assert!(r["handle"].as_str().unwrap().starts_with("select-"), "{r}");
    // Không có --as-of: Python so với NULL trong SQL, không note nào khớp.
    let r = run(&["valid", "factor"]);
    assert_eq!(r, serde_json::json!({"count": 0, "items": []}));
}

#[test]
fn loops_without_max_len_exits_one_like_python() {
    let v = vault();
    let o = pkm(&[&v.vault, "loops", "Yếu tố/Số lead.md", "--db", &v.db]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    assert_eq!(
        String::from_utf8(o.stderr).unwrap(),
        "loops cần --max-len\n"
    );
}

#[test]
fn unknown_command_and_missing_arguments_are_usage_errors() {
    assert_eq!(pkm(&["v", "nope"]).status.code(), Some(2));
    assert_eq!(pkm(&[]).status.code(), Some(2));
    let o = pkm(&["--help"]);
    assert!(o.status.success());
    assert!(stdout(&o).contains("pkm mcp install"));
}

#[test]
fn mcp_install_dry_run_writes_nothing() {
    let v = vault();
    let config = v.root.join("Claude").join("claude_desktop_config.json");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        "{\"mcpServers\": {\"ohs\": {\"command\": \"x\"}}}\n",
    )
    .unwrap();
    let before = fs::read(&config).unwrap();
    let o = pkm(&[
        "mcp",
        "install",
        &v.vault,
        "--db",
        &v.db,
        "--packs",
        "business",
        "--config",
        &config.to_string_lossy(),
        "--no-ohs",
        "--dry-run",
    ]);
    assert!(o.status.success(), "{o:?}");
    let text = stdout(&o);
    assert!(
        text.starts_with("[dry-run] không ghi gì. Kế hoạch:\n"),
        "{text}"
    );
    assert!(
        text.contains("\"mcp\""),
        "mục kg chạy `pkm VAULT mcp`: {text}"
    );
    assert_eq!(fs::read(&config).unwrap(), before, "cấu hình không đổi");
    assert!(!Path::new(&v.db).exists(), "dry-run không dựng index");
}

#[test]
fn doctor_reports_json_and_fails_without_config_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("claude_desktop_config.json");
    fs::write(&config, "{\"mcpServers\": {}}\n").unwrap();
    let o = pkm(&[
        "doctor",
        "--config",
        &config.to_string_lossy(),
        "--ohs",
        &tmp.path().join("ohs").to_string_lossy(),
        "--json",
    ]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let rep: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(rep["schema"], 1);
    assert_eq!(rep["ok"], false);
    assert_eq!(rep["error"]["code"], "checks_failed");
    assert!(
        rep["data"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "kg_entry")
    );
}

// ------------------------------------------------------------------------------------------------ related

mod mcp_support;

/// `pkm VAULT related ...` không thấy biến OHS của máy chạy test; `env` thêm vào.
fn related(v: &Vault, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pkm"));
    for k in ["PKM_OHS_DIR", "PKM_NO_OHS", "KG_OHS_CLI", "KG_OHS_NODE"] {
        c.env_remove(k);
    }
    c.envs(env.iter().copied())
        .arg(&v.vault)
        .arg("related")
        .args(args)
        .args(["--db", &v.db, "--packs", "business"])
        .output()
        .expect("chạy được pkm")
}

#[test]
fn related_with_hits_needs_no_ohs() {
    let v = vault();
    let o = related(&v, &["--hit", "Yếu tố/Số lead.md", "--k", "5"], &[]);
    assert!(o.status.success(), "{o:?}");
    let r: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(r["seed_source"], "hits");
    assert_eq!(r["seeds"][0]["id"], "Yếu tố/Số lead.md");
    assert_eq!(r["related"][0]["id"], "Yếu tố/Doanh thu.md");
}

#[test]
fn related_query_without_ohs_prints_not_ready_and_fails() {
    let v = vault();
    let o = related(&v, &["doanh", "thu"], &[("PKM_NO_OHS", "1")]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let r: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(r["error"], "ohs_not_ready");
    assert_eq!(r["state"], "broken");
    assert!(
        String::from_utf8_lossy(&o.stderr).starts_with("related: ohs_not_ready"),
        "{o:?}"
    );
}

#[test]
fn related_query_waits_for_ohs_then_shuts_it_down() {
    let v = vault();
    let Some(pkg) = mcp_support::fake_ohs_package(&v.root) else {
        return;
    };
    let pkg = pkg.to_string_lossy().into_owned();
    let o = related(&v, &["Yếu tố/Số lead.md", "--ohs", &pkg], &[]);
    assert!(o.status.success(), "{o:?}");
    let r: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(r["seed_source"], "ohs-cli");
    assert_eq!(r["seeds"][0]["id"], "Yếu tố/Số lead.md");
    let pids = mcp_support::fake_pids(Path::new(&v.vault));
    assert_eq!(pids.len(), 1);
    assert!(
        pids.iter().all(|p| !mcp_support::process::alive(*p)),
        "OHS còn chạy"
    );
}

#[test]
fn related_query_gives_up_after_ohs_wait() {
    let v = vault();
    let Some(pkg) = mcp_support::fake_ohs_package(&v.root) else {
        return;
    };
    fs::write(
        Path::new(&v.vault).join("fake-ohs.json"),
        r#"{"silent": true}"#,
    )
    .unwrap();
    let pkg = pkg.to_string_lossy().into_owned();
    // Gói chỉ qua biến môi trường, như sau `pkm mcp install`.
    let o = related(&v, &["lead", "--ohs-wait", "1"], &[("PKM_OHS_DIR", &pkg)]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let r: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(r["error"], "ohs_not_ready");
    assert_eq!(r["state"], "starting");
    let pids = mcp_support::fake_pids(Path::new(&v.vault));
    assert_eq!(pids.len(), 1);
    assert!(
        pids.iter().all(|p| !mcp_support::process::alive(*p)),
        "OHS còn chạy"
    );
}
