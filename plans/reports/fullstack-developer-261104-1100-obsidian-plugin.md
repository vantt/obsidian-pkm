# Phase Implementation Report: Obsidian plugin skeleton and workshop

## Executed Phase
- Phase: step 5 of `plans/261103-product-layout/plan.md`, first part (plugin skeleton, "Dựng xưởng", merged review log, folder grid and table zoom)
- Plan: `plans/261103-product-layout/`
- Branch: `product-b-plugin` (from `origin/product-b` at `561aafe`)
- Status: completed. Not yet run inside a real Obsidian (no Obsidian or Obsidian CLI on this machine).

## Files
All new, except `.gitignore`:

- `apps/obsidian-plugin/`: `manifest.json` (id `pkm`, version `0.1.0-rc.4`, `minAppVersion` 1.9.0, `isDesktopOnly: true`), `package.json`, `tsconfig.json`, `esbuild.config.mjs`, `vitest.config.ts`, `.gitattributes` (LF, because templates are hashed), `styles.css`, `README.md` (Vietnamese), `scripts/check-version.mjs`
- `src/`: `main.ts`, `settings.ts`, `settings-tab.ts`, `pkm-path.ts`, `bridge.ts`, `review-log.ts`, `table-zoom.ts`, `text-modules.d.ts`, `workshop/{plan,apply,digest,obsidian-io,command}.ts`, `workshop/templates/` (16 static files)
- `test/`: `obsidian-mock.ts`, `memory-io.ts`, four `*.test.ts` files, `fixtures/python-workshop.json`
- Repo root: `pnpm-workspace.yaml`, `package.json`, `pnpm-lock.yaml`, and `.gitignore` (adds `node_modules/` and `/apps/obsidian-plugin/dist/`)

## Tasks Completed
- [x] TS + esbuild skeleton. The manifest version equals the Cargo workspace version. `pnpm run build` writes `main.js`, `manifest.json` and `styles.css` to `dist/` (gitignored) after it checks the version and runs the typecheck.
- [x] Settings tab:
  - The pkm path is auto-detected in this order: `PKM_INSTALL_DIR`, then `%LOCALAPPDATA%\pkm\bin\pkm.exe` on Windows or `~/.local/bin/pkm` elsewhere.
  - A "Kiểm tra" button runs `pkm --version`.
  - Other settings: the timeout, the topic roots and extra capture folders.
- [x] Bridge (`src/bridge.ts`):
  - It runs `pkm <args> --json` through `execFile` with a timeout, a 64 MB output cap and `windowsHide`, then parses stdout whatever the exit code is.
  - Every failure becomes a `PkmError` with a Vietnamese message. The cases are: not configured, ENOENT, EACCES/EPERM, timeout, output that is not JSON, a non-zero exit, `{"error": "text"}`, the doctor envelope `{"error": {code, why, fix}}`, and `ohs_not_ready`.
  - For `ohs_not_ready`, the message depends on `state` and includes the percentage, the time left and `action`. `progress` and `eta_s` may be null.
  - A "Kiểm tra pkm (doctor)" command uses the bridge.
- [x] "Dựng xưởng" and "Gỡ xưởng" are ported from `workshop_ui.py`:
  - Templates are static files bundled as text, and JS only fills `{{quick}}`, `{{inboxFilter}}` and `{{name}}`.
  - Output is byte-identical to the Python script. A golden test checks this on a neutral fixture vault, covering paths, order, content, digests and the homepage and templater configs.
- [x] The rule against overwriting edited files is ported as-is:
  - The manifest keeps the same path and format as the Python one (`_E11/workshop-ui-manifest.json`), including the parsed-YAML digest for `.base` files. Windows backslash keys are normalized.
  - Clash means a planned file already exists but the workshop did not write it. When that happens, nothing is written.
  - Edited files are kept.
  - Stale files are removed unless edited.
  - Configs are saved before the first write and restored on removal.
  - A config file that is not valid JSON stops the run before any note is written.
- [x] Writes to notes and bases go through `vault.process` (existing files) or `vault.create`. Deletes go through `fileManager.trashFile`. Hidden paths and configs use the adapter.
- [x] The review log, folder-grid CSS and table zoom are merged into this plugin. CSS classes are renamed `e11-zoom*` → `pkm-zoom*`, so they cannot collide with the old plugin.
- [x] The workshop's migration from the Python setup:
  - It removes the old `e11-review-log` plugin files and the `e11-folder-grid.css` snippet if they are unchanged. Both were in the Python manifest.
  - It drops `e11-review-log` and `e11-table-zoom` from `community-plugins.json`.
  - It drops `e11-folder-grid` from `enabledCssSnippets`.
- [x] Unit tests use vitest with a mock `obsidian` module.

## Decisions
- **Package manager:** pnpm 12.10.1, which is installed on this machine. The root `package.json` pins `packageManager`. `pnpm-workspace.yaml` has `allowBuilds: { esbuild: true }` because pnpm 12 blocks dependency build scripts by default.
- **Theme:** the theme stays a separate CSS snippet that `pkm setup` copies from `presets/b/`, not part of the plugin. The reasons:
  - The embedded fonts are about 1 MB as base64.
  - It only fits the Primary theme.
  - The user must be able to turn it off without losing the plugin.

  The plugin's `styles.css` carries only its own components (the folder grid and the zoom button). Note for whoever ports `theme.mjs install` into `pkm setup`: stop installing the `e11-table-zoom` plugin, because this plugin replaces it.
- **Real vault content:** `workshop_ui.py` hard-coded two topic roots and one capture-folder path from the real vault (`TOPIC_ROOTS`, `INBOXES`). They are not copied here. Both are now settings: "Thư mục chủ đề" (default empty, which means no topic pages) and "Thư mục lưu nhanh khác" (default empty; `Inbox` is always listed). To reproduce bản B, the user enters the values from the Python script's constants into settings. The fixture uses neutral folders (`Notes/1. Projects/Alpha` and similar). A grep of the commit for those real folder names finds nothing.
- **Kept as-is (schema and workflow vocabulary, not note content):** the station tags `to-*`, the paths `_E11/`, `_Đề xuất/`, `_Xưởng/`, `_Templates/`, and the Vietnamese UI text of the templates.
- **Folder names with quotes** (`"` or `'`) are rejected in settings, because they would break the `.base` filter.

## Deviations from the Python and source plugins
- No offline view counts in the dry run. Inside Obsidian, the views themselves show the counts. The confirmation dialog lists the topic pages, the edited files it keeps and the stale files it will remove.
- Table zoom drops the debug trace of call stacks and the automatic report after each click. The "Ghi báo cáo nút phóng bảng" command still writes `_E11/table-zoom-report.json`.
- Review-log appends are serialized, so two quick events cannot both create the file. The `_E11/` folder is created if it is missing.

## Tests Status
- Type check: pass (`tsc --noEmit`, strict mode with `noUncheckedIndexedAccess`).
- Unit tests: 38 passed across 4 files. They cover:
  - the bridge, including real child processes for exit codes, timeout kill and ENOENT;
  - path detection, settings normalization and the version check against the real `Cargo.toml`;
  - the review-log tracker;
  - the workshop: the golden check, clash, edited, a reformatted `.base`, stale removal, migration from a Python manifest, idempotence, a broken config JSON, and removal with config restore.
- Build: pass. The bundle (`main.js`, 55.6 KB) was loaded under a stub `obsidian`: it exports a `Plugin` subclass and contains the templates.
- Integration in Obsidian: not run (no Obsidian or Obsidian CLI here). The plan's done criterion ("so bằng Obsidian CLI" on a fixture vault, plus a playbook entry) is still open.

## UI still to build (next round), and the pkm JSON it will call
None of these `pkm` subcommands exist yet. Phase 6 moves `propose`, `material` and `zones` into `pkm-core`, and the names below are proposals for that contract. The plugin side only clicks and writes through the Obsidian API.

1. **Proposals (`_Đề xuất/`)**
   - `pkm VAULT propose --json [--similar FILE]` lists candidate proposals (kind, sources, reason). The plugin should write the proposal notes itself, or Rust writes new files with `--create --json` while Obsidian is closed.
   - Accept (`nhận`) and reject (`bỏ`, with a reason) are buttons. They write `status`, `closed` and `reject_reason` through `processFrontMatter`.
   - Applying an accepted proposal (set `type` for a batch, `who` on notes, fix a link, create a draft idea or person page) edits user notes. So it is done in the plugin through `processFrontMatter` and `vault.process`, logged in `_E11/proposals-log.jsonl`, with a "Trả lại" (revert) command.
   - `pkm VAULT propose stats --json` reads `_E11/review-log.jsonl`, which this plugin already writes, and returns acceptance and source-open rates for a stats view.
2. **Material for a draft (`Bài viết/X.md`)**
   - `pkm VAULT material "Bài viết/X.md" --json [--k 12]` returns sections with OHS hits plus graph finds: edge chain, layer, and who proposed each step.
   - The plugin renders `_Xưởng/Nguyên liệu/<draft>.md` or a side view, keeps ticked items, and adds ticked notes to the draft's `uses` through `processFrontMatter`, logged in `_E11/material-log.jsonl`.
   - Its errors include `ohs_not_ready`, which the bridge already maps.
3. **Zones (post suggestions)**
   - `pkm VAULT zones --json` returns the zones with their notes, highlights, score and an "already has a post" flag.
   - The plugin shows a zones screen with a "start a post from this zone" action that creates `Bài viết/…` from the template, and optionally writes `_Xưởng/Gợi ý bài viết.md`.
   - It needs OHS vectors, so `ohs_not_ready` applies here too.
4. **Shared:** typed results from `packages/pkm-contract` once it exists. For now the bridge returns `unknown`, and each screen should validate the shape it reads.

## Next Steps
- When the layout PR is opened: CI wiring (`pnpm install --frozen-lockfile`, `pnpm --filter pkm-obsidian-plugin test` and `build`) and release assets for `main.js`, `manifest.json` and `styles.css` under the same tag. `.github/` was not touched in this round.
- `pkm setup` should download the plugin release, check its sha256, and place it in `.obsidian/plugins/pkm/`.
- Test manually in Obsidian on a fixture vault and on bản B. For bản B, set the topic roots and the extra inbox in settings, then confirm that Dựng xưởng reports 0 clashes, keeps the edited files and removes the old `e11-review-log` files.

## Unresolved Questions
- Should "Dựng xưởng" enable the Homepage and Templater plugins itself? The Python script did not; `setup_vault.py` did. For now the plugin only writes their `data.json` and asks the user to reload Obsidian.
- Prerelease versions (`0.1.0-rc.4`) are fine for a sideloaded plugin. The Obsidian community store would need `x.y.z` without a suffix. Decide this before any store submission.
