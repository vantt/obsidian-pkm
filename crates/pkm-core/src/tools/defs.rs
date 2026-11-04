//! Định nghĩa tool cho `tools/list` của MCP: tên, mô tả, schema tham số.
//!
//! Đặc tả: `tool_defs` của `scripts/kg/mcp_server.py` ở sha ghim. Chuỗi chép nguyên, khoá theo đúng thứ tự Python
//! (client thấy đúng các câu này, nên câu chữ là một phần của hợp đồng MCP).
//!
//! Lệch có chủ ý của một cửa (`docs/MCP-CHANGELOG.md`): thêm `search` (sau `find`) và `read` (sau `note`), viết lại
//! `related`. Mười tool còn lại giữ nguyên chữ của Python.

use serde_json::{Map, Value, json};

const NODE_DESC: &str = "Note id = vault path exactly as returned by find/other tools, e.g. \"Khách hàng/Công ty ABC.md\".";
const SYSTEM_DESC: &str = "true = also walk through and return system pages (generated pages, proposal queue) that are hidden by default because they link to everything.";
const LIMIT_DESC: &str = "Items shown before the result is summarised (default 20). Long results return count, by_type, the first items and a handle for `more`.";

/// Một tool: tên, mô tả, thuộc tính của schema tham số, tham số bắt buộc.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub properties: Map<String, Value>,
    pub required: &'static [&'static str],
}

impl ToolDef {
    /// Mục của `tools/list`: `name`, `description`, `inputSchema` (`type`, `properties`, `required`).
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": {"type": "object", "properties": self.properties, "required": self.required},
        })
    }
}

fn str_() -> Value {
    json!({"type": "string"})
}

fn node() -> Value {
    json!({"type": "string", "description": NODE_DESC})
}

fn limit() -> Value {
    json!({"type": "integer", "description": LIMIT_DESC})
}

fn system() -> Value {
    json!({"type": "boolean", "description": SYSTEM_DESC})
}

/// `FILTERS`, thêm `system` khi vault có note hệ thống.
fn filters(hidden: bool) -> Vec<(&'static str, Value)> {
    let mut f = vec![
        (
            "rels",
            json!({"type": "array", "items": str_(), "description": "Only these relation names (forward names from the server instructions; inverse names are produced automatically when walking backwards). Omit = all relations."}),
        ),
        (
            "as_of",
            json!({"type": "string", "description": "YYYY-MM-DD. Only edges whose validity window (valid_from..valid_to) contains this day: who works where, which contract/claim held then. Use today for \"now/currently/đang\". Omit = ignore time."}),
        ),
        (
            "statuses",
            json!({"type": "array", "items": {"type": "string", "enum": ["accepted", "superseded", "proposed", "rejected"]},
                   "description": "Edge layers counted. Default: accepted + superseded + proposed (machine suggestions, marked layer \"proposed\"). [\"proposed\"] = only unreviewed suggestions."}),
        ),
        (
            "accepted_only",
            json!({"type": "boolean", "description": "true = drop machine-suggested (proposed) edges; keep only what the user wrote or approved."}),
        ),
    ];
    if hidden {
        f.push(("system", system()));
    }
    f
}

/// Ghép thuộc tính theo thứ tự: `head`, các bộ lọc `mid`, rồi `tail`.
fn props(
    head: Vec<(&'static str, Value)>,
    mid: Vec<(&'static str, Value)>,
    tail: Vec<(&'static str, Value)>,
) -> Map<String, Value> {
    head.into_iter()
        .chain(mid)
        .chain(tail)
        .map(|(k, v)| (k.to_owned(), v))
        .collect()
}

fn str_or_list(description: &str) -> Value {
    json!({"oneOf": [{"type": "string"}, {"type": "array", "items": {"type": "string"}}], "description": description})
}

/// Tool `search` của một cửa.
fn search_def(s: Vec<(&'static str, Value)>) -> ToolDef {
    ToolDef {
        name: "search",
        description: "Find notes by meaning or words: topics, ideas, half-remembered content (\"cái bài về…\"), questions whose wording differs from the note, bilingual (question in Vietnamese, note in English), typos and missing diacritics. Hybrid BM25 + embeddings via the vault's search index. Use it when the user DESCRIBES what a note is about; use `find` when the user NAMES a note, person or thing; use `select` for every note of a type. Returns ranked items: id (note id, the same id the graph tools take), title, type, score (0-1), snippet. A snippet is an excerpt: call `read` before quoting or judging relevance. Result {\"error\":\"ohs_not_ready\",...} means the search index is starting or indexing (state, progress, eta_s, action): tell the user, answer with find / select / graph tools meanwhile, and never conclude from it that a note does not exist.",
        properties: props(
            vec![
                (
                    "query",
                    json!({"type": "string", "description": "What to look for, in the user's words (Vietnamese or English). Omit only when `path` is given."}),
                ),
                (
                    "queries",
                    json!({"type": "array", "items": str_(), "description": "2-4 reformulations searched together and merged (better recall; add an English or Vietnamese translation of the question when the note may be in the other language)."}),
                ),
                (
                    "path",
                    json!({"type": "string", "description": "Note id. Return notes similar in meaning to this note instead of searching text (what an isolated note could connect to). Ignores query."}),
                ),
                (
                    "mode",
                    json!({"type": "string", "enum": ["hybrid", "semantic", "fulltext", "title"], "description": "hybrid (default) for most questions; semantic when the user's wording differs from the note's; fulltext for an exact term; title only to navigate to a named note (prefer `find`)."}),
                ),
                (
                    "scope",
                    str_or_list(
                        "Folder prefix(es) to search in, e.g. \"Dự án/\"; prefix \"-\" to exclude. Only when the user limits the area.",
                    ),
                ),
                (
                    "tag",
                    str_or_list(
                        "Tag(s) every result must have; prefix \"-\" to exclude. Only when the user names a tag.",
                    ),
                ),
                (
                    "limit",
                    json!({"type": "integer", "description": "Items returned (default 10; keep <= 15, later ranks are weak)."}),
                ),
                (
                    "snippet_length",
                    json!({"type": "integer", "description": "Snippet characters per item (default 200; 0 = none)."}),
                ),
            ],
            s,
            vec![],
        ),
        required: &[],
    }
}

/// Tool `read` của một cửa.
fn read_def() -> ToolDef {
    ToolDef {
        name: "read",
        description: "Full body text of one or more notes (frontmatter removed), for quoting and for judging relevance. `note` gives metadata and edge counts; `read` gives the text. Pass note ids exactly as returned by search / find / related. Reads several notes in ONE call: after search or related, read the top 5-8 together instead of one call each. Bodies longer than max_chars are cut with truncated:true and chars = full length; call again with offset for the rest. Unknown id: that item carries error + did_you_mean, the others are still returned. Works while the search index is still starting.",
        properties: props(
            vec![
                (
                    "nodes",
                    json!({"type": "array", "items": str_(), "description": "Note ids (vault paths), 1-20."}),
                ),
                (
                    "max_chars",
                    json!({"type": "integer", "description": "Characters per note (default 4000; 0 = whole note). The whole reply is also capped by the server; with many notes each body is cut evenly."}),
                ),
                (
                    "offset",
                    json!({"type": "integer", "description": "Skip this many characters of each body (default 0)."}),
                ),
            ],
            vec![],
            vec![],
        ),
        required: &["nodes"],
    }
}

/// `tool_defs(hidden)`: mọi tool theo thứ tự của `tools/list`. `hidden`: vault có note hệ thống (thêm tham số
/// `system`).
pub fn definitions(hidden: bool) -> Vec<ToolDef> {
    let f = || filters(hidden);
    let s = || {
        if hidden {
            vec![("system", system())]
        } else {
            Vec::new()
        }
    };
    vec![
        ToolDef {
            name: "find",
            description: "Look up notes by name the user typed: every word of `name` must match a word of a note title or alias (case and Vietnamese diacritics ignored); an exact vault path also works. Returns id, title, type, aliases. Use it first whenever the user names something. Several hits that are different things (two people with similar names): ask the user which one, do not pick. Hits that are the same thing (duplicate notes): say so.",
            properties: props(
                vec![
                    ("name", str_()),
                    (
                        "type",
                        json!({"type": "string", "description": "Optional note type filter (see instructions)."}),
                    ),
                    ("limit", limit()),
                ],
                vec![],
                vec![],
            ),
            required: &["name"],
        },
        search_def(s()),
        ToolDef {
            name: "note",
            description: "Read one note: type, frontmatter properties, aliases, validity window, provenance fields (status, proposed_by, extracted_by, asserted_at), edge counts by relation and layer, block ids, and the start of the body. For a claim note also the edge it asserts.",
            properties: props(
                vec![
                    ("node", node()),
                    (
                        "body_chars",
                        json!({"type": "integer", "description": "Characters of body text to include (default 800, 0 = none)."}),
                    ),
                ],
                vec![],
                vec![],
            ),
            required: &["node"],
        },
        read_def(),
        ToolDef {
            name: "neighbors",
            description: "Typed edges around one note, one step. Each item: rel (name in walking direction; incoming edges use the inverse name, e.g. has_project for an incoming client edge), node, type, dir, status and layer (accepted | proposed), claim (the claim note behind a causal/fact edge), valid_from/valid_to, source (note#^block), proposed_by, extracted_by, confidence. Good for \"what is X connected to\", \"who works at X now\" (rels + as_of), \"what increases X\" (dir in).",
            properties: props(
                vec![
                    ("node", node()),
                    (
                        "dir",
                        json!({"type": "string", "enum": ["out", "in", "both"], "description": "out = edges written on this note (forward), in = edges pointing at it, both (default)."}),
                    ),
                ],
                f(),
                vec![("limit", limit())],
            ),
            required: &["node"],
        },
        ToolDef {
            name: "reach",
            description: "Every note reachable from `node` by repeatedly following edges, start excluded, with the smallest depth when max_depth is set. dir \"out\" = downstream (everything X leads to / affects / causes), \"in\" = upstream (everything that leads to / influences X). One call answers broad multi-hop questions (\"all factors that eventually drive revenue\"); do not walk neighbors step by step for that. Long results are summarised by type; report the count and grouping rather than copying every id.",
            properties: props(
                vec![
                    ("node", node()),
                    (
                        "dir",
                        json!({"type": "string", "enum": ["out", "in", "both"], "description": "Default out (downstream)."}),
                    ),
                    (
                        "max_depth",
                        json!({"type": "integer", "description": "Max hops (omit = unlimited)."}),
                    ),
                ],
                f(),
                vec![("limit", limit())],
            ),
            required: &["node"],
        },
        ToolDef {
            name: "paths",
            description: "Top-k simple directed paths from one note to another (shortest first), at most max_len edges. Each path: nodes, rels, statuses per step, sign (+/-, net effect from relation polarity: increases +, decreases/prevents -), layer (accepted only if every step is accepted). Use for \"how does X affect Y\", \"why\", \"is X connected to Y\".",
            properties: props(
                vec![
                    ("from", node()),
                    ("to", node()),
                    (
                        "max_len",
                        json!({"type": "integer", "description": "Max edges per path (default 6)."}),
                    ),
                    (
                        "k",
                        json!({"type": "integer", "description": "Paths shown (default 10)."}),
                    ),
                ],
                f(),
                vec![],
            ),
            required: &["from", "to"],
        },
        ToolDef {
            name: "loops",
            description: "Feedback loops (simple directed cycles) through one note, at most max_len edges; both are required (whole-vault cycle search is too expensive). Each loop: nodes (starting at `node`), rels, negatives (number of negative links), kind (reinforcing if even, balancing if odd), layer. Use rels = causal relations (increases, decreases, prevents, conditions) for vicious/virtuous circles.",
            properties: props(
                vec![
                    ("node", node()),
                    (
                        "max_len",
                        json!({"type": "integer", "description": "Max edges per loop, e.g. 6; keep it small (<= 10)."}),
                    ),
                ],
                f(),
                vec![("limit", limit())],
            ),
            required: &["node", "max_len"],
        },
        ToolDef {
            name: "provenance",
            description: "Where a claim (or any note) comes from: cited sources with the #^block they point to, notes that support or dispute it, record (status, proposed_by, extracted_by, asserted_at, validity window), the edge it asserts, and the chain of notes that superseded it.",
            properties: props(vec![("claim", node())], vec![], vec![]),
            required: &["claim"],
        },
        ToolDef {
            name: "select",
            description: "Notes whose frontmatter matches every given key = value (string compare). Named filters: `type` (note type), `status` (review state: proposed = waiting for the user to review, accepted, superseded, rejected) and `proposed_by` (WHO proposed it: llm, model, rule, human). These are different: \"created / proposed by the AI/LLM\" = proposed_by \"llm\" (any status, any type); \"waiting for review\" = status \"proposed\" (by anyone). Other keys go in `where`. Use for listing notes of a type, e.g. type \"tool\" for the tools / software the user works with.",
            properties: props(
                vec![
                    (
                        "type",
                        json!({"type": "string", "description": "Note type."}),
                    ),
                    (
                        "status",
                        json!({"type": "string", "enum": ["proposed", "accepted", "superseded", "rejected"]}),
                    ),
                    (
                        "proposed_by",
                        json!({"type": "string", "description": "llm | model | rule | human"}),
                    ),
                    (
                        "where",
                        json!({"type": "object", "description": "Other frontmatter key: value pairs, e.g. {\"confidence\": \"established\"}."}),
                    ),
                    ("limit", limit()),
                ],
                s(),
                vec![],
            ),
            required: &[],
        },
        ToolDef {
            name: "valid",
            description: "Notes of one type whose own validity window (valid_from..valid_to) contains a day: contracts, decisions, price lists, claims in force on that date.",
            properties: props(
                vec![
                    (
                        "type",
                        json!({"type": "string", "description": "Note type, e.g. contract, decision, claim."}),
                    ),
                    (
                        "as_of",
                        json!({"type": "string", "description": "YYYY-MM-DD"}),
                    ),
                    ("limit", limit()),
                ],
                s(),
                vec![],
            ),
            required: &["type", "as_of"],
        },
        ToolDef {
            name: "related",
            description: "Notes related to a question or a note, ranked by personalised PageRank over the typed graph and seeded by semantic search: ONE call = search + graph walk. Give `query` (the user's question or an idea, in their words), or `node` (a note id: start from notes similar in meaning to it, e.g. what an isolated note could connect to), or `hits` (search results you already hold; skips the search). Returns seeds (the search hits re-ranked by graph support, found_by \"ohs\", with snippet when seeded by query or node) and related (notes only the graph reached, found_by \"graph\": what text search missed), each with via = the edge chain from a seed and layer (accepted | proposed, say which). Use for \"liên quan gì / nối với gì / có thể nối với gì\"; do not run search first just to feed hits; then `read` the top notes before stating a relation. accepted_only=true shows what the user's own links contribute. Result {\"error\":\"ohs_not_ready\",...}: as in search.",
            properties: props(
                vec![
                    (
                        "query",
                        json!({"type": "string", "description": "Question or idea to search, then walk the graph from the hits."}),
                    ),
                    (
                        "queries",
                        json!({"type": "array", "items": str_(), "description": "2-4 rewordings searched together (e.g. the question in English and Vietnamese) for better seeds."}),
                    ),
                    (
                        "node",
                        json!({"type": "string", "description": "Note id: seeds are the notes most similar in meaning to this note (the note itself excluded)."}),
                    ),
                    (
                        "hits",
                        json!({"type": "array", "items": {}, "description": "Search results you already have: objects with id or path (and score), or id strings, in search order. Skips the search step. Precedence: hits, then node, then query/queries."}),
                    ),
                    (
                        "k",
                        json!({"type": "integer", "description": "Graph-only notes returned (default 15)."}),
                    ),
                    (
                        "types",
                        json!({"type": "array", "items": str_(), "description": "Only return notes of these types."}),
                    ),
                ],
                f(),
                vec![],
            ),
            required: &[],
        },
        ToolDef {
            name: "more",
            description: "Next items of a long result, by the handle it returned. Only when you really need the items (the user asked for the full list); otherwise answer from count and by_type.",
            properties: props(
                vec![
                    ("handle", str_()),
                    (
                        "offset",
                        json!({"type": "integer", "description": "Default 0."}),
                    ),
                    (
                        "limit",
                        json!({"type": "integer", "description": "Default 20."}),
                    ),
                ],
                vec![],
                vec![],
            ),
            required: &["handle"],
        },
    ]
}

/// Kết quả `tools/list`: `{"tools": [...]}`.
pub fn tools_list(hidden: bool) -> Value {
    json!({"tools": definitions(hidden).iter().map(ToolDef::to_json).collect::<Vec<_>>()})
}
