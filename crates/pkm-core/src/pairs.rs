//! Note gần nghĩa từ vector note của OHS: cặp note gần nghĩa nằm ở hai nhóm thư mục khác nhau (hàng chờ của đề
//! xuất `related`) và K láng giềng của mỗi note (đầu vào của [`crate::zones`]).
//!
//! Xuất phát từ `scripts/e11/similar_pairs.mjs` và `scripts/e11/note_knn.mjs` của repo nghiên cứu. Hai script đó đọc
//! thẳng `vec_chunks` của OHS bằng `better-sqlite3`; ở đây vector đi qua action `vectors` của OHS ([`Ohs::vectors`]:
//! vector đoạn gộp trung bình rồi chuẩn hoá L2, cùng hướng với tổng của script), còn lọc note, cosine, chọn cặp và K
//! láng giềng tính trong Rust với cùng thứ tự phép tính (tích vô hướng trên `f32`, cộng dồn `f64`, sắp ổn định, làm
//! tròn ba chữ số như `Math.round`).
//!
//! Note bị loại: trang sinh và thư mục hệ thống ([`is_generated`]), file không phải `.md`, note có ít hơn
//! `min_words` từ. Số từ đếm trên thân note (bỏ frontmatter) đọc từ vault, như cột `content` mà script đếm.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{Value, json};

use crate::ohs::{Ohs, OhsError, VectorReply};
use crate::vault;

/// Trang sinh tự động, trang chủ đề và thư mục hệ thống của bản B: không phải nguyên liệu.
static GENERATED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(_E11|_Templates|_Xưởng|_Đề xuất|_ohs)/| \(chủ đề\)\.md$|^Trang chủ\.md$")
        .expect("regex tĩnh hợp lệ")
});

/// Frontmatter ở đầu note (cùng mẫu với `vault`).
pub(crate) static FRONTMATTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*(?:\r?\n|\z)").expect("regex tĩnh hợp lệ")
});

/// Trang sinh hoặc note trong thư mục hệ thống (`SKIP` của hai script và của `zones.py`).
pub fn is_generated(path: &str) -> bool {
    GENERATED.is_match(path)
}

/// Số từ (cách nhau bằng khoảng trắng) của thân note, bỏ frontmatter.
pub fn body_words(text: &str) -> usize {
    let body = FRONTMATTER.find(text).map_or(text, |m| &text[m.end()..]);
    body.split_whitespace().count()
}

/// `round(x, n)` của Python với số thực: làm tròn đúng theo giá trị nhị phân, hoà thì về số chẵn.
pub(crate) fn py_round(x: f64, ndigits: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{x:.ndigits$}").parse().unwrap_or(x)
}

/// `Math.round(x * 1000) / 1000` của JS: hoà thì về phía +∞.
pub fn js_round3(x: f64) -> f64 {
    let y = x * 1000.0;
    let r = y.round();
    let r = if (r - y).abs() == 0.5 { y.ceil() } else { r };
    r / 1000.0
}

/// Vector đơn vị của các note được giữ, theo thứ tự OHS trả.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteVectors {
    pub paths: Vec<String>,
    pub dim: usize,
    /// Hàng nối nhau, mỗi hàng `dim` số `f32` (như `Float32Array` của script).
    data: Vec<f32>,
}

impl NoteVectors {
    /// Vector của các bản ghi `vectors` mà `keep(đường dẫn)` đúng. Bản ghi `{path, error}` bị bỏ qua (chỉ có khi hỏi
    /// đích danh note). Vector có số chiều lệch là lỗi: OHS đã hứa không trả vector sai.
    pub fn from_reply(reply: &VectorReply, keep: impl Fn(&str) -> bool) -> Result<Self, String> {
        let mut out = NoteVectors {
            paths: Vec::new(),
            dim: 0,
            data: Vec::new(),
        };
        for r in &reply.records {
            let Some(path) = r["path"].as_str() else {
                continue;
            };
            let Some(vector) = r["vector"].as_array() else {
                continue;
            };
            if !keep(path) {
                continue;
            }
            if out.paths.is_empty() {
                out.dim = vector.len();
            } else if vector.len() != out.dim {
                return Err(format!(
                    "vector của {path} có {} chiều, note trước có {}",
                    vector.len(),
                    out.dim
                ));
            }
            for x in vector {
                let x = x
                    .as_f64()
                    .ok_or_else(|| format!("vector của {path} có phần tử không phải số: {x}"))?;
                out.data.push(x as f32);
            }
            out.paths.push(path.to_owned());
        }
        Ok(out)
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    fn row(&self, i: usize) -> &[f32] {
        &self.data[i * self.dim..(i + 1) * self.dim]
    }

    /// Tích vô hướng của hai hàng: nhân từng cặp `f32` trong `f64`, cộng dồn `f64` theo thứ tự chiều (như JS).
    pub fn dot(&self, i: usize, j: usize) -> f64 {
        self.row(i)
            .iter()
            .zip(self.row(j))
            .fold(0.0, |d, (a, b)| d + f64::from(*a) * f64::from(*b))
    }
}

/// Lấy vector mọi note từ OHS (gộp trung bình, chuẩn hoá) và giữ note không phải trang sinh, đuôi `.md`, có ít nhất
/// `min_words` từ trong thân. OHS chưa sẵn sàng thì trả [`OhsError::NotReady`], không có đường lùi.
pub fn fetch(ohs: &Ohs, vault_dir: &Path, min_words: usize) -> Result<NoteVectors, FetchError> {
    let reply = ohs
        .vectors(&[], json!({"pooling": "mean", "normalize": true}))
        .map_err(FetchError::Ohs)?;
    let words = word_counts(vault_dir).map_err(|e| FetchError::Other(e.to_string()))?;
    NoteVectors::from_reply(&reply, |p| {
        !is_generated(p) && p.ends_with(".md") && words.get(p).copied().unwrap_or(0) >= min_words
    })
    .map_err(FetchError::Other)
}

/// Lỗi của [`fetch`].
#[derive(Debug, Clone, PartialEq)]
pub enum FetchError {
    Ohs(OhsError),
    Other(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ohs(e) => e.fmt(f),
            Self::Other(m) => f.write_str(m),
        }
    }
}

/// Số từ thân của mọi note trong vault, theo đường dẫn tương đối NFC.
pub fn word_counts(vault_dir: &Path) -> std::io::Result<HashMap<String, usize>> {
    let notes = vault::list_notes(vault_dir, None)?;
    Ok(notes
        .into_iter()
        .map(|(rel, f)| {
            let n = vault::read(&f.path).map_or(0, |t| body_words(&t));
            (rel, n)
        })
        .collect())
}

/// Nhóm của một note: ba đoạn đường dẫn đầu. Hai note cùng nhóm là các bản của một tài liệu, không phải quan hệ mà
/// tìm từ khoá bỏ sót.
fn group(path: &str) -> String {
    path.split('/').take(3).collect::<Vec<_>>().join("/")
}

/// Tham số chọn cặp (mặc định như `similar_pairs.mjs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairOptions {
    /// Cosine tối thiểu.
    pub min: f64,
    /// Số cặp tối đa.
    pub max: usize,
    /// Số cặp tối đa của mỗi note.
    pub per_note: usize,
    /// Số từ tối thiểu của thân note.
    pub min_words: usize,
}

impl Default for PairOptions {
    fn default() -> Self {
        Self {
            min: 0.85,
            max: 60,
            per_note: 2,
            min_words: 40,
        }
    }
}

/// Kết quả chọn cặp: cặp đã chọn và số đếm cho dòng tóm tắt.
#[derive(Debug, Clone, PartialEq)]
pub struct Pairs {
    /// `[{a, b, score}]`, điểm giảm dần, điểm làm tròn ba chữ số: định dạng file `similar-pairs.json` mà adapter
    /// `e11` và `propose --similar` đọc.
    pub picked: Vec<Value>,
    /// Số note có vector được giữ.
    pub notes: usize,
    /// Số cặp khác nhóm đạt `min` trước khi chọn.
    pub candidates: usize,
}

/// Cặp note khác nhóm có cosine từ `min`, điểm cao trước; mỗi note nhiều nhất `per_note` cặp, tổng nhiều nhất `max`.
pub fn similar_pairs(v: &NoteVectors, o: &PairOptions) -> Pairs {
    let groups: Vec<String> = v.paths.iter().map(|p| group(p)).collect();
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..v.len() {
        for j in i + 1..v.len() {
            if groups[i] == groups[j] {
                continue;
            }
            let d = v.dot(i, j);
            if d >= o.min {
                pairs.push((d, i, j));
            }
        }
    }
    // Sắp ổn định, điểm giảm dần (như `Array.prototype.sort` của V8).
    pairs.sort_by(|x, y| y.0.total_cmp(&x.0));
    let mut used = vec![0usize; v.len()];
    let mut picked = Vec::new();
    for &(score, a, b) in &pairs {
        if picked.len() >= o.max {
            break;
        }
        if used[a] >= o.per_note || used[b] >= o.per_note {
            continue;
        }
        picked
            .push(json!({"a": v.paths[a], "b": v.paths[b], "score": js_number(js_round3(score))}));
        used[a] += 1;
        used[b] += 1;
    }
    Pairs {
        picked,
        notes: v.len(),
        candidates: pairs.len(),
    }
}

/// Số như `JSON.stringify` in: số nguyên viết không có phần thập phân.
fn js_number(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        json!(x as i64)
    } else {
        json!(x)
    }
}

/// K note gần nhất của mỗi note (kể cả cùng thư mục), điểm giảm dần làm tròn ba chữ số; hoà thì note đứng trước theo
/// thứ tự OHS trả đứng trước. Mọi note có mặt, kể cả khi danh sách rỗng.
pub fn knn(v: &NoteVectors, k: usize) -> IndexMap<String, Vec<(String, f64)>> {
    let mut best: Vec<Vec<(f64, usize)>> = vec![Vec::new(); v.len()];
    let push = |b: &mut Vec<(f64, usize)>, d: f64, j: usize| {
        if k == 0 {
            return;
        }
        if b.len() == k {
            if d <= b[k - 1].0 {
                return;
            }
            b.pop();
        }
        // Chèn sau mọi phần tử có điểm lớn hơn hoặc bằng: sắp ổn định.
        let at = b.partition_point(|x| x.0 >= d);
        b.insert(at, (d, j));
    };
    for i in 0..v.len() {
        for j in i + 1..v.len() {
            let d = v.dot(i, j);
            push(&mut best[i], d, j);
            push(&mut best[j], d, i);
        }
    }
    v.paths
        .iter()
        .zip(best)
        .map(|(p, b)| {
            let list = b
                .into_iter()
                .map(|(d, j)| (v.paths[j].clone(), js_round3(d)))
                .collect();
            (p.clone(), list)
        })
        .collect()
}

#[cfg(test)]
mod tests;
