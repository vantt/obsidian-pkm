# Plugin UI for proposals, material and zones (product layout, step 5, second part)

Branch `product-b-plugin-ui` (from `origin/product-b` at `87eb6d8`). Code commit `4a1b5d6`, docs commit `ba1a280`. Status: done.

## Outcome

The Obsidian plugin now has the interactive half of `propose.py`, `material.py` and `zones.py` (research repo `9061581`). It calls `pkm` for every vault-wide computation and edits notes only through Obsidian's API. The plugin's apply, revert and stats were compared with the Python scripts on the invented propose fixture vault. Created notes, the stats page, every reverted file and the first log line match byte for byte. Edited notes match in their properties and body bytes. The material page matches `material.render` byte for byte, except the `generated_by` line.

## Commands (all in `apps/obsidian-plugin/`)

| Command | What it does |
|---|---|
| Đề xuất chờ duyệt | Opens a modal listing `_Đề xuất/` proposals: the ones already marked first, then the waiting ones. Clicking a name opens the proposal note. Nhận and Bỏ (with an optional reason, saved to `reject_reason`) only change `status`. Chọn lại puts a proposal back to `chờ`. Làm mới and Áp dụng buttons are in the modal. |
| Làm mới đề xuất | Runs `pkm VAULT pairs --write [--db]`, then `pkm VAULT propose --create --similar VAULT/_E11/similar-pairs.json --json`. If pairs fails (including `ohs_not_ready`), the plugin shows the usual notice and runs propose without `--similar`, so there is no `related` kind. |
| Áp dụng đề xuất đã chọn | Does what Python `--apply` does for each kind. Then it sets `status`, `closed` and `asserted_at`, and appends the same line to `_E11/proposals-log.jsonl`. |
| Hoàn tác đề xuất đã áp dụng | Asks first, then does what Python `--revert` does. The log is renamed to `.reverted-<stamp>`. |
| Đo duyệt đề xuất | Does what Python `--stats` does, writes `_E11/Đo duyệt.md`, and opens it. |
| Nguyên liệu cho bản nháp này | Moves the previous page's ticks into `uses` and logs them to `_E11/material-log.jsonl`. Then it runs `pkm VAULT material DRAFT [--db]`, rewrites `_Xưởng/Nguyên liệu/<draft>.md` with the ticks kept, and opens the page in a split pane. It also works from the material page itself. |
| Gợi ý bài viết (vùng tri thức) | Runs `pkm VAULT zones --apply` when `_Xưởng/Gợi ý bài viết.md` is missing (pkm writes the page), otherwise `--write`. It then shows the zones in a modal with the grade already written on the page, the core notes, the highlights, and a "Viết bài từ vùng này" button. |

New setting: **Index graph (`--db`)**, stored in `data.json` as `graphDbPath`. It is passed to material, zones and pairs.

## Files

- New: `src/text.ts` (frontmatter split, CRLF, Python string semantics, local stamps), `src/note-io.ts` (`NoteIO` and `ObsidianNoteIO`: `processFrontMatter`, `vault.process`, adapter append and rename, link resolution), `src/open-note.ts`, `src/proposals/{apply,stats,refresh,commands}.ts`, `src/material/{page,flow,command}.ts`, `src/zones/{zones,commands}.ts`.
- Changed: `src/main.ts` (commands, `callPkm`, `vaultPath`, `dbArgs`), `src/bridge.ts` (option `json: false`), `src/settings.ts`, `src/settings-tab.ts`, `src/workshop/digest.ts` (`pyJson` can keep key order), `src/workshop/command.ts` (exports `ConfirmModal`), `styles.css`.
- Tests: `test/memory-note-io.ts` (in-memory vault that behaves like `processFrontMatter`), `test/proposals.test.ts`, `test/writing.test.ts`, plus additions to `test/bridge.test.ts` and `test/settings-and-path.test.ts`.
- Goldens: `test/fixtures/python-proposals.json` and `test/fixtures/python-writing.json`, made by `make-python-proposals.py` and `make-python-writing.py` from the research repo at `9061581`. All data in them is invented: the vault from `crates/pkm-core/tests/propose/vault` plus one topic page, and hand-made material and zone input.
- Docs: `apps/obsidian-plugin/README.md` (Vietnamese) and `spec/PORTING.md` (a "Phần plugin" part in the propose section and in the material/zones section, plus two test-table rows).

## Checks

- `pnpm test`: 63 tests in 6 files pass.
- `pnpm run build` (version check, `tsc --noEmit`, esbuild) passes.

## Intended differences from Python (details in spec/PORTING.md)

- **Properties are edited through `processFrontMatter`.** Obsidian rewrites the YAML block in its own layout. Property values and body bytes are kept, and CRLF frontmatter is restored to CRLF.
  - This changes the material contract's rule "the rest of the file stays byte for byte": it now holds for the body only. PORTING.md is updated.
  - In the proposals log, the old text of a note that an earlier proposal in the same run had already edited differs from Python in YAML layout only. Revert is still exact.
- **`fix-link` edits only the body.** Python re-dumped the frontmatter, and added `---\n{}\n---` to notes that had no frontmatter.
- **A failing proposal does not stop the run.** If a target note is missing or `notes`/`data` is malformed, that proposal's edits are rolled back, it stays `nhận`, and the error is reported. Python crashed part way through.
- **A single `related`/`who` string is treated as a one-item list.**
- **Revert checks the whole log before changing any file.**
- **Stats skip broken log lines and non-`.md` files.** The explanation says "plugin PKM" instead of "plugin e11-review-log".
- **Material: "in the index" becomes "exists in the vault".** The plugin has no DB access. "Already used" is read from the draft's resolved `uses` links. The log's `via` is `pkm plugin tick`, and the page says `generated_by: pkm material`.
- **Material: ticks are kept on the refreshed page.** A draft without frontmatter gets one.
- **Zones post creation goes through the vault API.** It writes the same content as the page's `obsidian://new` link. A post that already exists is opened, not overwritten.

## Concerns

- **The task said to call material with `--json`, but `pkm VAULT material`, `zones` and `pairs` reject the flag.** They always print JSON (clap `Args` in `crates/pkm/src/cli.rs`, module `writing`). The plugin calls them without the flag.
- **`pkm VAULT propose` has no `--db` option**, so the graph index setting is not passed to propose.
- **The `processFrontMatter` behaviour is not verified in a running Obsidian.** The mock reproduces its effect (LF YAML, body kept). Two points are untested: that Obsidian recognises a CRLF frontmatter block, and that it accepts the local timestamp strings unchanged. One manual run on a test vault is advised before release.
- **The proposal note body is written by Rust (`crates/`, outside my scope).** It still tells the user to run `propose.py --apply`. It should point to the plugin's "Áp dụng đề xuất đã chọn" command.
- **I did not run a smoke test against a built `pkm` binary.** A hook blocks paths containing `target`. I checked the argument shapes against `cli.rs` and `crates/pkm/tests/cli_{propose,writing}.rs` instead.

## Unresolved questions

- `pkm setup` must write the graph index path into the plugin's `data.json` under the key `graphDbPath`. Is that key acceptable to the setup agent?
- Should the material log's `via` keep the old value `material.py tick` for continuity in research analyses?
