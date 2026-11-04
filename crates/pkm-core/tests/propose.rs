//! `pkm_core::propose` trên vault dựng sẵn.
//!
//! - `tests/propose/vault/`: vault bịa đủ bảy kiểu đề xuất và các ca bị loại (thư mục hệ thống, thư mục chấm,
//!   `.excalidraw.md`, nhúng, đính kèm, ngữ cảnh địa chỉ, danh sách tên). `expected.json` và `expected-notes.json`
//!   là đầu ra của `propose.proposals` và `propose.proposal_note(p, '2026-11-04')` của bản Python (`9061581`) trên
//!   chính vault đó và `similar.json`.
//! - kg-solo (`PKM_KG_SOLO=<thư mục>`, dựng như CI): so với `propose/kg-solo.json`, khớp đầu ra bản Python trên cùng
//!   vault. `PKM_WRITE_GOLDEN=1` ghi lại golden thay vì so.

use std::fs;
use std::path::{Path, PathBuf};

use pkm_core::propose::{self, Kind};
use serde_json::Value;

fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/propose")
}

fn read_json(p: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap()
}

fn fixture() -> Vec<propose::Proposal> {
    let similar =
        propose::parse_similar(&fs::read_to_string(here().join("similar.json")).unwrap()).unwrap();
    propose::proposals(&here().join("vault"), &similar).unwrap()
}

#[test]
fn fixture_proposals_match_python() {
    let got = Value::Array(fixture().iter().map(propose::Proposal::to_json).collect());
    assert_eq!(
        pretty(&got),
        pretty(&read_json(&here().join("expected.json")))
    );
}

#[test]
fn fixture_proposal_notes_match_python() {
    let want = read_json(&here().join("expected-notes.json"));
    let want = want.as_array().unwrap();
    let props = fixture();
    assert_eq!(props.len(), want.len());
    for (p, w) in props.iter().zip(want) {
        assert_eq!(
            propose::proposal_note(p, "2026-11-04"),
            w.as_str().unwrap(),
            "{:?}",
            p.kind
        );
    }
}

#[test]
fn fixture_covers_every_kind() {
    let props = fixture();
    for k in Kind::ALL {
        assert!(props.iter().any(|p| p.kind == k), "{k:?}");
    }
}

#[test]
fn kg_solo_matches_golden() {
    let Some(dir) = std::env::var_os("PKM_KG_SOLO") else {
        eprintln!("bỏ qua: chưa đặt PKM_KG_SOLO");
        return;
    };
    let props = propose::proposals(Path::new(&dir), &[]).unwrap();
    let got = Value::Array(props.iter().map(propose::Proposal::to_json).collect());
    let golden = here().join("kg-solo.json");
    if std::env::var_os("PKM_WRITE_GOLDEN").is_some() {
        fs::write(&golden, pretty(&got) + "\n").unwrap();
        return;
    }
    assert_eq!(pretty(&got), pretty(&read_json(&golden)));
}
