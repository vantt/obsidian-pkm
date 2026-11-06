// End-to-end tests in a real Obsidian: build the plugin, copy it into the e2e vault (vault.mjs), reload it, then run
// every e2e/*.e2e.mjs inside Obsidian through the Obsidian CLI (`obsidian eval`) and check what comes back.
//
//   pnpm e2e                      build + all tests
//   pnpm e2e -- --no-build compare  skip the build, only files whose name contains "compare"
//
// Needs Obsidian running with the e2e vault open (once by hand: Open folder as vault on the folder vault.mjs made,
// turn on community plugins). Env: OBSIDIAN_CLI (default `obsidian`), PKM_E2E_VAULT (see vault.mjs).
//
// A test file exports `name`, `run` (an async function evaluated INSIDE Obsidian: no imports, no closures, returns a
// JSON-able value; `app` is Obsidian's global) and `check(result)` (runs here in Node, throws on failure).
import { execFile, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { prepare, vaultDir, vaultName } from './vault.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const filters = args.filter((a) => !a.startsWith('--'));
const CLI = process.env.OBSIDIAN_CLI || 'obsidian';
const dir = vaultDir();
const vault = vaultName(dir);
// Windows caps a command line at 32 767 characters; the eval code travels as one argument.
const MAX_CODE = 30000;

/** /mnt/c/x from WSL as C:\x, which is what the user picks in Obsidian's folder dialog. */
const winPath = (p) => p.replace(/^\/mnt\/([a-z])\//, (_, d) => `${d.toUpperCase()}:\\`).replace(/\//g, (s) => (p.startsWith('/mnt/') ? '\\' : s));

function cli(cmdArgs, timeout = 120_000) {
  return new Promise((resolve, reject) => {
    execFile(CLI, [`vault=${vault}`, ...cmdArgs], { timeout, maxBuffer: 16 << 20 }, (err, stdout, stderr) => {
      const out = `${stdout}${stderr}`.trim();
      if (err) reject(new Error(err.killed ? `timed out after ${timeout} ms: ${cmdArgs[0]}` : out || err.message));
      else resolve(out);
    });
  });
}

/** `obsidian eval` prints `=> <value>`; errors come back as text. */
function parseEval(out) {
  const i = out.lastIndexOf('=> ');
  if (i < 0) throw new Error(`no result from eval: ${out.slice(0, 500)}`);
  const raw = out.slice(i + 3).trim();
  try {
    return JSON.parse(raw);
  } catch {
    throw new Error(`eval returned non-JSON: ${raw.slice(0, 500)}`);
  }
}

async function main() {
  if (!args.includes('--no-build')) execFileSync('npm', ['run', 'build'], { cwd: path.join(here, '..'), stdio: 'inherit' });
  prepare(dir);
  console.log(`vault: ${dir} ("${vault}")`);

  try {
    await cli(['version'], 20_000);
  } catch (e) {
    throw new Error(`Obsidian CLI not reachable (${CLI}): ${e.message}. Is Obsidian running?`);
  }
  const plugins = await cli(['plugins:enabled'], 30_000).catch((e) => `ERR ${e.message}`);
  if (!/^pkm$/m.test(plugins)) {
    if (plugins.startsWith('ERR') || /not found|unknown vault/i.test(plugins)) {
      throw new Error(
        `Obsidian does not know the vault "${vault}". Once by hand: Obsidian > Open folder as vault > ${winPath(dir)}, ` +
          `then Settings > Community plugins > Turn on. Then run again.\n${plugins}`,
      );
    }
    await cli(['plugin:enable', 'id=pkm'], 30_000);
  }
  await cli(['plugin:reload', 'id=pkm'], 30_000);

  const files = fs
    .readdirSync(here)
    .filter((f) => f.endsWith('.e2e.mjs') && (filters.length === 0 || filters.some((s) => f.includes(s))))
    .sort();
  let failed = 0;
  for (const f of files) {
    const t = await import(pathToFileURL(path.join(here, f)).href);
    const code = `(${t.run.toString()})().then((r) => JSON.stringify(r))`;
    if (code.length > MAX_CODE) throw new Error(`${f}: eval code is ${code.length} chars, over ${MAX_CODE}`);
    const t0 = Date.now();
    try {
      const out = await cli(['eval', `code=${code}`], t.timeoutMs ?? 180_000);
      const result = parseEval(out);
      // eval returns the JSON string itself, so parse twice when needed
      const value = typeof result === 'string' ? JSON.parse(result) : result;
      t.check(value);
      console.log(`ok   ${t.name} (${Date.now() - t0} ms)`);
    } catch (e) {
      failed++;
      console.log(`FAIL ${t.name} (${Date.now() - t0} ms)\n     ${String(e.message).replace(/\n/g, '\n     ')}`);
    }
  }
  console.log(`${files.length - failed}/${files.length} passed`);
  process.exitCode = failed ? 1 : 0;
}

main().catch((e) => {
  console.error(e.message);
  process.exitCode = 2;
});
