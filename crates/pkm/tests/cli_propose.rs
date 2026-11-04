//! `pkm VAULT propose` như người dùng gọi: JSON, chế độ ghi note đề xuất, cặp gần nghĩa từ file, lỗi đầu vào.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn pkm(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pkm"))
        .args(args)
        .output()
        .expect("chạy được pkm")
}

fn json_of(o: &Output) -> Value {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).expect("stdout là JSON")
}

/// Vault bịa: hai note trùng thân, một note có đoạn tô, `.obsidian/` khi `obsidian`.
fn vault(obsidian: bool) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let body = "Một đoạn văn đủ dài để được tính là nội dung trùng nhau giữa hai note.\n";
    for (rel, text) in [
        ("A/Bản một.md", body.to_owned()),
        ("B/Bản hai.md", body.to_owned()),
        (
            "C/Đọc.md",
            "==một ý đáng giữ lại== trong note.\n".to_owned(),
        ),
    ] {
        let p = tmp.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    if obsidian {
        fs::create_dir_all(tmp.path().join(".obsidian")).unwrap();
    }
    tmp
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn json_lists_new_proposals_without_writing() {
    let v = vault(false);
    let out = json_of(&pkm(&[path(v.path()), "propose", "--json"]));
    assert_eq!(out["total"], 2);
    assert_eq!(out["new"], 2);
    assert_eq!(
        out["by_kind"],
        serde_json::json!({"duplicate": 1, "idea": 1})
    );
    assert_eq!(out["proposals"][0]["kind"], "duplicate");
    assert_eq!(
        out["proposals"][0]["notes"],
        serde_json::json!(["A/Bản một.md", "B/Bản hai.md"])
    );
    assert_eq!(out["written"], serde_json::json!([]));
    assert!(!v.path().join("_Đề xuất").exists());
}

#[test]
fn create_writes_new_notes_once() {
    let v = vault(true);
    let out = json_of(&pkm(&[path(v.path()), "propose", "--create", "--json"]));
    let written: Vec<&str> = out["written"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert_eq!(
        written,
        ["_Đề xuất/Trùng - Bản một.md", "_Đề xuất/Rút ý - Đọc.md"]
    );
    let note = fs::read_to_string(v.path().join(written[0])).unwrap();
    assert!(note.starts_with("---\ntype: proposal\nkind: duplicate\nstatus: chờ\n"));
    assert!(note.contains("![[A/Bản một]]"));

    let again = json_of(&pkm(&[path(v.path()), "propose", "--create", "--json"]));
    assert_eq!(
        (again["total"].as_u64(), again["new"].as_u64()),
        (Some(2), Some(0))
    );

    let text = pkm(&[path(v.path()), "propose"]);
    assert!(text.status.success());
    assert_eq!(
        String::from_utf8(text.stdout).unwrap(),
        "proposals 2, new 0: {}\n"
    );
}

#[test]
fn create_needs_an_obsidian_vault() {
    let v = vault(false);
    let o = pkm(&[path(v.path()), "propose", "--create"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("not a vault: no .obsidian/"));
    assert!(!v.path().join("_Đề xuất").exists());
}

#[test]
fn similar_pairs_come_from_a_file() {
    let v = vault(false);
    let sim = v.path().join("similar.json");
    fs::write(
        &sim,
        r#"[{"a": "A/Bản một.md", "b": "C/Đọc.md", "score": 0.75}, {"a": "A/Bản một.md", "b": "Z.md"}]"#,
    )
    .unwrap();
    let out = json_of(&pkm(&[
        path(v.path()),
        "propose",
        "--similar",
        path(&sim),
        "--json",
    ]));
    let related: Vec<&Value> = out["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["kind"] == "related")
        .collect();
    assert_eq!(related.len(), 1);
    assert_eq!(related[0]["score"], 0.75);

    fs::write(&sim, r#"{"a": "x"}"#).unwrap();
    let o = pkm(&[path(v.path()), "propose", "--similar", path(&sim)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("--similar"));
}

#[test]
fn missing_vault_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("không có");
    let o = pkm(&[path(&missing), "propose", "--json"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("không phải thư mục"));
}
