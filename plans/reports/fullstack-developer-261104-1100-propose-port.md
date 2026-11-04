# Port `propose.py` into `pkm-core` (product layout, step 6, `propose` part)

Branch `product-b-propose` (from `origin/product-b` at `561aafe`), code commit `14c024b`. Status: done.

## Outcome

The computing half of `scripts/e11/propose.py` (research repo `9061581`) now lives in `pkm_core::propose`, exposed as
`pkm VAULT propose [--similar FILE] [--create] [--json]`. On an invented fixture vault covering all seven kinds, Rust
and Python produce the same proposals and the same proposal-note bytes. Running `--create` with each version on two
copies of the vault writes identical files with identical names, and each version reads the queue the other wrote
and adds nothing. On kg-solo (60 `person` proposals) the Rust output equals the Python output.

## Files

- `crates/pkm-core/src/propose.rs` (new): loading the vault, the seven rules, reading the queue, dedupe and cap,
  `proposal_note`, `write` (only `create_new` files in `_Đề xuất/`).
- `crates/pkm-core/src/propose/dump.rs` (new): a writer that produces the same YAML as PyYAML's
  `safe_dump(sort_keys=False, allow_unicode=True, width=10_000)`. Its plain-or-quoted choice reuses the existing
  YAML loader as the resolver.
- `crates/pkm-core/src/propose/tests.rs`, `src/propose/pyyaml-dump-cases.json` (new): 15 unit tests, including 74
  tricky strings, floats and nested containers checked against PyYAML 6.0.3 output.
- `crates/pkm-core/tests/propose.rs` and `tests/propose/` (new): the invented vault, `similar.json`, Python-generated
  `expected.json` and `expected-notes.json`, and the `kg-solo.json` golden.
- `crates/pkm-core/tests/.gitattributes`: added `propose/** -text`, so the fixture keeps its exact bytes (one CRLF
  note) on Windows checkouts.
- `crates/pkm-core/src/lib.rs`: one line, `pub mod propose;`.
- `crates/pkm/src/cli.rs`: one dispatch `if` placed right after the `mcp` check, one usage line, and a self-contained
  `ProposeCmd` and `propose_command` block placed before `mod tests`. `OPS` and `VaultArgs` are untouched, which
  keeps merges with the `material`/`zones` work easy.
- `crates/pkm/tests/cli_propose.rs` (new): 5 CLI tests.
- `spec/PORTING.md`: a new section, "Module `propose`", covering scope, the JSON contract, what was kept on purpose
  and the intended differences.

No new dependencies. No docs change: `docs/` has no user command list.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and
  `cargo test --workspace --locked` all pass.
- `PKM_KG_SOLO=<kg-solo> cargo test -p pkm-core --test propose` passes.

## Intended differences from Python (details in spec/PORTING.md)

- Rust walks the vault in sorted path order. This only changes the order between several `duplicate` or
  `same-topic` groups.
- Note paths are NFC.
- A bad frontmatter, an unreadable file or an odd queue entry is skipped. Python would crash.
- `--similar` is validated and reports errors.
- Only `--create` requires `.obsidian/`.
- A non-string `proposed_type` is converted with `str()`.
- `--json` is new.
- Kept on purpose: `extracted_by: propose.py <kind>@schema_v3`, and the body line that says to run
  `propose.py --apply`. Until the plugin exists, the Python apply is still the only way to apply.

## Plugin work (not ported) and its JSON contract

Get proposals with `pkm VAULT propose --json [--similar FILE] [--create]`:

```json
{"total": 60, "new": 20, "by_kind": {"person": 20},
 "proposals": [{"kind": "...", "notes": ["a.md", "..."], "from_text": "...", "to": "...", "value": "...",
                "score": 0.81, "reason": "...", "on_accept": "...", "lines": ["..."]}],
 "written": ["_Đề xuất/Người - X.md"]}
```

- Optional keys depend on the kind: `from_text` and `to` (fix-link), `value` (type-batch, person), `score` (related,
  may be `null`), `lines` (idea, person). `written[i]` belongs to `proposals[i]`, and the list is empty without
  `--create`.
- A proposal note has this frontmatter, in this order: `type: proposal`, `kind`, `status: chờ`, `created`,
  `proposed_by`, `extracted_by`, `notes` (wikilinks `[[path-without-.md|title]]`), `reason`, and `data` when it
  applies (`{from_text, to}` / `{value}` / `{score}`).
- The plugin must implement, the same way as Python's `apply`/`revert`/`stats` at `9061581`:
  1. **Decide.** The user sets `status` to `nhận` or `bỏ`, and can optionally fill `reject_reason` (or `lý do bỏ`).
  2. **Apply on `nhận`.** Each kind edits notes as follows:
     - duplicate: add `duplicate_of: <notes[0] wikilink>` to `notes[1:]`.
     - same-topic and related: add the other note to `related` on both notes.
     - fix-link: in each source (`notes[:-1]`), replace `[[{from_text}` with `[[{to without .md}`.
     - type-batch: set `type` to the `proposed_type` value (or `data.value`), then remove `proposed_type`.
     - idea: create a new draft `Ý/<title[:60]> - ý.md`. Its frontmatter is `type: note`, `topic` (the nearest
       ancestor `<dir>/<dir> (chủ đề).md` as a link, or empty), `gist: ''`, `source: [notes[0]]`, `wise: null`,
       `use: null`, `wf: [to-clarify]`. Its body has a "## Ý chính (lời mình)" section, then "## Từ nguồn" with
       the highlights quoted, then "## Mình thấy gì, đem thử ở đâu".
     - person: create `Người/<name>.md` (`type: person` plus the Bases table of notes that link to it) if it is
       missing, and append `[[Người/<name>|<name>]]` to `who` on each note.
     - On every edit, keep the note's CRLF or LF line endings.
  3. **Close.** Set `status` to `đã áp dụng` or `đã bỏ` and `closed` to an ISO timestamp, plus `asserted_at` (the
     date) on accept. Then append one JSONL line to `_E11/proposals-log.jsonl`:
     `{at, proposal, proposal_old, decision, reject_reason, changes: {path: old_text|null}}`.
  4. **Revert.** Replay the log newest first: restore the old texts, delete files created as `null`, restore
     `proposal_old`, then rename the log to `.reverted-<stamp>`.
  5. **Stats.** For each kind: decided, accept rate, decisions that have a log, how often a source note was opened
     between opening the proposal and deciding, and the median seconds per proposal. The input is
     `_E11/review-log.jsonl` (`{at, event: open|status, path, status}`). The output is `_E11/Đo duyệt.md`.

## Concerns

- CI runs `cargo test` before it builds kg-solo, so `kg_solo_matches_golden` is skipped there, like the other
  `PKM_KG_SOLO` tests. The invented-vault parity tests do run in CI. Making the kg-solo golden run in CI needs one
  line in `ci.yml` (`PKM_KG_SOLO=$RUNNER_TEMP/kg-solo cargo test -p pkm-core --test propose`), which is outside
  this task's file list.
- The "run on vault B through a private command that prints only counts" step of phase 6 was not done (no access
  to vault B here). `pkm VAULT propose` without `--json` prints only counts plus up to six sample note names, so
  run it as `pkm VAULT propose | head -1` for counts only.
- The worktree isolation blocked writing this report into the primary checkout's `plans/reports/`. It exists only
  as the committed copy in this branch.

## Unresolved questions

- Should `extracted_by` switch to `pkm propose <kind>@schema_v3`, and should the body's apply hint name the plugin
  once the plugin exists? Both were kept as they are for now (see spec/PORTING.md).
