//! Validator schema v3 dạng SQL view. Mỗi view `v3_rN` trả (rule, node, detail) và chỉ đọc bảng dữ liệu (nodes,
//! props, links, edges, names, blocks) cùng các bảng `schema_*` nạp từ schema.yaml lúc build; `v3_deviations` là hợp
//! của chúng.
//!
//! Đặc tả: `scripts/kg/validate.py` ở sha ghim. Câu SQL của mọi view chép nguyên (D3): ghép từ cùng các mảnh như
//! Python nên `sqlite_master.sql` giống từng byte. Luật (đánh số như `rules` trong schema.yaml):
//!
//! - R0 frontmatter không đọc được (YAML lỗi, không đóng)
//! - R1 thiếu type, nhiều type, type ngoài schema (và các gói bật)
//! - R2 quan hệ ngoài schema: khoá frontmatter có wikilink nhưng không là cạnh, trường liên kết của type, hay khoá
//!   nghịch đảo
//! - R3 đích của cạnh hay trường liên kết không có note
//! - R4 domain/range: type note nguồn, note đích (tính cả `extends`, `same`, `*`); cả hai đầu của cạnh claim
//! - R5 khoá nghịch đảo viết tay (down, used_by, ...)
//! - R6 cặp loại trừ theo `excludes_with` (SKOS S27: `related` và `up` trên cùng một cặp, hướng nào cũng vậy)
//! - R7 cạnh trỏ về chính note
//! - R8 tên note hoặc alias trùng (không phân biệt hoa thường, dấu)
//! - R9 trường bắt buộc (`required`, `required_if`), giá trị ngoài enum (kể cả `relation` không là quan hệ claim),
//!   sai mẫu
//! - R10 quan hệ `human_only` (conditions) do máy (llm, model, rule) ghi mà không ở trạng thái proposed
//! - R11 ngày sai định dạng hoặc không có thật; valid_from sau valid_to; asserted_at ở tương lai
//! - R12 superseded thiếu superseded_by hoặc valid_to; chuỗi superseded_by có vòng
//! - R13 `#^block` trong source không có ở note đích; claim accepted không có source
//! - R14 lỗi của chính schema.yaml (extends, domain/range trỏ type không có), ghi lúc nạp
//! - R15 cạnh cardinality one có hơn một đích cùng lúc (frontmatter: cùng note; claim: cửa sổ chồng nhau)

use std::fmt::Write as _;
use std::sync::LazyLock;

use indexmap::IndexMap;
use rusqlite::Connection;
use serde_json::{Value, json};

/// `_ANC`: type `t` là `s` hoặc con cháu của `s`.
fn anc(t: &str, s: &str) -> String {
    format!("EXISTS (SELECT 1 FROM schema_type_ancestor a WHERE a.type = {t} AND a.ancestor = {s})")
}

/// `_KNOWN`: khoá frontmatter của note kiểu `st` được schema biết: cạnh (owner `*`) hoặc trường liên kết của type
/// (hay type cha).
fn known(e: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM schema_link k WHERE k.key = {e}.key AND (k.owner = '*' OR {}))",
        anc(&format!("{e}.st"), "k.owner")
    )
}

/// `_OK_SRC`.
fn ok_src(st: &str) -> String {
    format!("(l.src = '*' OR {})", anc(st, "l.src"))
}

/// `_OK_DST`.
fn ok_dst(st: &str, dt: &str) -> String {
    format!(
        "(l.dst = '*' OR (l.dst = 'same' AND {dt} = {st}) OR {})",
        anc(dt, "l.dst")
    )
}

/// `VIEWS`: (tên, câu SQL) theo thứ tự tạo của Python (view phụ trước, rồi `v3_r0` .. `v3_r15`).
pub static VIEWS: LazyLock<Vec<(&'static str, String)>> = LazyLock::new(|| {
    vec![
        (
            "fm_links",
            "SELECT l.src, l.key, l.target, l.frag, l.dst, l.via, s.type AS st, d.type AS dt FROM links l
        JOIN nodes s ON s.id = l.src LEFT JOIN nodes d ON d.id = l.dst
        WHERE l.origin = 'frontmatter' AND l.key != 'aliases' AND coalesce(l.via, '') != 'attachment' "
                .to_owned(),
        ),
        (
            "inverse_keys",
            "SELECT DISTINCT inverse AS key FROM schema_edge WHERE inverse IS NOT NULL AND inverse NOT IN (SELECT key FROM schema_edge)"
                .to_owned(),
        ),
        (
            "v3_r0",
            "SELECT 'R0' AS rule, id AS node, yaml_error AS detail FROM nodes WHERE yaml_error IS NOT NULL"
                .to_owned(),
        ),
        (
            "v3_r1",
            "SELECT 'R1' AS rule, id AS node, CASE WHEN ntypes > 1 THEN 'nhiều type' WHEN ntypes = 0 OR type IS NULL THEN 'thiếu type'
          ELSE 'type ngoài schema: ' || type END AS detail
        FROM nodes WHERE yaml_error IS NULL AND (ntypes != 1 OR type IS NULL OR type NOT IN (SELECT name FROM schema_node_type))"
                .to_owned(),
        ),
        (
            "v3_r2",
            "SELECT DISTINCT 'R2' AS rule, e.src AS node, 'khoá \"' || e.key || '\" có wikilink nhưng không là cạnh hay trường liên kết của schema' AS detail
        FROM fm_links e WHERE e.key NOT IN (SELECT key FROM inverse_keys) AND NOT "
                .to_owned()
                + &known("e"),
        ),
        (
            "v3_r3",
            "SELECT 'R3' AS rule, e.src AS node, e.key || ': [[' || e.target || ']] không có note' AS detail
        FROM fm_links e WHERE e.dst IS NULL AND "
                .to_owned()
                + &known("e"),
        ),
        (
            "v3_r4",
            "SELECT 'R4' AS rule, e.src AS node, e.key || ': ' || coalesce(e.st, '(không type)') || ' → ' || coalesce(e.dt, '(không type)')
          || ' (' || e.dst || ') ngoài domain/range' AS detail
        FROM fm_links e WHERE e.dst IS NOT NULL AND e.dst != e.src AND "
                .to_owned()
                + &known("e")
                + "
          AND NOT EXISTS (SELECT 1 FROM schema_link l WHERE l.key = e.key AND (l.owner = '*' OR "
                + &anc("e.st", "l.owner")
                + ")
            AND "
                + &ok_src("e.st")
                + " AND "
                + &ok_dst("e.st", "e.dt")
                + ")
      UNION ALL
      SELECT 'R4', c.claim, c.rel || ' của claim: ' || coalesce(s.type, '(không type)') || ' → ' || coalesce(d.type, '(không type)') || ' ngoài domain/range'
        FROM edges c JOIN nodes s ON s.id = c.src JOIN nodes d ON d.id = c.dst
        WHERE c.origin = 'claim' AND NOT EXISTS (SELECT 1 FROM schema_claim_rel l WHERE l.rel = c.rel
          AND "
                + &ok_src("s.type")
                + " AND "
                + &ok_dst("s.type", "d.type")
                + ")",
        ),
        (
            "v3_r5",
            "SELECT DISTINCT 'R5' AS rule, node, 'khoá nghịch đảo viết tay: ' || key AS detail FROM props
        WHERE key IN (SELECT key FROM inverse_keys)"
                .to_owned(),
        ),
        (
            "v3_r6",
            "SELECT DISTINCT 'R6' AS rule, b.src AS node, 'vừa ' || x.rel || ' vừa ' || x.other || ' với ' ||
          CASE WHEN b.src = a.src THEN a.dst ELSE a.src END AS detail
        FROM schema_excludes x JOIN edges a ON a.rel = x.rel AND a.claim IS NULL AND a.dst IS NOT NULL
        JOIN edges b ON b.rel = x.other AND b.claim IS NULL AND ((a.src = b.src AND a.dst = b.dst) OR (a.src = b.dst AND a.dst = b.src))"
                .to_owned(),
        ),
        (
            "v3_r7",
            "SELECT 'R7' AS rule, src AS node, key || ' trỏ về chính note' AS detail FROM fm_links WHERE dst = src"
                .to_owned(),
        ),
        (
            "v3_r8",
            "SELECT 'R8' AS rule, node, kind || ' trùng (không phân biệt hoa thường, dấu): ' || name AS detail FROM names
        WHERE norm IN (SELECT norm FROM names GROUP BY norm HAVING count(DISTINCT node) > 1)"
                .to_owned(),
        ),
        (
            "v3_r9",
            "SELECT 'R9' AS rule, n.id AS node, 'thiếu trường bắt buộc của ' || n.type || ': ' || f.field AS detail
        FROM nodes n JOIN schema_field f ON f.required = 1 AND f.owner != '*' AND "
                .to_owned()
                + &anc("n.type", "f.owner")
                + "
        WHERE NOT EXISTS (SELECT 1 FROM props p WHERE p.node = n.id AND p.key = f.field AND p.value IS NOT NULL AND p.value != '')
      UNION ALL
      SELECT 'R9', n.id, 'thiếu ' || r.field || ' (bắt buộc khi ' || r.cond_key || ' = ' || r.cond_value || ')'
        FROM nodes n JOIN schema_required_if r ON "
                + &anc("n.type", "r.owner")
                + "
        JOIN props c ON c.node = n.id AND c.key = r.cond_key AND c.value = r.cond_value
        WHERE NOT EXISTS (SELECT 1 FROM props p WHERE p.node = n.id AND p.key = r.field AND p.value IS NOT NULL AND p.value != '')
      UNION ALL
      SELECT DISTINCT 'R9', p.node, p.key || ' = ' || p.value || ' ngoài tập ' || f.enum_name FROM props p JOIN nodes n ON n.id = p.node
        JOIN schema_field f ON f.field = p.key AND f.enum_name IS NOT NULL AND (f.owner = '*' OR "
                + &anc("n.type", "f.owner")
                + ")
        WHERE p.value IS NOT NULL AND p.value NOT IN (SELECT value FROM schema_enum WHERE name = f.enum_name)
      UNION ALL
      SELECT DISTINCT 'R9', p.node, p.key || ' = ' || p.value || ' sai mẫu ' || f.pattern FROM props p JOIN nodes n ON n.id = p.node
        JOIN schema_field f ON f.field = p.key AND f.pattern IS NOT NULL AND (f.owner = '*' OR "
                + &anc("n.type", "f.owner")
                + ")
        WHERE p.value IS NOT NULL AND NOT regexp(f.pattern, p.value)",
        ),
        (
            "v3_r10",
            "SELECT 'R10' AS rule, n.id AS node, r.value || ' do máy (' || n.proposed_by || ') ghi phải để proposed, đang ' || coalesce(n.status, '(trống)') AS detail
        FROM nodes n JOIN props r ON r.node = n.id AND r.key = 'relation'
        JOIN schema_edge x ON x.key = r.value AND x.human_only = 1
        WHERE n.type = 'claim' AND n.proposed_by IN ('llm', 'model', 'rule') AND coalesce(n.status, '') != 'proposed'
      UNION ALL
      SELECT DISTINCT 'R10', n.id, e.key || ' do máy (' || n.proposed_by || ') ghi ngoài claim proposed'
        FROM fm_links e JOIN nodes n ON n.id = e.src JOIN schema_edge x ON x.key = e.key AND x.human_only = 1
        WHERE n.proposed_by IN ('llm', 'model', 'rule') AND coalesce(n.status, '') != 'proposed' "
                .to_owned(),
        ),
        (
            "v3_r11",
            r"SELECT 'R11' AS rule, p.node, p.key || ' sai định dạng ngày: ' || p.value AS detail FROM props p JOIN nodes n ON n.id = p.node
        JOIN (SELECT DISTINCT field, owner, value_type FROM schema_field WHERE value_type IN ('date', 'datetime')) f
          ON f.field = p.key AND (f.owner = '*' OR "
                .to_owned()
                + &anc("n.type", "f.owner")
                + r")
        WHERE p.value IS NOT NULL AND (NOT regexp(CASE f.value_type WHEN 'date' THEN '^\d{4}-\d{2}-\d{2}$'
          ELSE '^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:?\d{2})?)?$' END, p.value)
          OR date(substr(p.value, 1, 10)) IS NULL)
      UNION ALL
      SELECT 'R11', node, 'asserted_at ở tương lai: ' || value FROM props WHERE key = 'asserted_at' AND substr(value, 1, 10) > date('now')
      UNION ALL
      SELECT 'R11', id, 'valid_from ' || valid_from || ' sau valid_to ' || valid_to FROM nodes
        WHERE valid_from IS NOT NULL AND valid_to IS NOT NULL AND valid_from > valid_to",
        ),
        (
            "v3_r12",
            "SELECT 'R12' AS rule, n.id AS node, 'superseded thiếu ' || k.f AS detail FROM nodes n
        JOIN (SELECT 'superseded_by' f UNION SELECT 'valid_to') k
        WHERE n.status = 'superseded' AND NOT EXISTS (SELECT 1 FROM props p WHERE p.node = n.id AND p.key = k.f AND p.value IS NOT NULL)
      UNION ALL
      SELECT 'R12', start, 'chuỗi superseded_by quay lại chính note' FROM (
        WITH RECURSIVE s(start, cur, k) AS (
          SELECT src, dst, 1 FROM links WHERE key = 'superseded_by' AND origin = 'frontmatter' AND dst IS NOT NULL AND dst != src
          UNION SELECT s.start, l.dst, s.k + 1 FROM s JOIN links l ON l.src = s.cur AND l.key = 'superseded_by' AND l.origin = 'frontmatter'
            WHERE l.dst IS NOT NULL AND s.k < 50 AND s.cur != s.start)
        SELECT DISTINCT start FROM s WHERE cur = start)"
                .to_owned(),
        ),
        (
            "v3_r13",
            "SELECT 'R13' AS rule, e.src AS node, 'khối không có trong note đích: ' || e.target || e.frag AS detail FROM fm_links e
        WHERE e.key = 'source' AND e.dst IS NOT NULL AND e.frag LIKE '#^%'
          AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.node = e.dst AND b.block = substr(e.frag, 3))
      UNION ALL
      SELECT 'R13', n.id, 'claim accepted không có source' FROM nodes n WHERE n.type = 'claim' AND n.status = 'accepted'
        AND NOT EXISTS (SELECT 1 FROM fm_links l WHERE l.src = n.id AND l.key = 'source' AND l.dst IS NOT NULL)"
                .to_owned(),
        ),
        (
            "v3_r14",
            "SELECT 'R14' AS rule, 'schema.yaml' AS node, detail FROM schema_problems".to_owned(),
        ),
        (
            "v3_r15",
            "SELECT 'R15' AS rule, e.src AS node, e.key || ' chỉ được một đích, đang có ' || count(DISTINCT coalesce(e.dst, e.target)) AS detail
        FROM fm_links e WHERE e.key IN (SELECT key FROM schema_edge WHERE cardinality = 'one')
          OR EXISTS (SELECT 1 FROM schema_field f WHERE f.field = e.key AND f.owner = e.st AND f.cardinality = 'one')
        GROUP BY e.src, e.key HAVING count(DISTINCT coalesce(e.dst, e.target)) > 1
      UNION ALL
      SELECT DISTINCT 'R15', b.claim, a.rel || ' chỉ được một đích cùng lúc: chồng cửa sổ với ' || a.claim
        FROM edges a JOIN edges b ON a.src = b.src AND a.rel = b.rel AND a.dst != b.dst AND a.claim < b.claim
        JOIN schema_edge x ON x.key = a.rel AND x.cardinality = 'one'
        WHERE a.origin = 'claim' AND b.origin = 'claim' AND a.status IN ('accepted', 'superseded') AND b.status IN ('accepted', 'superseded')
          AND coalesce(a.valid_from, '0000') <= coalesce(b.valid_to, '9999') AND coalesce(b.valid_from, '0000') <= coalesce(a.valid_to, '9999')"
                .to_owned(),
        ),
    ]
});

/// `RULES`: tên view `v3_rN`, sắp theo số luật.
pub fn rules() -> Vec<&'static str> {
    let mut r: Vec<&'static str> = VIEWS
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| n.starts_with("v3_r"))
        .collect();
    r.sort_by_key(|n| rule_number(&n[3..]));
    r
}

/// Số của luật `RN` (luật không đúng dạng xếp cuối).
fn rule_number(rule: &str) -> u32 {
    rule.get(1..)
        .and_then(|n| n.parse().ok())
        .unwrap_or(u32::MAX)
}

/// `create_views`: xoá rồi tạo lại mọi view và `v3_deviations`.
pub fn create_views(db: &Connection) -> rusqlite::Result<()> {
    for (name, _) in VIEWS.iter() {
        db.execute_batch(&format!("DROP VIEW IF EXISTS {name}"))?;
    }
    db.execute_batch("DROP VIEW IF EXISTS v3_deviations")?;
    for (name, sql) in VIEWS.iter() {
        db.execute_batch(&format!("CREATE VIEW {name} AS {sql}"))?;
    }
    let union = rules()
        .iter()
        .map(|v| format!("SELECT rule, node, detail FROM {v}"))
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    db.execute_batch(&format!("CREATE VIEW v3_deviations AS {union}"))
}

/// Số chỗ lệch và số note lệch của một luật.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Count {
    pub deviations: i64,
    pub notes: i64,
}

/// Một chỗ lệch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deviation {
    pub rule: String,
    pub node: Option<String>,
    pub detail: Option<String>,
}

impl Deviation {
    fn to_json(&self) -> Value {
        json!({"rule": self.rule, "node": self.node, "detail": self.detail})
    }
}

/// `counts`: luật -> số đếm, theo số luật tăng dần (chỉ luật có lệch).
pub fn counts(db: &Connection) -> rusqlite::Result<IndexMap<String, Count>> {
    let mut st =
        db.prepare("SELECT rule, count(*), count(DISTINCT node) FROM v3_deviations GROUP BY rule")?;
    let mut rows = st
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Count {
                    deviations: r.get(1)?,
                    notes: r.get(2)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // `sorted` của Python ổn định; cùng số luật thì giữ thứ tự SQLite trả.
    rows.sort_by_key(|(r, _)| rule_number(r));
    Ok(rows.into_iter().collect())
}

/// `deviations`: mọi chỗ lệch (hoặc của một luật), sắp theo luật, note, chi tiết. `limit` 0 hay `None` là không giới
/// hạn (như `if limit:` của Python); số âm cũng không giới hạn (`LIMIT -1` của SQLite).
pub fn deviations(
    db: &Connection,
    rule: Option<&str>,
    limit: Option<i64>,
) -> rusqlite::Result<Vec<Deviation>> {
    let rule = rule.filter(|r| !r.is_empty());
    let mut sql = String::from("SELECT rule, node, detail FROM v3_deviations");
    if rule.is_some() {
        sql.push_str(" WHERE rule = ?");
    }
    sql.push_str(" ORDER BY rule, node, detail");
    if let Some(n) = limit.filter(|n| *n != 0) {
        let _ = write!(sql, " LIMIT {n}");
    }
    let mut st = db.prepare(&sql)?;
    let map = |r: &rusqlite::Row<'_>| {
        Ok(Deviation {
            rule: r.get(0)?,
            node: r.get(1)?,
            detail: r.get(2)?,
        })
    };
    match rule {
        Some(r) => st.query_map([r], map)?.collect(),
        None => st.query_map([], map)?.collect(),
    }
}

/// Kết quả của lệnh `validate`: số đếm mọi luật, và ví dụ của các luật được hỏi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub counts: IndexMap<String, Count>,
    pub examples: IndexMap<String, Vec<Deviation>>,
}

/// Lệnh `validate [--rule R] [--limit N]`: số đếm, và tối đa `limit` (mặc định 5, như `a.limit or 5`) ví dụ cho mỗi
/// luật có lệch (hoặc chỉ luật `rule`).
pub fn report(db: &Connection, rule: Option<&str>, limit: Option<i64>) -> rusqlite::Result<Report> {
    let counts = counts(db)?;
    let limit = limit.filter(|n| *n != 0).unwrap_or(5);
    let rule = rule.filter(|r| !r.is_empty());
    let mut examples = IndexMap::new();
    for r in counts.keys() {
        if rule.is_none_or(|x| x == r) {
            examples.insert(r.clone(), deviations(db, Some(r), Some(limit))?);
        }
    }
    Ok(Report { counts, examples })
}

impl Report {
    /// `{"counts": ..., "examples": ...}` như `--json`.
    pub fn to_json(&self) -> Value {
        let counts: serde_json::Map<String, Value> = self
            .counts
            .iter()
            .map(|(r, c)| {
                (
                    r.clone(),
                    json!({"deviations": c.deviations, "notes": c.notes}),
                )
            })
            .collect();
        let examples: serde_json::Map<String, Value> = self
            .examples
            .iter()
            .map(|(r, v)| (r.clone(), v.iter().map(Deviation::to_json).collect()))
            .collect();
        json!({"counts": counts, "examples": examples})
    }

    /// Đầu ra văn bản của `validate` không `--json`, từng ký tự như Python (kể cả xuống dòng cuối).
    pub fn to_text(&self) -> String {
        let mut out = String::from("Lệch schema v3 (luật | số chỗ | số note):\n");
        for (r, c) in &self.counts {
            let _ = writeln!(out, "  {r:<4} {:>6} {:>6}", c.deviations, c.notes);
        }
        for (r, items) in &self.examples {
            let _ = writeln!(out, "\n{r}:");
            for d in items {
                let _ = writeln!(
                    out,
                    "  {}: {}",
                    d.node.as_deref().unwrap_or("None"),
                    d.detail.as_deref().unwrap_or("None")
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
