//! Vùng tri thức: nhóm note, đoạn tô và ý cùng nói về một việc, để gợi ý bài viết.
//!
//! Xuất phát từ `scripts/e11/zones.py` của repo nghiên cứu, cùng thuật toán và cùng thứ tự phép tính (điểm so với
//! bản Python sai khác trong mức làm tròn số thực):
//!
//! 1. Đồ thị note ([`build_graph`]): K láng giềng của mỗi note ([`crate::pairs::knn`]), trọng số theo thang cục bộ
//!    (so với điểm của láng giềng thứ 7 của chính note đó chứ không so một ngưỡng chung, vì điểm e5 nền cao), cộng
//!    cạnh `accepted` người viết (wikilink thân note, `notes`, `reason`, link frontmatter). Không dùng cạnh thư mục
//!    và `author`: vùng sẽ chỉ lặp lại cây thư mục hoặc gom trích theo tác giả.
//! 2. Louvain ([`louvain`]) chia cộng đồng; cộng đồng quá to thì chia tiếp trong chính nó ([`zones_of`]). Thứ tự
//!    duyệt nút xáo bằng Mersenne Twister hạt giống 7 giống hệt `random.Random(7).shuffle` của Python, nên cùng đầu
//!    vào ra cùng vùng như bản Python.
//! 3. Chấm mỗi vùng ([`describe`]): note có tô, đoạn tô theo màu, ý của mình, số thư mục, số note; "đã có bài".
//!
//! Module không ghi note nào. [`page`] dựng trang `_Xưởng/Gợi ý bài viết.md`; [`write_page_if_absent`] chỉ ghi khi
//! trang chưa có, vì người dùng chấm vùng ngay trên trang đó.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{Map, Value, json};

use crate::pairs::{is_generated, py_round};
use crate::query::{Graph, QueryError};
use crate::related::py_sum;
use crate::vault::{self, nfc};

/// Trang gợi ý bài viết, tương đối với vault.
pub const PAGE: &str = "_Xưởng/Gợi ý bài viết.md";
/// File vùng, tương đối với vault.
pub const OUT: &str = ".e11/zones.json";
/// Cạnh người viết `(rel, origin)`.
const HAND_EDGES: [(&str, &str); 4] = [
    ("link", "body"),
    ("notes", "frontmatter"),
    ("reason", "frontmatter"),
    ("link", "frontmatter"),
];
/// Hạt giống của bộ xáo (như `random.Random(7)`).
const SEED: u32 = 7;
/// `==đoạn tô==`, kèm màu `#hl/<màu>` ngay sau. Ký tự đầu của đoạn không phải `=` (thay cho `(?!=)` của Python).
static EQ: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"==([^=\n][^\n]*?)==(?: #hl/([a-z]+))?").expect("regex tĩnh hợp lệ")
});
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w+").expect("regex tĩnh hợp lệ"));
const STOP: &str = "và của là có cho các những một không được trong với để khi này đó thì mà như từ theo về đã sẽ lại \
ra vào bị nên cũng rất hay người làm sao gì nào đến tại trên dưới hơn nhất the a an of to and in for on is with how \
what why your you from by at or be are it this that my md note ghi chép bản copy notes summary chapter thread post \
book_ part";
static STOPWORDS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| STOP.split_whitespace().collect());

/// Trọng số của màu tô (màu khác: 1).
fn weight(colour: Option<&str>) -> u64 {
    match colour {
        Some("green" | "red" | "blue") => 3,
        Some("orange") => 2,
        _ => 1,
    }
}

/// Nghĩa của màu tô. Màu ngoài bảng giữ nguyên tên (bản Python dừng với `KeyError`).
fn meaning(colour: Option<&str>) -> String {
    match colour {
        Some("green" | "blue") => "làm được",
        Some("red") => "lỗi hay mắc",
        Some("orange") => "mới học",
        Some("yellow") => "thú vị",
        Some("purple") => "tím",
        None => "chưa màu",
        Some(other) => other,
    }
    .to_owned()
}

/// Cạnh vô hướng `(a, b)` với `a < b`.
pub type Edge = (String, String);
/// Trọng số cạnh theo thứ tự thêm vào (thứ tự này quyết định thứ tự cộng số thực, như `dict` của Python).
pub type Weights = IndexMap<Edge, f64>;

fn edge(a: &str, b: &str) -> Edge {
    if a < b {
        (a.to_owned(), b.to_owned())
    } else {
        (b.to_owned(), a.to_owned())
    }
}

// ------------------------------------------------------------------------------------------------ đồ thị

/// Trọng số cạnh: gần nghĩa theo thang cục bộ (10 láng giềng đầu), cộng 1 cho mỗi cạnh người viết. Chỉ giữ note trong
/// `keep`. Trả trọng số và số cạnh người viết.
pub fn build_graph(
    knn: &IndexMap<String, Vec<(String, f64)>>,
    g: &Graph,
    keep: &HashSet<String>,
) -> Result<(Weights, usize), QueryError> {
    let mut w = Weights::new();
    let sigma: HashMap<&str, f64> = knn
        .iter()
        .filter_map(|(p, v)| {
            let s = v.get(6).or(v.last())?.1;
            Some((p.as_str(), (1.0 - s).max(1e-3)))
        })
        .collect();
    for (a, v) in knn {
        for (b, s) in v.iter().take(10) {
            if a == b || !keep.contains(a) || !keep.contains(b) {
                continue;
            }
            let sa = sigma[a.as_str()];
            let sb = sigma.get(b.as_str()).copied().unwrap_or(sa);
            let x = (-((1.0 - s).powi(2)) / (sa * sb)).exp();
            let e = w.entry(edge(a, b)).or_insert(0.0);
            *e = e.max(x);
        }
    }
    let mut hand = 0;
    let mut st = g.conn().prepare(
        "SELECT src, dst, rel, origin FROM edges WHERE status = 'accepted' AND dst IS NOT NULL",
    )?;
    let mut rows = st.query([])?;
    while let Some(r) = rows.next()? {
        let (src, dst): (String, String) = (r.get(0)?, r.get(1)?);
        let (rel, origin): (Option<String>, Option<String>) = (r.get(2)?, r.get(3)?);
        let is_hand = HAND_EDGES
            .iter()
            .any(|(hr, ho)| rel.as_deref() == Some(hr) && origin.as_deref() == Some(ho));
        if !is_hand || src == dst || !keep.contains(&src) || !keep.contains(&dst) {
            continue;
        }
        *w.entry(edge(&src, &dst)).or_insert(0.0) += 1.0;
        hand += 1;
    }
    Ok((w, hand))
}

/// Mersenne Twister MT19937 với `seed` một từ 32 bit, đúng như `random.Random(seed)` của Python (`init_by_array`).
struct PyRandom {
    mt: [u32; 624],
    index: usize,
}

impl PyRandom {
    fn new(seed: u32) -> Self {
        let mut mt = [0u32; 624];
        mt[0] = 19_650_218;
        for i in 1..624 {
            mt[i] = 1_812_433_253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        let key = [seed];
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..624.max(key.len()) {
            mt[i] = (mt[i] ^ (mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                mt[0] = mt[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..623 {
            mt[i] = (mt[i] ^ (mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1_566_083_941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                mt[0] = mt[623];
                i = 1;
            }
        }
        mt[0] = 0x8000_0000;
        Self { mt, index: 624 }
    }

    fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            for k in 0..624 {
                let y = (self.mt[k] & 0x8000_0000) | (self.mt[(k + 1) % 624] & 0x7fff_ffff);
                let mag = if y & 1 == 1 { 0x9908_b0df } else { 0 };
                self.mt[k] = self.mt[(k + 397) % 624] ^ (y >> 1) ^ mag;
            }
            self.index = 0;
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// `_randbelow_with_getrandbits(n)`, `n >= 1`, `n < 2^32`.
    fn below(&mut self, n: usize) -> usize {
        let bits = usize::BITS - n.leading_zeros();
        loop {
            let r = (self.next_u32() >> (32 - bits)) as usize;
            if r < n {
                return r;
            }
        }
    }

    /// `random.shuffle`.
    fn shuffle<T>(&mut self, x: &mut [T]) {
        for i in (1..x.len()).rev() {
            let j = self.below(i + 1);
            x.swap(i, j);
        }
    }
}

/// Louvain (chuyển nút rồi gộp, lặp tới khi hết đổi) trên các nút `nodes` với trọng số `w`. Trả `{nút: mã cộng
/// đồng}`, mã đánh từ 0, nút xếp theo cộng đồng.
pub fn louvain(nodes: &BTreeSet<String>, w: &Weights, gamma: f64) -> IndexMap<String, usize> {
    let mut rnd = PyRandom::new(SEED);
    // Nút đánh số theo thứ tự tên (cộng đồng mang số của nút đại diện).
    let names: Vec<&String> = nodes.iter().collect();
    let id: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let mut adj: IndexMap<usize, IndexMap<usize, f64>> = IndexMap::new();
    for ((a, b), x) in w {
        if let (Some(&a), Some(&b)) = (id.get(a.as_str()), id.get(b.as_str())) {
            *adj.entry(a).or_default().entry(b).or_insert(0.0) += x;
            *adj.entry(b).or_default().entry(a).or_insert(0.0) += x;
        }
    }
    let mut members: IndexMap<usize, Vec<usize>> = (0..names.len()).map(|i| (i, vec![i])).collect();
    loop {
        let deg: IndexMap<usize, f64> = members
            .keys()
            .map(|&n| {
                let d = adj.get(&n).map_or(0.0, |m| py_sum(m.values().copied()));
                (n, d)
            })
            .collect();
        let m2 = py_sum(deg.values().copied());
        let m2 = if m2 == 0.0 { 1.0 } else { m2 };
        let mut com: IndexMap<usize, usize> = members.keys().map(|&n| (n, n)).collect();
        let mut tot: HashMap<usize, f64> = deg.iter().map(|(&n, &d)| (n, d)).collect();
        // `sorted(members)`: theo tên của nút đại diện; số nút tăng theo tên nên sắp theo số.
        let mut order: Vec<usize> = members.keys().copied().collect();
        order.sort_unstable();
        let (mut moved, mut changed) = (true, false);
        while moved {
            moved = false;
            rnd.shuffle(&mut order);
            for &n in &order {
                let (c0, k) = (com[&n], deg[&n]);
                let mut links: IndexMap<usize, f64> = IndexMap::new();
                if let Some(nb) = adj.get(&n) {
                    for (&b, &x) in nb {
                        if b != n {
                            *links.entry(com[&b]).or_insert(0.0) += x;
                        }
                    }
                }
                *tot.get_mut(&c0).expect("cộng đồng có tổng") -= k;
                let mut best = c0;
                let mut gain = links.get(&c0).copied().unwrap_or(0.0) - gamma * tot[&c0] * k / m2;
                for (&c, &x) in &links {
                    let g = x - gamma * tot[&c] * k / m2;
                    if g > gain + 1e-12 {
                        best = c;
                        gain = g;
                    }
                }
                *tot.get_mut(&best).expect("cộng đồng có tổng") += k;
                if best != c0 {
                    com.insert(n, best);
                    moved = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
        let mut new_members: IndexMap<usize, Vec<usize>> = IndexMap::new();
        for (n, c) in &com {
            new_members
                .entry(*c)
                .or_default()
                .extend(members[n].iter().copied());
        }
        let mut new_adj: IndexMap<usize, IndexMap<usize, f64>> = IndexMap::new();
        for (a, nbrs) in &adj {
            for (b, x) in nbrs {
                let (ca, cb) = (com[a], com[b]);
                *new_adj.entry(ca).or_default().entry(cb).or_insert(0.0) += x;
            }
        }
        members = new_members;
        adj = new_adj;
    }
    let mut out = IndexMap::new();
    for (i, ms) in members.values().enumerate() {
        for n in ms {
            out.insert(names[*n].clone(), i);
        }
    }
    out
}

/// Nhóm của kết quả Louvain, theo thứ tự mã cộng đồng.
fn groups(com: IndexMap<String, usize>) -> Vec<BTreeSet<String>> {
    let mut g: IndexMap<usize, BTreeSet<String>> = IndexMap::new();
    for (n, c) in com {
        g.entry(c).or_default().insert(n);
    }
    g.into_values().collect()
}

/// Vùng: cộng đồng Louvain từ `min_size` note; cộng đồng quá `max_size` thì chia tiếp (gamma 1,5).
pub fn zones_of(
    nodes: &BTreeSet<String>,
    w: &Weights,
    max_size: usize,
    min_size: usize,
) -> Vec<BTreeSet<String>> {
    let mut out = Vec::new();
    for g in groups(louvain(nodes, w, 1.0)) {
        if g.len() > max_size {
            let sub = groups(louvain(&g, w, 1.5));
            if sub.len() > 1 {
                for s in sub {
                    if s.len() > max_size {
                        out.extend(zones_of(&s, w, max_size, min_size));
                    } else if s.len() >= min_size {
                        out.push(s);
                    }
                }
                continue;
            }
        }
        if g.len() >= min_size {
            out.push(g);
        }
    }
    out
}

// ------------------------------------------------------------------------------------------------ chấm

/// Thông tin một note cho việc chấm.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoteInfo {
    pub title: String,
    pub created: String,
    /// Đoạn tô `(chữ, màu)`.
    pub hl: Vec<(String, Option<String>)>,
    /// Ý của mình: trong `Ý/`, hoặc type `idea`, `note`.
    pub own: bool,
    /// Bài viết: trong `Bài viết/`, hoặc type `post`.
    pub post: bool,
}

/// Từ của tiêu đề: chữ thường, dài hơn một ký tự, không phải từ dừng, không trong `skip`, không phải số.
pub fn words(title: &str, skip: &HashSet<String>) -> Vec<String> {
    let lower = nfc(title).to_lowercase();
    WORD.find_iter(&lower)
        .map(|m| m.as_str())
        .filter(|t| {
            t.chars().count() > 1
                && !STOPWORDS.contains(t)
                && !skip.contains(*t)
                && !t.chars().all(char::is_numeric)
        })
        .map(str::to_owned)
        .collect()
}

/// `Counter.most_common()`: đếm giảm dần, hoà giữ thứ tự gặp.
fn most_common<K: Clone + Eq + std::hash::Hash>(c: &IndexMap<K, u64>) -> Vec<(K, u64)> {
    let mut v: Vec<(K, u64)> = c.iter().map(|(k, n)| (k.clone(), *n)).collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1));
    v
}

/// Chấm một vùng: tên (các từ hay gặp trong tiêu đề, theo tf-idf), điểm, note chính, đoạn tô theo màu, thư mục,
/// bài đã có.
pub fn describe(
    zone: &BTreeSet<String>,
    w: &Weights,
    info: &HashMap<String, NoteInfo>,
    df: &HashMap<String, u64>,
    n_all: usize,
    skip: &HashSet<String>,
) -> Value {
    let mut inner: IndexMap<&str, f64> = IndexMap::new();
    for ((a, b), x) in w {
        if zone.contains(a) && zone.contains(b) {
            *inner.entry(a).or_insert(0.0) += x;
            *inner.entry(b).or_insert(0.0) += x;
        }
    }
    let mut ranked: Vec<(&str, f64)> = inner.iter().map(|(n, x)| (*n, *x)).collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let mut core: Vec<&str> = ranked.into_iter().map(|(n, _)| n).collect();
    core.extend(
        zone.iter()
            .map(String::as_str)
            .filter(|n| !inner.contains_key(n)),
    );

    let mut tf: IndexMap<String, u64> = IndexMap::new();
    for n in zone {
        let uniq: BTreeSet<String> = words(&info[n].title, skip).into_iter().collect();
        for t in uniq {
            *tf.entry(t).or_insert(0) += 1;
        }
    }
    let mut terms: Vec<(&String, u64)> = tf.iter().map(|(t, c)| (t, *c)).collect();
    let key = |t: &String, c: u64| -(c as f64) * (n_all as f64 / df[t] as f64).ln();
    terms.sort_by(|a, b| {
        key(a.0, a.1)
            .total_cmp(&key(b.0, b.1))
            .then_with(|| a.0.cmp(b.0))
    });
    let label: Vec<&str> = terms
        .iter()
        .filter(|(_, c)| *c >= 2)
        .map(|(t, _)| t.as_str())
        .take(4)
        .collect();

    let hl: Vec<&(String, Option<String>)> = core.iter().flat_map(|n| &info[*n].hl).collect();
    let hl_w: u64 = hl.iter().map(|(_, c)| weight(c.as_deref())).sum();
    let with_hl = zone.iter().filter(|n| !info[*n].hl.is_empty()).count();
    let own = zone.iter().filter(|n| info[*n].own).count();
    let mut folders: IndexMap<&str, u64> = IndexMap::new();
    for n in zone {
        *folders.entry(vault::folder(n)).or_insert(0) += 1;
    }
    let posts: Vec<&String> = zone.iter().filter(|n| info[*n].post).collect();
    let mut years: Vec<String> = zone
        .iter()
        .filter(|n| !info[*n].created.is_empty())
        .map(|n| info[n].created.chars().take(4).collect())
        .collect();
    years.sort();
    let score = (with_hl * 2) as f64
        + hl_w.min(150) as f64 / 10.0
        + (own * 3) as f64
        + folders.len().min(6) as f64
        + zone.len() as f64 / 5.0;
    let mut picks: Vec<&&(String, Option<String>)> = hl.iter().collect();
    picks.sort_by_key(|h| std::cmp::Reverse(weight(h.1.as_deref())));
    let picks: Vec<Value> = picks
        .into_iter()
        .take(5)
        .map(|(t, c)| json!({"text": t, "colour": meaning(c.as_deref())}))
        .collect();
    let mut colours: IndexMap<String, u64> = IndexMap::new();
    for (_, c) in &hl {
        *colours.entry(meaning(c.as_deref())).or_insert(0) += 1;
    }
    let colours: Map<String, Value> = most_common(&colours)
        .into_iter()
        .map(|(k, n)| (k, json!(n)))
        .collect();
    let top_folders: Vec<&str> = most_common(&folders)
        .into_iter()
        .take(3)
        .map(|(f, _)| f)
        .collect();
    let label = if label.is_empty() {
        info[core[0]].title.clone()
    } else {
        label.join(" · ")
    };
    json!({
        "label": label,
        "score": py_round(score, 1),
        "notes": zone.len(),
        "core": core.iter().take(8).collect::<Vec<_>>(),
        "all": core,
        "with_highlights": with_hl,
        "highlights": hl.len(),
        "highlight_weight": hl_w,
        "colours": colours,
        "own_ideas": own,
        "folders": folders.len(),
        "top_folders": top_folders,
        "posts": posts,
        "years": if years.is_empty() { json!([]) } else { json!([years[0], years[years.len() - 1]]) },
        "picks": picks,
    })
}

/// Thông tin chấm của các note `nodes`: tiêu đề, type, ngày tạo từ index; đoạn tô đọc từ file trong vault.
pub fn note_info(
    vault_dir: &Path,
    g: &Graph,
    nodes: &BTreeSet<String>,
) -> Result<HashMap<String, NoteInfo>, QueryError> {
    let mut types: HashMap<String, Option<String>> = HashMap::new();
    let mut titles: HashMap<String, Option<String>> = HashMap::new();
    {
        let mut st = g.conn().prepare("SELECT id, type, title FROM nodes")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let id: String = r.get(0)?;
            types.insert(id.clone(), r.get(1)?);
            titles.insert(id, r.get(2)?);
        }
    }
    let mut created: HashMap<String, String> = HashMap::new();
    {
        let mut st = g
            .conn()
            .prepare("SELECT node, value FROM props WHERE key = 'created'")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let v: Option<String> = r.get(1)?;
            created.insert(r.get(0)?, v.unwrap_or_default());
        }
    }
    let mut out = HashMap::new();
    for n in nodes {
        let body = vault::read(&vault_dir.join(n)).unwrap_or_default();
        let t = types.get(n).cloned().flatten();
        let title = titles
            .get(n)
            .cloned()
            .flatten()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| vault::title(n).to_owned());
        let hl = EQ
            .captures_iter(&body)
            .map(|c| {
                (
                    c[1].trim().to_owned(),
                    c.get(2).map(|m| m.as_str().to_owned()),
                )
            })
            .collect();
        out.insert(
            n.clone(),
            NoteInfo {
                title,
                created: created.get(n).cloned().unwrap_or_default(),
                hl,
                own: n.starts_with("Ý/") || matches!(t.as_deref(), Some("idea" | "note")),
                post: n.starts_with("Bài viết/") || t.as_deref() == Some("post"),
            },
        );
    }
    Ok(out)
}

/// Tham số của [`zones`] (mặc định như `zones.py`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub max_size: usize,
    pub min_size: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_size: 60,
            min_size: 5,
        }
    }
}

/// Vùng đã chấm, điểm giảm dần, và số đếm của lần chạy.
#[derive(Debug, Clone, PartialEq)]
pub struct Zones {
    pub zones: Vec<Value>,
    /// `{notes, knn, hand_edges, zones, covered, size_min, size_median, size_max, with_posts, single_folder}`.
    pub stats: Value,
}

/// Chia vùng và chấm. `knn`: K láng giềng của mỗi note ([`crate::pairs::knn`]); `g`: index graph của vault.
pub fn zones(
    vault_dir: &Path,
    g: &Graph,
    knn: &IndexMap<String, Vec<(String, f64)>>,
    o: &Options,
) -> Result<Zones, QueryError> {
    let mut keep = HashSet::new();
    {
        let mut st = g.conn().prepare("SELECT id FROM nodes")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let id: String = r.get(0)?;
            if !is_generated(&id) && id.ends_with(".md") {
                keep.insert(id);
            }
        }
    }
    let (w, hand) = build_graph(knn, g, &keep)?;
    let nodes: BTreeSet<String> = w.keys().flat_map(|(a, b)| [a.clone(), b.clone()]).collect();
    let zs = zones_of(&nodes, &w, o.max_size, o.min_size);
    let info = note_info(vault_dir, g, &nodes)?;
    // Tên tác giả, tài khoản (tweet, trích) không nói vùng nói về gì.
    let mut skip = HashSet::new();
    {
        let mut st = g
            .conn()
            .prepare("SELECT value FROM props WHERE key = 'author'")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            if let Some(v) = r.get::<_, Option<String>>(0)? {
                let lower = nfc(&v).to_lowercase();
                skip.extend(WORD.find_iter(&lower).map(|m| m.as_str().to_owned()));
            }
        }
    }
    let mut df: HashMap<String, u64> = HashMap::new();
    for n in &nodes {
        let uniq: BTreeSet<String> = words(&info[n].title, &skip).into_iter().collect();
        for t in uniq {
            *df.entry(t).or_insert(0) += 1;
        }
    }
    let mut out: Vec<Value> = zs
        .iter()
        .map(|z| describe(z, &w, &info, &df, nodes.len(), &skip))
        .collect();
    out.sort_by(|a, b| {
        let s = |v: &Value| v["score"].as_f64().unwrap_or(0.0);
        s(b).total_cmp(&s(a))
    });
    let mut sizes: Vec<u64> = out.iter().filter_map(|z| z["notes"].as_u64()).collect();
    sizes.sort_unstable();
    let stats = json!({
        "notes": nodes.len(),
        "knn": knn.len(),
        "hand_edges": hand,
        "zones": out.len(),
        "covered": sizes.iter().sum::<u64>(),
        "size_min": sizes.first(),
        "size_median": sizes.get(sizes.len() / 2),
        "size_max": sizes.last(),
        "with_posts": out.iter().filter(|z| z["posts"].as_array().is_some_and(|p| !p.is_empty())).count(),
        "single_folder": out.iter().filter(|z| z["folders"] == 1).count(),
    });
    Ok(Zones { zones: out, stats })
}

// ------------------------------------------------------------------------------------------------ trang

/// `urllib.parse.quote(s, safe='')`.
fn quote(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn link(node: &str) -> String {
    let stem = node.strip_suffix(".md").unwrap_or(node);
    format!("[[{stem}|{}]]", vault::title(node))
}

fn strs(v: &Value) -> Vec<&str> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Số như `str()` của Python in (số thực luôn có phần thập phân).
fn py_num(v: &Value) -> String {
    match v.as_f64() {
        Some(x) if v.is_f64() && x.fract() == 0.0 && x.abs() < 1e16 => format!("{x:.1}"),
        _ => v.to_string(),
    }
}

/// Trang `_Xưởng/Gợi ý bài viết.md`: `top` vùng đầu, mỗi vùng một mục có dòng **Chấm** để người dùng ghi và link
/// `obsidian://new` dựng bài nháp từ note chính của vùng. `stamp` là ngày chạy (`YYYY-MM-DD`).
pub fn page(zs: &[Value], vault_name: &str, stamp: &str, top: usize) -> String {
    let mut out: Vec<String> = vec![
        "---".into(),
        format!("generated: {stamp}"),
        "generated_by: pkm zones".into(),
        "---".into(),
        String::new(),
        "# Gợi ý bài viết".into(),
        String::new(),
        format!(
            "Máy quét {stamp}: {} vùng tri thức (nhóm note cùng nói về một việc, theo nghĩa và liên kết bạn viết). \
             Vùng xếp theo điểm: note có tô, màu tô (xanh lá, đỏ nặng hơn vàng; tính tới 150 điểm), ý của mình, số \
             thư mục, số note. Tên vùng là các chữ hay gặp trong tiêu đề, máy đặt, chưa phải góc viết: góc viết là \
             việc của bạn.",
            zs.len()
        ),
        String::new(),
        "Chấm: ghi `đáng viết`, `không` hoặc `bất ngờ` (vùng tự mình không thấy) ở dòng **Chấm** của mỗi vùng."
            .into(),
        String::new(),
    ];
    for (i, z) in zs.iter().take(top).enumerate() {
        let label = z["label"].as_str().unwrap_or_default();
        let core = strs(&z["core"]);
        let content = format!(
            "---\ntype: post\nstatus: ý tưởng\n---\n\n## Nguồn (máy gom từ vùng)\n\n{}\n",
            core.iter()
                .map(|n| format!("- [[{}]]", n.strip_suffix(".md").unwrap_or(n)))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let new = format!(
            "vault={}&file={}&content={}",
            quote(vault_name),
            quote(&format!("Bài viết/{}", label.replace(" · ", " "))),
            quote(&content)
        );
        let colours = z["colours"]
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| format!("{k} {v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let years = strs(&z["years"]);
        let posts = strs(&z["posts"]);
        let mut line = format!(
            "{} note, {} thư mục, {} note có tô ({} đoạn: {colours}), {} ý của mình",
            z["notes"], z["folders"], z["with_highlights"], z["highlights"], z["own_ideas"]
        );
        if let [first, last] = years[..] {
            line.push_str(&format!(", năm {first}–{last}"));
        }
        line.push_str(&format!(". Điểm {}.", py_num(&z["score"])));
        if !posts.is_empty() {
            line.push_str(" **Đã có bài:** ");
            line.push_str(&posts.iter().map(|p| link(p)).collect::<Vec<_>>().join(", "));
        }
        out.extend([
            format!("## {}. {label}", i + 1),
            String::new(),
            line,
            String::new(),
            format!(
                "Note chính: {}",
                core.iter().map(|n| link(n)).collect::<Vec<_>>().join(" · ")
            ),
            String::new(),
        ]);
        let picks = z["picks"].as_array().cloned().unwrap_or_default();
        for p in &picks {
            let text: String = p["text"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(220)
                .collect();
            out.push(format!(
                "> {text} *({})*\n>",
                p["colour"].as_str().unwrap_or_default()
            ));
        }
        if !picks.is_empty() {
            if let Some(last) = out.last_mut() {
                last.truncate(last.len() - 2);
            }
            out.push(String::new());
        }
        out.push(format!(
            "[Viết bài từ vùng này](obsidian://new?{new}) · **Chấm:** "
        ));
        out.push(String::new());
    }
    if zs.len() > top {
        out.push(format!(
            "Còn {} vùng điểm thấp hơn trong `{OUT}`.",
            zs.len() - top
        ));
        out.push(String::new());
    }
    out.join("\n")
}

/// Ghi trang gợi ý vào `vault_dir/`[`PAGE`] nếu trang chưa có. Trả `false` (không đụng gì) khi trang đã có: trang
/// giữ dòng chấm của người dùng.
pub fn write_page_if_absent(vault_dir: &Path, text: &str) -> io::Result<bool> {
    let path = vault_dir.join(PAGE);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut f) => {
            f.write_all(text.as_bytes())?;
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests;
