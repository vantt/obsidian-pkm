//! `pkm VAULT material|pairs|zones` như người dùng gọi, với OHS giả (`fake-ohs` của `pkm-core`): JSON ra stdout, file
//! ghi đúng chỗ, trang gợi ý không bị đè, lỗi `ohs_not_ready` khi không có OHS, không còn tiến trình OHS sau lệnh.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

mod mcp_support;

struct Vault {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    vault: PathBuf,
}

const BODY: &str = "một hai ba bốn năm sáu bảy tám chín mười";

/// Vault kiểu bản B: cụm `Vườn/` (có đoạn tô), cụm `Bếp/`, bản nháp, trang hệ thống; index dựng như bản B.
fn vault() -> Vault {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let mut notes: Vec<(String, String)> = Vec::new();
    for i in 0..6 {
        notes.push((
            format!("Vườn/Cây {i}.md"),
            format!("---\ntype: note\n---\nCây vườn {i} ==tưới sớm== #hl/green {BODY}\n"),
        ));
        notes.push((
            format!("Bếp/Món {i}.md"),
            format!(
                "---\ntype: clip\n---\nMón bếp {i} [[Món {}]] {BODY}\n",
                (i + 1) % 6
            ),
        ));
    }
    notes.push((
        "Bài viết/Nháp.md".into(),
        "---\ntype: post\nuses: []\n---\n\n## Ý\n\nVườn cần người chăm mỗi ngày.\n".into(),
    ));
    notes.push((
        "_Xưởng/Trang.md".into(),
        format!("---\ntype: toc\n---\n{BODY}\n"),
    ));
    for (p, t) in &notes {
        let f = vault.join(p);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(&f, t).unwrap();
    }
    let v = Vault {
        root: tmp.path().to_path_buf(),
        vault,
        _tmp: tmp,
    };
    let o = pkm(
        &v,
        &[
            "build",
            "--adapter",
            "e11",
            "--packs",
            "capture",
            "--no-aliases",
            "--edge-keys",
            "links",
            "--body-edges",
        ],
        &[],
    );
    assert!(o.status.success(), "{o:?}");
    v
}

/// Vector giả: cụm `Vườn/` quanh trục x, cụm `Bếp/` quanh trục y; trang hệ thống sát cụm `Vườn/`.
fn fake_config(extra: Value) -> Value {
    let mut vectors = serde_json::Map::new();
    for i in 0..6 {
        let d = f64::from(i) * 0.01;
        vectors.insert(format!("Vườn/Cây {i}.md"), json!([1.0, d, 0.0]));
        vectors.insert(format!("Bếp/Món {i}.md"), json!([d, 1.0, 0.0]));
    }
    vectors.insert("_Xưởng/Trang.md".into(), json!([1.0, 0.0, 0.0]));
    let mut cfg = json!({"vectors": vectors});
    if let (Some(c), Value::Object(e)) = (cfg.as_object_mut(), extra) {
        c.extend(e);
    }
    cfg
}

fn pkm(v: &Vault, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pkm"));
    for k in ["PKM_OHS_DIR", "PKM_NO_OHS", "KG_OHS_CLI", "KG_OHS_NODE"] {
        c.env_remove(k);
    }
    c.envs(env.iter().copied())
        .arg(&v.vault)
        .args(args)
        .output()
        .expect("chạy được pkm")
}

fn json_out(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("{e}: {o:?}"))
}

/// Gói OHS giả với cấu hình `cfg`; `None` khi không có `fake-ohs` (test bỏ qua).
fn fake(v: &Vault, cfg: &Value) -> Option<String> {
    let pkg = mcp_support::fake_ohs_package(&v.root)?;
    fs::write(v.vault.join("fake-ohs.json"), cfg.to_string()).unwrap();
    Some(pkg.to_string_lossy().into_owned())
}

fn assert_no_ohs_left(v: &Vault) {
    let pids = mcp_support::fake_pids(Path::new(&v.vault));
    assert!(!pids.is_empty());
    assert!(
        pids.iter().all(|p| !mcp_support::process::alive(*p)),
        "OHS còn chạy"
    );
}

#[test]
fn pairs_prints_and_writes_the_similar_pairs_file() {
    let v = vault();
    let Some(pkg) = fake(&v, &fake_config(json!({}))) else {
        return;
    };
    let o = pkm(
        &v,
        &[
            "pairs",
            "--ohs",
            &pkg,
            "--min",
            "0.9",
            "--min-words",
            "5",
            "--write",
        ],
        &[],
    );
    assert!(o.status.success(), "{o:?}");
    let list = json_out(&o);
    let file = v.vault.join("_E11").join("similar-pairs.json");
    let written: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(written, list);
    let pairs = list.as_array().unwrap();
    // `Vườn/` và `Bếp/` vuông góc; trong một thư mục hai đoạn đường dẫn là hai nhóm khác nhau nên vẫn là cặp. Trang
    // hệ thống không bao giờ vào cặp. Mỗi note tối đa hai cặp.
    assert!(!pairs.is_empty());
    for p in pairs {
        let (a, b) = (p["a"].as_str().unwrap(), p["b"].as_str().unwrap());
        assert_eq!(a.split('/').next(), b.split('/').next(), "{p}");
        assert!(!a.starts_with("_Xưởng") && !b.starts_with("_Xưởng"), "{p}");
    }
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(stderr.starts_with("notes 12, pairs >= 0.9: "), "{stderr}");
    assert_no_ohs_left(&v);
}

#[test]
fn zones_write_json_and_page_but_never_overwrite_the_page() {
    let v = vault();
    let Some(pkg) = fake(&v, &fake_config(json!({}))) else {
        return;
    };
    let o = pkm(&v, &["zones", "--ohs", &pkg, "--apply", "--k", "5"], &[]);
    assert!(o.status.success(), "{o:?}");
    let out = json_out(&o);
    assert_eq!(out["zones"].as_array().unwrap().len(), 2, "{out}");
    assert_eq!(out["stats"]["zones"], 2);
    let file: Value =
        serde_json::from_str(&fs::read_to_string(v.vault.join(".e11/zones.json")).unwrap())
            .unwrap();
    assert_eq!(file["zones"], out["zones"]);
    assert_eq!(file["generated"], out["generated"]);
    let page = v.vault.join("_Xưởng").join("Gợi ý bài viết.md");
    let text = fs::read_to_string(&page).unwrap();
    assert!(text.contains("# Gợi ý bài viết"), "{text}");
    assert!(text.contains("tưới sớm *(làm được)*"), "{text}");

    fs::write(&page, "đáng viết").unwrap();
    let o = pkm(&v, &["zones", "--ohs", &pkg, "--apply", "--k", "5"], &[]);
    assert!(o.status.success(), "{o:?}");
    assert_eq!(fs::read_to_string(&page).unwrap(), "đáng viết");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("giữ nguyên"),
        "{o:?}"
    );
    assert_no_ohs_left(&v);
}

#[test]
fn material_searches_each_paragraph_and_walks_the_graph() {
    let v = vault();
    let cfg =
        fake_config(json!({"hits": ["Vườn/Cây 1.md", "Bài viết/Nháp.md", "_Xưởng/Trang.md"]}));
    let Some(pkg) = fake(&v, &cfg) else {
        return;
    };
    let o = pkm(&v, &["material", "Bài viết/Nháp", "--ohs", &pkg], &[]);
    assert!(o.status.success(), "{o:?}");
    let r = json_out(&o);
    assert_eq!(r["draft"], "Bài viết/Nháp.md");
    assert_eq!(
        r["queries"],
        json!([["tên bài", "Nháp"], ["Ý", "Vườn cần người chăm mỗi ngày."]])
    );
    assert_eq!(r["methods"], json!({"ohs": 2}));
    let items = r["items"].as_array().unwrap();
    assert_eq!(items[0]["id"], "Vườn/Cây 1.md");
    assert_eq!(items[0]["sections"], json!(["tên bài", "Ý"]));
    assert_eq!(items[0]["highlights"], 1);
    assert!(
        items
            .iter()
            .all(|i| i["id"] != "Bài viết/Nháp.md" && i["id"] != "_Xưởng/Trang.md"),
        "{r}"
    );
    assert_no_ohs_left(&v);
}

#[test]
fn without_ohs_commands_report_not_ready_and_exit_1() {
    let v = vault();
    for args in [
        &["material", "Bài viết/Nháp"][..],
        &["pairs"][..],
        &["zones"][..],
    ] {
        let o = pkm(&v, args, &[("PKM_NO_OHS", "1")]);
        assert_eq!(o.status.code(), Some(1), "{args:?} {o:?}");
        let e = json_out(&o);
        assert_eq!(e["error"], "ohs_not_ready", "{args:?}");
        assert_eq!(e["state"], "broken");
        assert_eq!(e.as_object().unwrap().len(), 5);
        assert!(
            String::from_utf8_lossy(&o.stderr).contains("ohs_not_ready"),
            "{o:?}"
        );
    }
}

#[test]
fn bad_draft_and_missing_index_are_reported() {
    let v = vault();
    let o = pkm(
        &v,
        &["material", "Bài viết/Không có"],
        &[("PKM_NO_OHS", "1")],
    );
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("không thấy bài"),
        "{o:?}"
    );
    let o = pkm(&v, &["material", "_Xưởng/Trang"], &[("PKM_NO_OHS", "1")]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("hệ thống"),
        "{o:?}"
    );
    let o = pkm(&v, &["material"], &[]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let db = v.root.join("không có.db");
    let o = pkm(
        &v,
        &["zones", "--db", db.to_str().unwrap()],
        &[("PKM_NO_OHS", "1")],
    );
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("chưa có index graph"),
        "{o:?}"
    );
}
