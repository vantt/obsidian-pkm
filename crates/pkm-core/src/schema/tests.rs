use super::*;

fn table<'a>(ts: &'a [Table], name: &str) -> &'a Table {
    ts.iter().find(|t| t.name == name).expect(name)
}

fn text(c: &Cell) -> Option<&str> {
    match c {
        Cell::Text(s) => Some(s),
        _ => None,
    }
}

fn custom(yaml: &str, packs: &[&str]) -> Result<(Schema, Vec<Table>), SchemaError> {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("schema.yaml");
    std::fs::write(&p, yaml).unwrap();
    load(&Source::File(p), packs)
}

const MINI: &str = "\
schema_version: 9
enums: {status: [proposed, accepted], n: [1, 2.5, true]}
edge_groups: {causal: [increases], hierarchy: [up]}
properties_common: {required: [type], optional: [aliases, type], fields: {aliases: {cardinality: many}}}
node_types:
  factor:
  claim:
    required: [relation, status]
    required_if:
      - when: {relation_group: causal}
        then: [cause, effect]
      - when: {mutual: true}
        then: [why]
    fields:
      relation: {enum_of_edges: [causal, facts]}
      status: {enum: status}
      source: {link_to: [note]}
  note: {extends: page}
  a: {extends: b}
  b: {extends: a}
edges:
  increases: {domain: [factor], range: [factor], polarity: '+', symmetric: yes}
  up: {domain: [note], range: [ghost], excludes_with: [down], block_ref: allowed}
  source: {domain: ['*'], range: [note]}
packs:
  extra:
    node_types: {page: {}, doc: {extends: page}}
    edges: {about: {domain: ['*'], range: [page], group: causal}}
    facts: {works_at: {domain: [factor], range: [factor]}}
    widen: {source: {range: [page]}, missing: {range: [x]}}
    lifecycle_types: [doc]
";

#[test]
fn embedded_schema_is_the_pinned_file() {
    assert_eq!(digest(&Source::Embedded).unwrap(), "bbaa9f0197ef");
    let doc = read_schema(&Source::Embedded).unwrap();
    assert_eq!(
        doc.root().get("updated").unwrap().py_repr().unwrap(),
        "datetime.date(2026, 10, 29)"
    );
}

#[test]
fn business_pack_loads_without_problems() {
    let (s, ts) = load(&Source::Embedded, &["business"]).unwrap();
    assert_eq!(s.version, "3");
    assert!(s.problems.is_empty(), "{:?}", s.problems);
    assert_eq!(s.types["contract"].extends.as_deref(), Some("document"));
    assert_eq!(s.edges["works_at"].kind, EdgeKind::Fact);
    assert!(s.edges["source"].range.contains(&"meeting".to_owned()));
    assert_eq!(
        s.claim_ends.get("works_at"),
        Some(&("subject".to_owned(), "object".to_owned()))
    );
    assert_eq!(
        s.claim_ends.get("increases"),
        Some(&("cause".to_owned(), "effect".to_owned()))
    );
    assert_eq!(
        ts.iter().map(|t| t.name).collect::<Vec<_>>(),
        TABLES.to_vec()
    );
    let ancestor = table(&ts, "schema_type_ancestor");
    assert!(
        ancestor
            .rows
            .contains(&vec!["contract".into(), "document".into()])
    );
}

#[test]
fn core_only_and_capture_pack() {
    let (s, _) = load(&Source::Embedded, &[]).unwrap();
    assert!(!s.types.contains_key("organization"));
    let (s, ts) = load(&Source::Embedded, &["capture"]).unwrap();
    assert!(s.problems.is_empty(), "{:?}", s.problems);
    assert_eq!(s.group_of["topic"], "navigation");
    let wise = table(&ts, "schema_enum")
        .rows
        .iter()
        .filter(|r| text(&r[0]) == Some("wise"))
        .count();
    assert_eq!(wise, 7);
}

#[test]
fn unknown_pack_is_an_error() {
    assert_eq!(
        load(&Source::Embedded, &["business", "nope", "x"]).unwrap_err(),
        SchemaError("gói không có trong schema.yaml: nope, x".into())
    );
}

#[test]
fn custom_schema_follows_schema_py() {
    let (s, ts) = custom(MINI, &["extra"]).unwrap();
    assert_eq!(s.version, "9");
    assert_eq!(s.enums["n"], ["1", "2.5", "True"]);
    assert_eq!(
        s.problems,
        [
            "extends của a có vòng",
            "extends của b có vòng",
            "up.range: type ghost không có (gói chưa bật?)",
            "up.excludes_with: không có cạnh down",
        ]
    );
    assert_eq!(s.edges["source"].range, ["note", "page"]);
    assert_eq!(s.group_of["about"], "causal");
    assert_eq!(s.groups["facts"], ["works_at"]);

    let edge = table(&ts, "schema_edge");
    let up = edge
        .rows
        .iter()
        .find(|r| text(&r[0]) == Some("up"))
        .unwrap();
    assert_eq!(
        up,
        &vec![
            "up".into(),
            Cell::Null,
            "edge".into(),
            "hierarchy".into(),
            Cell::Null,
            Cell::Null,
            Cell::Int(0),
            Cell::Null,
            Cell::Int(0),
            Cell::Int(1),
        ]
    );
    let inc = edge
        .rows
        .iter()
        .find(|r| text(&r[0]) == Some("increases"))
        .unwrap();
    assert_eq!((&inc[6], text(&inc[7])), (&Cell::Int(1), Some("+")));

    // `relation` lấy tên cạnh của các nhóm causal và facts.
    let rel: Vec<_> = table(&ts, "schema_enum")
        .rows
        .iter()
        .filter(|r| text(&r[0]) == Some("relation"))
        .map(|r| text(&r[1]).unwrap())
        .collect();
    assert_eq!(rel, ["increases", "about", "works_at"]);

    // Fact không vào schema_link nhưng vào schema_claim_rel; trường `source` của claim nhận cả phần gói nới.
    let link = table(&ts, "schema_link");
    assert!(!link.rows.iter().any(|r| text(&r[0]) == Some("works_at")));
    assert!(
        table(&ts, "schema_claim_rel")
            .rows
            .iter()
            .any(|r| text(&r[0]) == Some("works_at"))
    );
    let src: Vec<_> = link
        .rows
        .iter()
        .filter(|r| text(&r[0]) == Some("source") && text(&r[1]) == Some("claim"))
        .map(|r| text(&r[3]).unwrap())
        .collect();
    assert_eq!(src, ["note", "note", "page"]);

    let req: Vec<_> = table(&ts, "schema_required_if")
        .rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| text(c).unwrap())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    assert_eq!(
        req,
        [
            "claim relation increases cause",
            "claim relation increases effect",
            "claim relation about cause",
            "claim relation about effect",
            "claim mutual True why",
        ]
    );

    let fields = table(&ts, "schema_field");
    let common_type = fields
        .rows
        .iter()
        .filter(|r| text(&r[0]) == Some("*") && text(&r[1]) == Some("type"))
        .count();
    assert_eq!(common_type, 2, "optional + required giữ trùng như Python");
    let rel_field = fields
        .rows
        .iter()
        .find(|r| text(&r[0]) == Some("claim") && text(&r[1]) == Some("relation"))
        .unwrap();
    assert_eq!(text(&rel_field[2]), Some("relation"));
    assert_eq!(rel_field[5], Cell::Int(1));
    let doc_fields: Vec<_> = fields
        .rows
        .iter()
        .filter(|r| text(&r[0]) == Some("doc"))
        .map(|r| text(&r[1]).unwrap())
        .collect();
    assert_eq!(doc_fields, ["valid_from", "valid_to", "status"]);

    let anc = table(&ts, "schema_type_ancestor");
    assert!(anc.rows.contains(&vec!["doc".into(), "page".into()]));
    assert!(anc.rows.contains(&vec!["a".into(), "b".into()]));
    assert_eq!(table(&ts, "schema_problems").rows.len(), 4);
}

#[test]
fn malformed_schema_is_an_error_not_a_panic() {
    for (yaml, want) in [
        ("[1]", "gốc"),
        ("enums: {}\n", "edge_groups"),
        ("a: [unclosed", "YAML lỗi"),
        ("", "rỗng"),
    ] {
        let e = custom(yaml, &[]).unwrap_err();
        assert!(e.0.contains(want), "{yaml:?}: {e}");
    }
    let no_domain = MINI.replace(
        "{domain: [factor], range: [factor], polarity",
        "{range: [factor], polarity",
    );
    assert!(custom(&no_domain, &[]).unwrap_err().0.contains("domain"));
}

#[test]
fn real_number_text_follows_sqlite() {
    for (f, want) in [
        (1.0, "1.0"),
        (2.5, "2.5"),
        (0.1, "0.1"),
        (100.0, "100.0"),
        (1e20, "1.0e+20"),
        (1.5e-7, "1.5e-07"),
        (123456789012345.0, "123456789012345.0"),
    ] {
        assert_eq!(sqlite_real_text(f), want, "{f}");
    }
}
