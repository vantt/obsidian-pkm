//! Mọi bảng của index SQLite mà `pkm-core::index` dựng, và view `v3_deviations`: build đủ rồi build tăng dần không
//! đổi gì vào một DB tạm, đọc bảng qua `pkm_core::index::dump_tables` (bộ so khớp không mở SQLite riêng), chuẩn hoá
//! như `dump-python.py` (`nodes.mtime` thành độ lệch, `meta` thay giá trị đổi theo lần chạy, đường dẫn tuyệt đối thay
//! bằng `<VAULT>`, `<DB>`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use pkm_core::index::{self, Build, EdgeKeys, Options};
use pkm_core::vault;
use serde_json::{Map, Value, json};

/// Khoá `meta` đổi theo lần chạy (như `VOLATILE_META` của Python).
const VOLATILE_META: [&str; 2] = ["built_at", "ms"];
const VOLATILE: &str = "<volatile>";

/// Bảng đã dump và số đo thời gian build. Index giữ lại trong thư mục tạm (`db`) cho tầng câu đáp án và tool, như
/// `dump-python.py` chạy các tầng đó trên cùng DB; xoá khi `Built` bị bỏ.
pub struct Built {
    pub tables: HashMap<String, Vec<Map<String, Value>>>,
    pub perf: Value,
    pub db: PathBuf,
    pub scrub: Scrub,
    _tmp: TempDir,
}

/// Thư mục tạm, xoá khi ra khỏi phạm vi.
pub(super) struct TempDir(pub(super) PathBuf);

impl TempDir {
    pub(super) fn new() -> Result<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let p = std::env::temp_dir().join(format!("pkm-parity-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&p).with_context(|| format!("không tạo được {}", p.display()))?;
        Ok(TempDir(p))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Cách đọc vault từ `run` của dump mẫu.
pub(super) fn read_run(run: &Value) -> Result<(Vec<String>, String, Options)> {
    let s = |k: &str| run.get(k).and_then(Value::as_str).unwrap_or_default();
    let b = |k: &str| run.get(k).and_then(Value::as_bool).unwrap_or(false);
    let packs = s("packs")
        .split(',')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect();
    let edge_keys = match s("edge_keys") {
        "" => EdgeKeys::Schema,
        k => EdgeKeys::parse(k).with_context(|| format!("run.edge_keys lạ: {k:?}"))?,
    };
    let opts = Options {
        aliases: !b("no_aliases"),
        edge_keys,
        body_edges: b("body_edges"),
    };
    Ok((packs, s("adapter").to_owned(), opts))
}

/// Thay đường dẫn tuyệt đối (cả bản đã giải symlink) bằng dấu; đường dài trước.
pub struct Scrub(Vec<(String, &'static str)>);

impl Scrub {
    pub(super) fn new(vault: &Path, db: &Path) -> Self {
        let mut pairs = Vec::new();
        for (p, mark) in [(db, "<DB>"), (vault, "<VAULT>")] {
            for q in [std::path::absolute(p).ok(), std::fs::canonicalize(p).ok()]
                .into_iter()
                .flatten()
            {
                let q = q.to_string_lossy().into_owned();
                if !pairs.iter().any(|(x, _)| *x == q) {
                    pairs.push((q, mark));
                }
            }
        }
        pairs.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
        Scrub(pairs)
    }

    pub fn text(&self, s: &str) -> String {
        self.0
            .iter()
            .fold(s.to_owned(), |acc, (p, mark)| acc.replace(p.as_str(), mark))
    }

    /// Mọi chuỗi (khoá và giá trị) trong một giá trị JSON.
    pub fn value(&self, v: Value) -> Value {
        match v {
            Value::String(s) => Value::String(self.text(&s)),
            Value::Array(a) => Value::Array(a.into_iter().map(|x| self.value(x)).collect()),
            Value::Object(m) => Value::Object(
                m.into_iter()
                    .map(|(k, x)| (self.text(&k), self.value(x)))
                    .collect(),
            ),
            v => v,
        }
    }
}

/// Build đủ rồi build tăng dần trên `vault`, dump mọi bảng.
pub fn build(vault_dir: &Path, run: &Value) -> Result<Built> {
    let (packs, adapter_spec, opts) = read_run(run)?;
    let adapter = pkm_core::adapter::load(&adapter_spec)?;
    let tmp = TempDir::new()?;
    let db_path = tmp.0.join("graph.db");
    let mut b = Build {
        packs,
        opts,
        full: true,
        adapter: adapter.as_deref(),
        ..Build::default()
    };
    let t0 = Instant::now();
    index::build(vault_dir, &db_path, &b).context("build đủ lỗi")?;
    let full_ms = t0.elapsed().as_secs_f64() * 1000.0;
    b.full = false;
    let t0 = Instant::now();
    let info = index::build(vault_dir, &db_path, &b).context("build tăng dần lỗi")?;
    let noop_ms = t0.elapsed().as_secs_f64() * 1000.0;
    if info.get("notes_read") != Some("0") {
        bail!(
            "build lần hai đọc lại {} note: vault đang bị sửa trong lúc dump?",
            info.get("notes_read").unwrap_or_default()
        );
    }

    let scrub = Scrub::new(vault_dir, &db_path);
    let db = index::connect(&db_path, true)?;
    let mut tables = HashMap::new();
    for t in index::dump_tables(&db)? {
        let mut rows = Vec::with_capacity(t.rows.len());
        for r in t.rows {
            let mut m: Map<String, Value> = t.columns.iter().cloned().zip(r).collect();
            if t.name == "nodes"
                && let (Some(Value::String(id)), Some(Value::Number(mt))) =
                    (m.get("id"), m.get("mtime"))
                && let Some(mt) = mt.as_i64()
            {
                let p = vault_dir.join(id);
                let disk = std::fs::metadata(&p)
                    .and_then(|x| vault::mtime_ms(&x))
                    .with_context(|| format!("không đọc được mtime của {}", p.display()))?;
                m.insert("mtime".into(), json!(mt - disk));
            }
            if t.name == "meta"
                && m.get("key")
                    .and_then(Value::as_str)
                    .is_some_and(|k| VOLATILE_META.contains(&k))
            {
                m.insert("value".into(), json!(VOLATILE));
            }
            for v in m.values_mut() {
                if let Value::String(s) = v {
                    *s = scrub.text(s);
                }
            }
            rows.push(m);
        }
        tables.insert(t.name, rows);
    }
    drop(db);
    let r1 = |x: f64| (x * 10.0).round() / 10.0;
    Ok(Built {
        tables,
        perf: json!({"build_full_ms": r1(full_ms), "build_noop_ms": r1(noop_ms)}),
        db: db_path,
        scrub,
        _tmp: tmp,
    })
}
