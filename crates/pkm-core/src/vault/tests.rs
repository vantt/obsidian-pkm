use std::fs;

use unicode_normalization::UnicodeNormalization;

use super::*;

fn link(key: &str, target: &str, frag: &str, origin: Origin, embed: bool) -> Link {
    Link {
        key: key.into(),
        target: target.into(),
        frag: frag.into(),
        origin,
        embed,
    }
}

fn props(p: &Parsed) -> Vec<(&str, Option<&str>)> {
    p.props
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_deref()))
        .collect()
}

/// `ListNotes.test_same_set_as_walk` của `scripts/tests/test_platform_paths.py`: bỏ thư mục chấm và
/// `.excalidraw.md`, khoá là NFC, mtime như `int(st_mtime * 1000)`.
#[test]
fn list_notes_same_set_as_walk() {
    let d = tempfile::tempdir().unwrap();
    let nfd: String = "Bài viết/d.md".nfd().collect();
    for rel in [
        "a.md",
        "x/b.md",
        "x/y/c.md",
        nfd.as_str(),
        ".obsidian/e.md",
        "x/.trash/f.md",
        "x/g.excalidraw.md",
        "x/h.txt",
    ] {
        let p = d.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "# t\n").unwrap();
    }
    let got = list_notes(d.path(), None).unwrap();
    assert_eq!(
        got.keys().map(String::as_str).collect::<Vec<_>>(),
        ["Bài viết/d.md", "a.md", "x/b.md", "x/y/c.md"]
    );
    let b = &got["x/b.md"];
    let real = d.path().join("x").join("b.md");
    assert_eq!(b.path, real);
    let meta = fs::metadata(&real).unwrap();
    assert_eq!(b.size, meta.len());
    let secs = meta
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    assert!((b.mtime as f64 - secs * 1000.0).abs() < 1.0);
}

#[test]
fn list_notes_applies_the_skip_rule() {
    let d = tempfile::tempdir().unwrap();
    fs::create_dir_all(d.path().join("_hệ thống")).unwrap();
    fs::write(d.path().join("_hệ thống/a.md"), "").unwrap();
    fs::write(d.path().join("b.md"), "").unwrap();
    let skip = |p: &str| p.starts_with("_hệ thống/");
    let got = list_notes(d.path(), Some(&skip)).unwrap();
    assert_eq!(got.keys().collect::<Vec<_>>(), ["b.md"]);
}

#[cfg(unix)]
#[test]
fn list_notes_skips_symlinked_folders() {
    let d = tempfile::tempdir().unwrap();
    fs::create_dir_all(d.path().join("real")).unwrap();
    fs::write(d.path().join("real/a.md"), "").unwrap();
    std::os::unix::fs::symlink(d.path().join("real"), d.path().join("link")).unwrap();
    let got = list_notes(d.path(), None).unwrap();
    assert_eq!(got.keys().collect::<Vec<_>>(), ["real/a.md"]);
}

/// `WindowsText.test_block_id_in_crlf_note` của `scripts/tests/test_platform_paths.py`.
#[test]
fn block_id_in_crlf_note() {
    let p = parse("Ý chính ^q1\r\nDòng sau ^q2\r\n");
    assert_eq!(p.blocks, ["q1", "q2"]);
}

#[test]
fn crlf_note_keeps_frontmatter_and_body() {
    let text = "---\r\ntype: note\r\nup: \"[[A]]\"\r\n---\r\nThân ^b1\r\n";
    let p = parse(text);
    assert_eq!(p.error, None);
    assert_eq!(props(&p), [("type", Some("note")), ("up", Some("[[A]]"))]);
    assert_eq!(p.blocks, ["b1"]);
    assert_eq!(p.links, [link("up", "A", "", Origin::Frontmatter, false)]);
}

#[test]
fn frontmatter_props_follow_vault_flat() {
    let p = parse(
        "---\ntype: claim\nvalid_from: 2026-01-05\nn: 3\nok: yes\nnone:\ntags: [a, [b, c], null]\n\
         meta: {who: An, when: 2026-01-05, deep: {x: 1}}\nsource: [\"[[Họp#^q1|nguồn]]\", \"[[B]] và [[C#h]]\"]\n---\n",
    );
    assert_eq!(p.error, None);
    assert_eq!(
        props(&p),
        [
            ("type", Some("claim")),
            ("valid_from", Some("2026-01-05")),
            ("n", Some("3")),
            ("ok", Some("true")),
            ("none", None),
            ("tags", Some("a")),
            ("tags", Some("b")),
            ("tags", Some("c")),
            ("tags", None),
            ("meta.who", Some("An")),
            ("meta.when", Some("2026-01-05")),
            ("meta.deep", Some("{'x': 1}")),
            ("source", Some("[[Họp#^q1|nguồn]]")),
            ("source", Some("[[B]] và [[C#h]]")),
        ]
    );
    assert_eq!(
        p.links,
        [
            link("source", "Họp", "#^q1", Origin::Frontmatter, false),
            link("source", "B", "", Origin::Frontmatter, false),
            link("source", "C", "#h", Origin::Frontmatter, false),
        ]
    );
    assert_eq!(p.first("type"), Some("claim"));
    assert_eq!(p.first("none"), None);
    assert_eq!(p.types(), [Some("claim")]);
}

#[test]
fn nested_dict_values_make_no_links() {
    let p = parse("---\nmeta: {up: \"[[A]]\"}\n---\n");
    assert_eq!(props(&p), [("meta.up", Some("[[A]]"))]);
    assert!(p.links.is_empty());
}

#[test]
fn frontmatter_errors() {
    assert_eq!(
        parse("---\ntype: note\nbad: [unclosed\n---\nthân [[A]]")
            .error
            .as_deref(),
        Some("YAML lỗi: while parsing a flow sequence")
    );
    let unclosed = parse("---\ntype: note\n[[A]]\n");
    assert_eq!(unclosed.error.as_deref(), Some("frontmatter không đóng"));
    assert_eq!(unclosed.links, [link("link", "A", "", Origin::Body, false)]);
    assert_eq!(
        parse("---\n- a\n---\n").error.as_deref(),
        Some("frontmatter không phải bảng khoá")
    );
    let empty = parse("---\n0\n---\n");
    assert_eq!((empty.error, empty.props.len()), (None, 0));
    let bom = parse("\u{feff}---\na: 1\n---\n");
    assert_eq!((bom.error, bom.props.len()), (None, 0));
}

/// Bốn dạng làm bản Python dừng cả lần index: Rust ghi lỗi cho riêng note, vẫn đọc thân note.
#[test]
fn forms_that_stop_python_become_a_yaml_error() {
    for (yaml, kind) in [
        ("b: !!bool maybe", "KeyError"),
        ("t: !!timestamp soon", "AttributeError"),
        ("i: !!int \"\"", "IndexError"),
        ("a: &r [1, *r]", "RecursionError"),
    ] {
        let p = parse(&format!("---\n{yaml}\n---\nthân [[A]] ^b"));
        assert_eq!(p.error, Some(format!("YAML lỗi: {kind}")), "{yaml}");
        assert!(p.props.is_empty());
        assert_eq!(p.links, [link("link", "A", "", Origin::Body, false)]);
        assert_eq!(p.blocks, ["b"]);
    }
}

#[test]
fn body_links_follow_spec_regexes() {
    let text = "---\na: 1\n---\n\
        [[Một]] ![[Ảnh.png]] [[Hai#Phần|hiện]] [[#Chỉ đoạn]] [[ Ba \\|x]]\n\
        ```\n[[Trong code]]\n```\n\
        ~~~js\n[[Cũng trong code]]\n~~~\n\
        [md](Thư%20mục/Note.md#h) ![ảnh](x.md) [web](https://a.b/c.md) [x](y.txt)\n\
        ![x [y](z.md)\n";
    let p = parse(text);
    assert_eq!(
        p.links,
        [
            link("link", "Một", "", Origin::Body, false),
            link("link", "Ảnh.png", "", Origin::Body, true),
            link("link", "Hai", "#Phần", Origin::Body, false),
            link("link", "", "#Chỉ đoạn", Origin::Body, false),
            link("link", "Ba \\", "", Origin::Body, false),
            link("link", "Thư mục/Note.md", "#h", Origin::BodyMd, false),
            link("link", "z.md", "", Origin::BodyMd, false),
        ]
    );
}

#[test]
fn blocks_are_sorted_unique_and_need_line_end() {
    let p =
        parse("a ^z9\nb ^a-1 \n^a-1\nc ^no more\nd^x\n```\n^in-code\n```\n\u{a0}^nb\n\u{1c}^fs");
    assert_eq!(p.blocks, ["a-1", "fs", "in-code", "nb", "z9"]);
}

#[test]
fn fold_ignores_case_marks_and_spacing() {
    assert_eq!(fold("  Công   ty ĐẠI Việt "), "cong ty dai viet");
    assert_eq!(fold("Cong ty ABC"), fold("Công ty ABC"));
    assert_eq!(fold("Straße"), "strasse");
}

#[test]
fn title_and_folder() {
    assert_eq!(title("Họp/Họp 2026-01-05.md"), "Họp 2026-01-05");
    assert_eq!(folder("Họp/Họp 2026-01-05.md"), "Họp");
    assert_eq!(folder("Gốc.md"), "");
}

#[test]
fn read_keeps_crlf_bom_and_replaces_bad_bytes() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.md");
    fs::write(&p, b"\xef\xbb\xbf---\r\na: caf\xe9\r\n").unwrap();
    assert_eq!(read(&p).unwrap(), "\u{feff}---\r\na: caf\u{fffd}\r\n");
}
