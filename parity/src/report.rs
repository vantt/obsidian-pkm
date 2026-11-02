//! Báo cáo của một lần so: `report.json` và bảng tóm tắt in ra terminal.
//!
//! Chế độ riêng tư (`--private`, vault thật của user): chỉ có số đơn vị và số lệch theo từng
//! bảng, op, tool, file, lệnh; không có tên note, giá trị, dòng hay id câu nào.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::compare::{Comparison, Diff, Status};
use crate::dump::{Dump, LAYERS};
use crate::normalize::FieldDiff;

/// Độ dài tối đa của một giá trị in trong báo cáo.
const VALUE_CHARS: usize = 240;
/// Số chỗ lệch tối đa in ra terminal (báo cáo JSON giữ đủ).
const PRINT_DIFFS: usize = 40;

pub struct Context<'a> {
    pub expected: &'a Dump,
    pub actual: &'a Dump,
    pub private: bool,
    pub strict: bool,
}

/// Mã thoát: 1 khi có lệch chưa có tên (hoặc, với `--strict`, còn mục chưa có), 0 khi không.
pub fn exit_code(cmp: &Comparison, strict: bool) -> i32 {
    i32::from(cmp.unnamed() > 0 || (strict && cmp.not_available() > 0))
}

fn short(v: &Value) -> Value {
    match v {
        Value::String(s) => Value::from(window(s, 0)),
        other => {
            let s = other.to_string();
            if s.chars().count() <= VALUE_CHARS {
                other.clone()
            } else {
                Value::from(window(&s, 0))
            }
        }
    }
}

/// Đoạn `VALUE_CHARS` ký tự của `s` bắt đầu từ ký tự `start`, đánh dấu phần bị cắt.
fn window(s: &str, start: usize) -> String {
    let n = s.chars().count();
    if n <= VALUE_CHARS && start == 0 {
        return s.to_owned();
    }
    let body: String = s.chars().skip(start).take(VALUE_CHARS).collect();
    let head = if start > 0 { "…" } else { "" };
    let tail = if start + VALUE_CHARS < n { "…" } else { "" };
    format!("{head}{body}{tail} [{n} ký tự]")
}

/// Giá trị hai bên của một chỗ lệch, rút gọn. Hai chuỗi dài thì cắt quanh ký tự khác đầu tiên.
fn shorten(f: &FieldDiff) -> (Option<Value>, Option<Value>) {
    if let (Some(Value::String(e)), Some(Value::String(a))) = (&f.expected, &f.actual)
        && (e.chars().count() > VALUE_CHARS || a.chars().count() > VALUE_CHARS)
    {
        let first = e.chars().zip(a.chars()).take_while(|(x, y)| x == y).count();
        let start = first.saturating_sub(VALUE_CHARS / 4);
        return (
            Some(Value::from(window(e, start))),
            Some(Value::from(window(a, start))),
        );
    }
    (f.expected.as_ref().map(short), f.actual.as_ref().map(short))
}

fn diff_json(d: &Diff) -> Value {
    json!({
        "kind": d.kind.code(),
        "unit": d.unit,
        "file": d.file,
        "line": d.line,
        "actual_line": d.actual_line,
        "exception": d.exception,
        "fields": d.fields.iter().map(|f| {
            let (expected, actual) = shorten(f);
            json!({"path": f.path, "expected": expected, "actual": actual})
        }).collect::<Vec<_>>(),
    })
}

pub fn to_json(cmp: &Comparison, ctx: &Context<'_>) -> Value {
    let side = |d: &Dump| {
        let m = &d.manifest;
        let mut v = json!({
            "implementation": m.implementation,
            "spec_sha": m.spec_sha,
            "not_available": m.not_available,
        });
        if !ctx.private {
            v["dir"] = Value::from(d.dir.display().to_string());
            v["vault"] = Value::from(m.vault.name.clone());
        }
        v
    };
    let mut layers = serde_json::Map::new();
    for layer in LAYERS.iter().filter(|l| **l != "perf") {
        let items: Vec<Value> = cmp
            .items
            .iter()
            .filter(|i| i.layer == *layer)
            .map(|i| {
                let mut v = json!({
                    "item": i.item,
                    "status": i.status.code(),
                    "units": i.units,
                    "diffs": i.unnamed(),
                    "named": i.named(),
                });
                if !ctx.private {
                    v["details"] = Value::from(i.diffs.iter().map(diff_json).collect::<Vec<_>>());
                }
                v
            })
            .collect();
        layers.insert((*layer).to_owned(), Value::from(items));
    }
    let perf = match &cmp.perf {
        None => json!({"status": "not_available"}),
        Some(rows) => json!({
            "status": "recorded",
            "rows": rows.iter().map(|r| json!({"metric": r.metric, "expected": r.expected, "actual": r.actual})).collect::<Vec<_>>(),
        }),
    };
    json!({
        "format": 1,
        "private": ctx.private,
        "strict": ctx.strict,
        "expected": side(ctx.expected),
        "actual": side(ctx.actual),
        "summary": {
            "items": cmp.items.len(),
            "match": cmp.items.iter().filter(|i| i.status == Status::Match).count(),
            "diff": cmp.items.iter().filter(|i| i.status == Status::Diff).count(),
            "not_available": cmp.not_available(),
            "unnamed_diffs": cmp.unnamed(),
            "named_diffs": cmp.items.iter().map(|i| i.named()).sum::<usize>(),
            "unused_exceptions": cmp.unused_exceptions,
        },
        "layers": Value::Object(layers),
        "perf": perf,
        "exit": exit_code(cmp, ctx.strict),
    })
}

/// Bảng tóm tắt cho terminal.
pub fn summary(cmp: &Comparison, ctx: &Context<'_>) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "So {} (thật) với {} (mẫu){}",
        ctx.actual.manifest.implementation,
        ctx.expected.manifest.implementation,
        if ctx.private {
            ", chế độ riêng tư: chỉ in số"
        } else {
            ""
        }
    );
    let _ = writeln!(
        s,
        "{:<8} {:<22} {:<8} {:>7} {:>6} {:>7}",
        "Tầng", "Mục", "Kết quả", "Đơn vị", "Lệch", "Có tên"
    );
    for i in &cmp.items {
        let (d, n) = if i.status == Status::NotAvailable {
            ("-".to_owned(), "-".to_owned())
        } else {
            (i.unnamed().to_string(), i.named().to_string())
        };
        let _ = writeln!(
            s,
            "{:<8} {:<22} {:<8} {:>7} {:>6} {:>7}",
            i.layer,
            i.item,
            i.status.label(),
            i.units,
            d,
            n
        );
    }
    match &cmp.perf {
        None => {
            let _ = writeln!(s, "perf     (mọi số đo)           chưa có");
        }
        Some(rows) => {
            let _ = writeln!(s, "Hiệu năng (ms, ghi số, không chặn): mẫu | thật");
            for r in rows {
                let f = |x: Option<f64>| x.map_or_else(|| "-".to_owned(), |v| format!("{v}"));
                let _ = writeln!(
                    s,
                    "  {:<34} {:>10} | {:>10}",
                    r.metric,
                    f(r.expected),
                    f(r.actual)
                );
            }
        }
    }
    if !ctx.private {
        let mut shown = 0;
        for i in &cmp.items {
            for d in i.diffs.iter().filter(|d| d.exception.is_none()) {
                if shown == PRINT_DIFFS {
                    let _ = writeln!(s, "… (đủ danh sách trong report.json)");
                    break;
                }
                shown += 1;
                let loc = match (d.line, d.actual_line) {
                    (Some(l), _) => format!("{}:{l}", d.file),
                    (None, Some(l)) => format!("{} (dump thật, dòng {l})", d.file),
                    (None, None) => d.file.clone(),
                };
                let _ = writeln!(
                    s,
                    "LỆCH {}/{} {} {} [{}]",
                    i.layer,
                    i.item,
                    loc,
                    d.kind.code(),
                    d.unit
                );
                for f in &d.fields {
                    let (e, a) = shorten(f);
                    let v = |x: Option<Value>| {
                        x.map_or_else(|| "(không có)".to_owned(), |v| v.to_string())
                    };
                    let _ = writeln!(s, "     {}: mẫu {} | thật {}", f.path, v(e), v(a));
                }
            }
        }
    }
    if !cmp.unused_exceptions.is_empty() {
        let _ = writeln!(
            s,
            "Ngoại lệ không dùng tới: {}",
            cmp.unused_exceptions.join(", ")
        );
    }
    let _ = writeln!(
        s,
        "Tổng: {} mục, {} lệch chưa có tên, {} mục chưa có.",
        cmp.items.len(),
        cmp.unnamed(),
        cmp.not_available()
    );
    s
}
