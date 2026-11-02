//! Bộ so khớp trên golden đã commit: không cần Python, chạy được trong CI.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use pkm_parity::compare::{DiffKind, Status, compare};
use pkm_parity::dump::Dump;
use pkm_parity::exceptions;
use pkm_parity::report::{self, exit_code};
use pkm_parity::{golden_dir, pinned_spec_sha, rust_side};

fn golden() -> PathBuf {
    golden_dir(&pinned_spec_sha().expect("đọc được spec/spec.lock"))
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}

/// Bản sao golden trong `dir/edited` với đúng một dòng của `file` đã sửa: dòng đầu tiên chứa cả
/// `select` và `needle`, thêm `X` ngay sau `needle` (đặt `needle` ngay trước một giá trị chuỗi để JSON
/// vẫn hợp lệ). `needle` rỗng: thêm ` X` vào cuối dòng. Trả thư mục bản sao và số dòng đã sửa.
fn edited_copy(dir: &Path, file: &str, select: &str, needle: &str) -> (PathBuf, usize) {
    let copy = dir.join("edited");
    copy_dir(&golden(), &copy);
    let path = copy.join(file);
    let text = fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let i = lines
        .iter()
        .position(|l| l.contains(select) && l.contains(needle) && !l.trim().is_empty())
        .unwrap_or_else(|| panic!("{file}: không có dòng chứa {select:?} và {needle:?}"));
    lines[i] = if needle.is_empty() {
        format!("{} X", lines[i])
    } else {
        lines[i].replacen(needle, &format!("{needle}X"), 1)
    };
    fs::write(&path, lines.join("\n")).unwrap();
    (copy, i + 1)
}

#[test]
fn golden_has_every_layer_and_matches_itself() {
    let g = Dump::open(&golden()).unwrap();
    assert_eq!(g.manifest.implementation, "python");
    assert_eq!(
        g.manifest.spec_sha.as_deref(),
        Some(pinned_spec_sha().unwrap().as_str())
    );
    for t in [
        "nodes",
        "props",
        "links",
        "raw_links",
        "blocks",
        "names",
        "edges",
        "meta",
        "v3_deviations",
    ] {
        assert!(g.manifest.tables.contains_key(t), "golden thiếu bảng {t}");
    }
    assert!(
        g.manifest
            .tables
            .keys()
            .filter(|t| t.starts_with("schema_"))
            .count()
            >= 10
    );
    for f in [
        "answers/answers.jsonl",
        "tools/calls.jsonl",
        "mcp/initialize.json",
        "mcp/tools_list.json",
        "mcp/instructions.txt",
        "cli/commands.json",
    ] {
        assert!(g.exists(f), "golden thiếu {f}");
    }
    assert!(
        !g.exists("perf.json"),
        "golden không ghi số đo hiệu năng (phụ thuộc máy)"
    );
    let cmp = compare(&g, &g, &[]).unwrap();
    assert_eq!(cmp.unnamed(), 0);
    assert_eq!(cmp.not_available(), 0);
    let tools: Vec<&str> = cmp
        .items
        .iter()
        .filter(|i| i.layer == "tools")
        .map(|i| i.item.as_str())
        .collect();
    for t in [
        "find",
        "note",
        "neighbors",
        "reach",
        "paths",
        "loops",
        "provenance",
        "select",
        "valid",
        "more",
        "related",
    ] {
        assert!(tools.contains(&t), "bộ câu tool thiếu {t}");
    }
}

#[test]
fn editing_one_golden_line_points_at_exactly_that_line() {
    // (file, dòng chứa, chỗ sửa, tầng, mục)
    let cases = [
        (
            "tables/nodes.jsonl",
            "\"type\": \"project\"",
            "\"title\": \"",
            "index",
            "nodes",
        ),
        (
            "tables/props.jsonl",
            "\"key\": \"type\"",
            "\"value\": \"",
            "index",
            "props",
        ),
        (
            "tables/edges.jsonl",
            "\"origin\": \"claim\"",
            "\"rel\": \"",
            "index",
            "edges",
        ),
        (
            "tools/calls.jsonl",
            "\"tool\": \"neighbors\"",
            "\"status\": \"",
            "tools",
            "neighbors",
        ),
        (
            "tools/calls.jsonl",
            "\"tool\": \"related\"",
            "\"found_by\": \"",
            "tools",
            "related",
        ),
        (
            "answers/answers.jsonl",
            "\"op\": \"cycles\"",
            "\"kind\": \"",
            "answers",
            "cycles",
        ),
        (
            "mcp/tools_list.json",
            "\"description\": \"",
            "\"description\": \"",
            "mcp",
            "tools_list",
        ),
        (
            "mcp/instructions.txt",
            "Note types",
            "",
            "mcp",
            "instructions",
        ),
        (
            "cli/stats.out",
            "\"options\": \"",
            "\"options\": \"",
            "cli",
            "stats",
        ),
        ("cli/validate.out", "R", "", "cli", "validate"),
    ];
    let g = Dump::open(&golden()).unwrap();
    for (file, select, needle, layer, item) in cases {
        let tmp = tempfile::tempdir().unwrap();
        let (dir, line) = edited_copy(tmp.path(), file, select, needle);
        let copy = Dump::open(&dir).unwrap();
        let cmp = compare(&copy, &g, &[]).unwrap();
        let diffs: Vec<_> = cmp
            .items
            .iter()
            .flat_map(|i| i.diffs.iter().map(move |d| (i, d)))
            .collect();
        assert_eq!(diffs.len(), 1, "{file}:{line}: {diffs:#?}");
        let (i, d) = diffs[0];
        assert_eq!((i.layer, i.item.as_str()), (layer, item), "{file}:{line}");
        assert_eq!(
            (d.file.as_str(), d.line, d.kind),
            (file, Some(line), DiffKind::Changed),
            "{file}:{line}"
        );
        assert_eq!(exit_code(&cmp, false), 1);
    }
}

#[test]
fn named_exception_turns_a_diff_into_a_named_one() {
    let tmp = tempfile::tempdir().unwrap();
    let (dir, _) = edited_copy(
        tmp.path(),
        "tools/calls.jsonl",
        "\"tool\": \"neighbors\"",
        "\"type\": \"",
    );
    let copy = Dump::open(&dir).unwrap();
    let g = Dump::open(&golden()).unwrap();
    let exc = tmp.path().join("exceptions.toml");
    let porting = tmp.path().join("PORTING.md");
    fs::write(
        &exc,
        "[[exception]]\nname = \"type-co-x\"\nlayer = \"tools\"\nitem = \"neighbors\"\n\
                     fields = [\"/result/items/*/type\"]\nreason = \"thử\"\n",
    )
    .unwrap();
    fs::write(&porting, "# Nhật ký\n\n## Lệch có chủ ý\n\n### type-co-x\n").unwrap();
    let list = exceptions::load(&exc, &porting).unwrap();
    let cmp = compare(&copy, &g, &list).unwrap();
    assert_eq!(cmp.unnamed(), 0);
    assert_eq!(cmp.items.iter().map(|i| i.named()).sum::<usize>(), 1);
    assert_eq!(exit_code(&cmp, false), 0);
}

/// Vault giả cùng tên và cùng tree hash với golden (bản Rust chưa đọc nội dung note).
fn fake_vault(dir: &Path, g: &Dump) -> PathBuf {
    let vault = dir.join(&g.manifest.vault.name);
    fs::create_dir_all(&vault).unwrap();
    let hash = g
        .manifest
        .vault
        .fixture
        .as_ref()
        .and_then(|f| f.tree_hash.clone())
        .unwrap();
    fs::write(
        vault.join(".fixture-manifest.json"),
        format!("{{\"treeHash\": \"{hash}\"}}"),
    )
    .unwrap();
    vault
}

#[test]
fn rust_side_reports_every_missing_layer_as_not_available_never_as_match() {
    let tmp = tempfile::tempdir().unwrap();
    let g = Dump::open(&golden()).unwrap();
    let vault = fake_vault(tmp.path(), &g);
    rust_side::dump(&vault, &g, &tmp.path().join("rust")).unwrap();
    let r = Dump::open(&tmp.path().join("rust")).unwrap();
    let cmp = compare(&g, &r, &[]).unwrap();
    assert!(!cmp.items.is_empty());
    for i in &cmp.items {
        assert_eq!(i.status, Status::NotAvailable, "{}/{}", i.layer, i.item);
    }
    assert!(cmp.perf.is_none());
    assert_eq!(exit_code(&cmp, false), 0);
    assert_eq!(exit_code(&cmp, true), 1, "--strict: còn mục chưa có là lỗi");
}

#[test]
fn rust_side_refuses_a_different_vault() {
    let tmp = tempfile::tempdir().unwrap();
    let g = Dump::open(&golden()).unwrap();
    let vault = fake_vault(tmp.path(), &g);
    fs::write(
        vault.join(".fixture-manifest.json"),
        "{\"treeHash\": \"khac\"}",
    )
    .unwrap();
    assert!(rust_side::dump(&vault, &g, &tmp.path().join("rust")).is_err());
    let other = tmp.path().join("vault-khac");
    fs::create_dir_all(&other).unwrap();
    assert!(rust_side::dump(&other, &g, &tmp.path().join("rust2")).is_err());
}

#[test]
fn private_report_has_counts_only() {
    let tmp = tempfile::tempdir().unwrap();
    let (dir, _) = edited_copy(tmp.path(), "tables/nodes.jsonl", "", "\"title\": \"");
    let copy = Dump::open(&dir).unwrap();
    let g = Dump::open(&golden()).unwrap();
    let cmp = compare(&copy, &g, &[]).unwrap();
    let ctx = report::Context {
        expected: &copy,
        actual: &g,
        private: true,
        strict: false,
    };
    let json = report::to_json(&cmp, &ctx).to_string();
    let text = report::summary(&cmp, &ctx);
    let nodes = &cmp.items.iter().find(|i| i.item == "nodes").unwrap();
    assert_eq!(nodes.unnamed(), 1);
    for out in [&json, &text] {
        assert!(!out.contains(".md"), "báo cáo riêng tư lộ tên note: {out}");
        assert!(
            !out.contains("details"),
            "báo cáo riêng tư có chi tiết lệch"
        );
        assert!(
            !out.contains(&g.manifest.vault.name),
            "báo cáo riêng tư lộ tên vault"
        );
    }
}

#[test]
fn binary_exit_codes() {
    let bin = env!("CARGO_BIN_EXE_parity");
    let tmp = tempfile::tempdir().unwrap();
    let (copy, line) = edited_copy(
        tmp.path(),
        "tables/meta.jsonl",
        "\"key\": \"notes_read\"",
        "\"value\": \"",
    );
    let report = tmp.path().join("report.json");
    let run = |a: &Path, b: &Path| {
        Command::new(bin)
            .args([
                "compare".as_ref(),
                a.as_os_str(),
                b.as_os_str(),
                "--report".as_ref(),
                report.as_os_str(),
            ])
            .output()
            .unwrap()
    };
    let same = run(&golden(), &golden());
    assert_eq!(
        same.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&same.stdout)
    );
    let edited = run(&copy, &golden());
    assert_eq!(edited.status.code(), Some(1));
    let out = String::from_utf8_lossy(&edited.stdout);
    assert!(out.contains(&format!("tables/meta.jsonl:{line}")), "{out}");
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(saved["summary"]["unnamed_diffs"], 1);
    let broken = run(&tmp.path().join("khong-co"), &golden());
    assert_eq!(broken.status.code(), Some(2));
}
