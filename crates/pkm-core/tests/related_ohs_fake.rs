//! Tool `related` chỉ có câu hỏi, hạt giống lấy từ OHS qua module `ohs`, với server giả `fake-ohs` (không cần Node
//! hay model). `fake-ohs` trả một kết quả có `path` là câu tìm ở dạng NFD, nên câu tìm là đường dẫn note muốn làm hạt
//! giống.

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
    assert_eq!(r["seed_source"], "ohs-cli");
    assert_eq!(r["missing"], json!([]));
    assert_eq!(ids(&r, "seeds"), ["Khái niệm/Đà Nẵng.md"]);
    assert_eq!(r["seeds"][0]["ohs_rank"], 1);
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
    assert_eq!(
        r["error"],
        "no OHS index reachable from this server; call the OHS search tool first and pass its results as hits"
    );
    assert!(r["detail"].as_str().unwrap().contains("search failed"));
    // OHS trả note không có trong index: như Python, `missing` và không có hạt giống.
    let r = tools::call(&mut c, "related", json!({"query": "Không có.md"})).unwrap();
    assert_eq!(
        r,
        json!({"seeds": [], "related": [], "missing": ["Không có.md"], "seed_source": "ohs-cli"})
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
