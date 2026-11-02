//! Ca biên YAML của bài thử S1 (`probes/s1-yaml/cases/`) với kết quả PyYAML đã ghi lại (`CSafeLoader`, PyYAML 6.0.3,
//! Python 3.14): `vault::parse` phải cho cùng `props` và cùng lỗi YAML, không cần Python.
//!
//! Bốn ca mà bản Python dừng hẳn (`crash`) thì Rust ghi lỗi `YAML lỗi: <tên ngoại lệ>` cho note
//! (spec/PORTING.md, "Lệch có chủ ý").

use std::collections::BTreeMap;
use std::path::Path;

use pkm_core::vault;
use serde_json::{Value, json};

fn reference() -> (Value, BTreeMap<String, Value>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let text = std::fs::read_to_string(dir.join("yaml-cases.pyyaml.jsonl")).unwrap();
    let mut lines = text
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap());
    let meta = lines.next().unwrap()["meta"].clone();
    let notes = lines
        .map(|v| (v["path"].as_str().unwrap().to_owned(), v))
        .collect();
    (meta, notes)
}

#[test]
fn every_edge_case_matches_pyyaml() {
    let (meta, reference) = reference();
    assert_eq!(meta["loader"], "CSafeLoader");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/yaml-cases");
    let notes = vault::list_notes(&dir, None).unwrap();
    assert_eq!(
        notes.keys().collect::<Vec<_>>(),
        reference.keys().collect::<Vec<_>>(),
        "mỗi ca có một bản ghi PyYAML"
    );
    let mut crashes = 0;
    for (rel, file) in &notes {
        let parsed = vault::parse(&vault::read(&file.path).unwrap());
        let py = &reference[rel];
        let props: Vec<Value> = parsed.props.iter().map(|(k, v)| json!([k, v])).collect();
        if let Some(kind) = py["crash"].as_str() {
            crashes += 1;
            assert_eq!(parsed.error, Some(format!("YAML lỗi: {kind}")), "{rel}");
            assert!(props.is_empty(), "{rel}");
            continue;
        }
        assert_eq!(json!(parsed.error), py["error"], "{rel}: lỗi");
        assert_eq!(json!(props), py["props"], "{rel}: props");
    }
    assert_eq!(crashes, 4);
}
