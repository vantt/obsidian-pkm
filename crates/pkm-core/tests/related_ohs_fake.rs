//! Tool `search` và `related` gieo bằng câu hỏi hay note, qua module `ohs`, với server giả `fake-ohs` (không cần
//! Node hay model). `fake-ohs` trả một kết quả có `path` là câu tìm ở dạng NFD, nên câu tìm là đường dẫn note muốn làm
//! hạt giống; cấu hình `hits` và `echo` của nó cho kết quả cố định và request nhận được.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use pkm_core::index::{self, Build};
use pkm_core::ohs::{Ohs, OhsConfig, OhsPackage};
use pkm_core::tools::{self, Context, Tools};
use serde_json::{Value, json};
use tempfile::TempDir;

const NOTES: [(&str, &str); 3] = [
    ("Khái niệm/Đà Nẵng.md", "type: concept\nup: [\"[[Gốc]]\"]"),
    ("Gốc.md", "type: concept"),
    ("Lá.md", "type: concept\nup: [\"[[Gốc]]\"]"),
];

struct Fixture {
    tmp: TempDir,
}

impl Fixture {
    fn new(fake: Value) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let f = Self { tmp };
        for (p, fm) in NOTES {
            let file = f.vault().join(p);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, format!("---\n{fm}\n---\n")).unwrap();
        }
        fs::write(f.vault().join("fake-ohs.json"), fake.to_string()).unwrap();
        let b = Build {
            packs: vec!["business".into()],
            ..Build::default()
        };
        index::build(&f.vault(), &f.db(), &b).expect("build");
        f
    }

    fn vault(&self) -> PathBuf {
        self.tmp.path().join("vault")
    }

    fn db(&self) -> PathBuf {
        self.tmp.path().join("graph.db")
    }

    fn ohs(&self) -> Ohs {
        let pkg = OhsPackage {
            root: self.tmp.path().to_path_buf(),
            node: PathBuf::from(env!("CARGO_BIN_EXE_fake-ohs")),
            cli: self.tmp.path().join("app/dist/src/cli.js"),
            pack: None,
        };
        let cfg = OhsConfig {
            ready_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(10),
            shutdown_timeout: Duration::from_secs(2),
            poll_interval: Duration::from_millis(50),
            ..OhsConfig::new(self.vault())
        };
        Ohs::start(pkg, cfg)
    }

    fn context(&self, ohs: Arc<Ohs>) -> Context {
        let t = Tools::open(&self.db()).expect("mở index");
        Context::new(t, &self.vault(), None).with_ohs(ohs)
    }
}

fn wait_ready(ohs: &Ohs) {
    let s = ohs.wait_ready(Duration::from_secs(10));
    assert!(s.is_ready(), "OHS giả không sẵn sàng: {s:?}");
}

fn ids(v: &Value, part: &str) -> Vec<String> {
    v[part]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn query_seeds_come_from_ohs_in_nfc() {
    let f = Fixture::new(Value::Null);
    let ohs = Arc::new(f.ohs());
    wait_ready(&ohs);
    let mut c = f.context(Arc::clone(&ohs));
    let r = tools::call(
        &mut c,
        "related",
        json!({"query": "Khái niệm/Đà Nẵng.md", "k": 5}),
    )
    .unwrap();
    assert_eq!(r["seed_source"], "query");
    assert_eq!(r["missing"], json!([]));
    assert_eq!(ids(&r, "seeds"), ["Khái niệm/Đà Nẵng.md"]);
    assert_eq!(r["seeds"][0]["ohs_rank"], 1);
    assert_eq!(r["seeds"][0]["snippet"], "trích Khái niệm/Đà Nẵng.md");
    assert!(r["related"][0].get("snippet").is_none());
    let related = ids(&r, "related");
    assert_eq!(related[0], "Gốc.md");
    assert!(related.contains(&"Lá.md".to_owned()), "{related:?}");
    assert_eq!(r["related"][0]["via"][0]["from"], "Khái niệm/Đà Nẵng.md");

    // Cùng kết quả như đưa hit thẳng vào (điểm 1.0 của OHS).
    let direct = tools::call(
        &mut c,
        "related",
        json!({"hits": [{"path": "Khái niệm/Đà Nẵng.md", "score": 1.0}], "k": 5}),
    )
    .unwrap();
    let strip = |mut v: Value| {
        v.as_object_mut().unwrap().remove("seed_source");
        for s in v["seeds"].as_array_mut().unwrap() {
            s.as_object_mut().unwrap().remove("snippet");
        }
        v
    };
    assert_eq!(strip(r), strip(direct));
}

#[test]
fn ohs_not_ready_while_indexing() {
    let f = Fixture::new(json!({
        "status": {"total": 10, "indexed": 4, "pending": 6, "model": "local:m",
                   "active_model": "local:m", "indexing_errors": []}
    }));
    let ohs = Arc::new(f.ohs());
    // OHS in `ready` nhưng index chưa xong: tool phải báo `indexing`, không tìm.
    let mut c = f.context(Arc::clone(&ohs));
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let r = loop {
        let r = tools::call(&mut c, "related", json!({"query": "Gốc.md"})).unwrap();
        if r["state"] == "indexing" || std::time::Instant::now() > deadline {
            break r;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(r["error"], "ohs_not_ready");
    assert_eq!(r["state"], "indexing");
    assert_eq!(r["progress"], json!(0.4));
    assert!(r.get("seeds").is_none());
}

#[test]
fn ohs_errors_and_unknown_paths_are_reported() {
    let f = Fixture::new(Value::Null);
    let ohs = Arc::new(f.ohs());
    wait_ready(&ohs);
    let mut c = f.context(Arc::clone(&ohs));
    let r = tools::call(&mut c, "related", json!({"query": "fail"})).unwrap();
    assert_eq!(r["error"], "ohs_error");
    assert!(r["detail"].as_str().unwrap().contains("search failed"));
    let r = tools::call(&mut c, "search", json!({"query": "fail"})).unwrap();
    assert_eq!(r["error"], "ohs_error");
    assert_eq!(r.as_object().unwrap().len(), 2);
    // OHS trả note không có trong index: như Python, `missing` và không có hạt giống.
    let r = tools::call(&mut c, "related", json!({"query": "Không có.md"})).unwrap();
    assert_eq!(
        r,
        json!({"seeds": [], "related": [], "missing": ["Không có.md"], "seed_source": "query"})
    );
}

#[test]
fn context_without_ohs_reports_broken() {
    let f = Fixture::new(Value::Null);
    let t = Tools::open(&f.db()).unwrap();
    let mut c = Context::new(t, Path::new(&f.vault()), None);
    let r = tools::call(&mut c, "related", json!({"query": "Gốc.md"})).unwrap();
    assert_eq!(r["error"], "ohs_not_ready");
    assert_eq!(r["state"], "broken");
    assert_eq!(r["progress"], Value::Null);
    assert_eq!(r["eta_s"], Value::Null);
}

/// `search` gửi tham số theo tên stdio của OHS (`snippetLength`, `notePath`, `queries` khi có từ hai câu) và trả
/// kết quả gọn: id NFC, title và type của index, điểm làm tròn, snippet; bỏ mọi trường khác của OHS.
#[test]
fn search_maps_arguments_to_stdio_and_trims_results() {
    let f = Fixture::new(json!({"echo": true, "hits": ["Khái niệm/Đà Nẵng.md", "Không có.md"]}));
    let ohs = Arc::new(f.ohs());
    wait_ready(&ohs);
    let mut c = f.context(Arc::clone(&ohs));
    let r = tools::call(
        &mut c,
        "search",
        json!({"query": "đà nẵng", "queries": ["da nang"], "mode": "semantic", "scope": "Khái niệm/",
               "tag": ["a", "-b"], "limit": 2, "snippet_length": 50}),
    )
    .unwrap();
    assert_eq!(r["count"], 2);
    assert_eq!(r["mode"], "semantic");
    let it = &r["items"][0];
    let keys: Vec<&str> = it.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["id", "title", "type", "score", "snippet"]);
    assert_eq!(it["id"], "Khái niệm/Đà Nẵng.md");
    assert_eq!(it["title"], "Đà Nẵng");
    assert_eq!(it["type"], "concept");
    assert_eq!(r["items"][1]["score"], 0.9);
    // Note không có trong index: giữ, title của OHS, không có type.
    assert_eq!(r["items"][1]["id"], "Không có.md");
    assert_eq!(r["items"][1]["title"], "tiêu đề OHS");
    assert_eq!(r["items"][1]["type"], Value::Null);
    let sent: Value = serde_json::from_str(it["snippet"].as_str().unwrap()).unwrap();
    assert_eq!(
        sent,
        json!({"query": "đà nẵng", "options": {"mode": "semantic", "scope": ["Khái niệm/"], "tag": ["a", "-b"],
               "limit": 2, "snippetLength": 50, "queries": ["đà nẵng", "da nang"]}})
    );

    // `path`: câu gửi đi là note id, kèm `notePath`; mặc định limit 10, snippet 200, hybrid.
    let r = tools::call(&mut c, "search", json!({"path": "Gốc.md"})).unwrap();
    let sent: Value = serde_json::from_str(r["items"][0]["snippet"].as_str().unwrap()).unwrap();
    assert_eq!(
        sent,
        json!({"query": "Gốc.md", "options": {"mode": "hybrid", "limit": 10, "snippetLength": 200,
               "notePath": "Gốc.md"}})
    );
}

/// Trang hệ thống bị bỏ khỏi `search` trừ khi `system`, như các tool khác.
#[test]
fn search_skips_system_pages_unless_asked() {
    let f = Fixture::new(json!({"hits": ["Gốc.md", "Lá.md"]}));
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('hidden', '[\"Gốc.md\"]')",
        [],
    )
    .unwrap();
    drop(conn);
    let ohs = Arc::new(f.ohs());
    wait_ready(&ohs);
    let mut c = f.context(Arc::clone(&ohs));
    let r = tools::call(&mut c, "search", json!({"query": "x"})).unwrap();
    assert_eq!(ids(&r, "items"), ["Lá.md"]);
    let r = tools::call(&mut c, "search", json!({"query": "x", "system": true})).unwrap();
    assert_eq!(ids(&r, "items"), ["Gốc.md", "Lá.md"]);
}

/// OHS vừa khởi động (`starting`): `search` chờ một lúc rồi tìm, không trả `ohs_not_ready` ngay.
#[test]
fn search_waits_while_ohs_is_starting() {
    let empty = json!({"status": {"total": 0, "indexed": 0, "pending": 0, "model": null,
                                  "active_model": "local:m", "indexing_errors": []}});
    let f = Fixture::new(empty);
    let ohs = Arc::new(f.ohs());
    let s = ohs.wait_ready(Duration::from_secs(1));
    assert_eq!(s.name(), "starting");
    let ready = f.vault().join("fake-ohs.json");
    let later = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        fs::write(ready, "{}").unwrap();
    });
    let mut c = f.context(Arc::clone(&ohs));
    let r = tools::call(&mut c, "search", json!({"query": "Gốc.md"})).unwrap();
    later.join().unwrap();
    assert_eq!(ids(&r, "items"), ["Gốc.md"], "{r}");
}

/// `related(node)`: OHS tìm note giống nghĩa với `notePath` là note đó; `seed_source` là `node`.
#[test]
fn node_seeds_come_from_ohs_similar_notes() {
    let f = Fixture::new(json!({"echo": true, "hits": ["Lá.md"]}));
    let ohs = Arc::new(f.ohs());
    wait_ready(&ohs);
    let mut c = f.context(Arc::clone(&ohs));
    let r = tools::call(&mut c, "related", json!({"node": "Gốc.md", "k": 5})).unwrap();
    assert_eq!(r["seed_source"], "node");
    assert_eq!(ids(&r, "seeds"), ["Lá.md"]);
    let sent: Value = serde_json::from_str(r["seeds"][0]["snippet"].as_str().unwrap()).unwrap();
    assert_eq!(
        sent,
        json!({"query": "Gốc.md", "options": {"notePath": "Gốc.md", "limit": 10, "snippetLength": 160}})
    );
}
