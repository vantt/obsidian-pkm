//! Binary `pkm` như người dùng gọi: lệnh trên vault không cần tầng tool (`build`, `stats`, `validate`), cách đọc
//! nhớ trong index, `pkm mcp install --dry-run`, `pkm doctor`, trợ giúp.

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
        "--no-ohs-start",
        "--json",
    ]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let rep: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(rep["healthy"], false);
    assert!(
        rep["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "kg_entry")
    );
}
