// The e2e vault: a throwaway Obsidian vault with made-up notes and the freshly built plugin. Never a real vault: the
// folder must be empty or carry the marker file this script writes, otherwise nothing is touched.
//
//   node e2e/vault.mjs            create or refresh the vault, copy dist/ into .obsidian/plugins/pkm
//
// Location: env PKM_E2E_VAULT, default C:\pkm-e2e-vault (from WSL /mnt/c/pkm-e2e-vault). Obsidian has to open the
// folder as a vault once by hand (Open folder as vault, then turn on community plugins); after that run.mjs drives it.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
export const MARKER = '.pkm-e2e-vault';

export function vaultDir() {
  if (process.env.PKM_E2E_VAULT) return process.env.PKM_E2E_VAULT;
  if (process.platform === 'win32') return 'C:\\pkm-e2e-vault';
  return fs.existsSync('/mnt/c') ? '/mnt/c/pkm-e2e-vault' : path.join(process.env.HOME ?? '.', 'pkm-e2e-vault');
}

/** Obsidian names a vault after its folder; the CLI's `vault=` takes that name. */
export const vaultName = (dir = vaultDir()) => path.basename(dir.replace(/[\\/]+$/, '').replace(/\\/g, '/'));

/** Made-up notes the tests rely on. Long and short notes have numbered sections so scroll tests can align them. */
export function notes() {
  const sections = (n, label) =>
    Array.from({ length: n }, (_, i) => `## Mục ${i + 1}\n\n${`Đoạn ${i + 1} của ${label}, chữ bịa để có chiều dài. `.repeat(30)}\n`).join('\n');
  return {
    'Đọc đối chiếu/Bản dài.md': `# Bản dài\n\n${sections(120, 'bản dài')}`,
    'Đọc đối chiếu/Bản ngắn.md': `# Bản ngắn\n\n${sections(60, 'bản ngắn')}`,
    'Đọc đối chiếu/Bản thứ ba.md': `# Bản thứ ba\n\n${sections(20, 'bản thứ ba')}`,
  };
}

export function prepare(dir = vaultDir(), dist = path.join(here, '..', 'dist')) {
  if (fs.existsSync(dir) && fs.readdirSync(dir).length > 0 && !fs.existsSync(path.join(dir, MARKER))) {
    throw new Error(`${dir} is not empty and has no ${MARKER}: refusing to touch it (is it a real vault?)`);
  }
  if (!fs.existsSync(path.join(dist, 'main.js'))) throw new Error(`no build at ${dist}: run pnpm build first`);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, MARKER), 'Throwaway vault for obsidian-pkm e2e tests (apps/obsidian-plugin/e2e). Safe to delete.\n');
  for (const [rel, text] of Object.entries(notes())) {
    const file = path.join(dir, rel);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, text);
  }
  const obs = path.join(dir, '.obsidian');
  const plugin = path.join(obs, 'plugins', 'pkm');
  fs.mkdirSync(plugin, { recursive: true });
  for (const f of ['main.js', 'manifest.json', 'styles.css']) fs.copyFileSync(path.join(dist, f), path.join(plugin, f));
  const list = path.join(obs, 'community-plugins.json');
  const enabled = fs.existsSync(list) ? JSON.parse(fs.readFileSync(list, 'utf8')) : [];
  if (!enabled.includes('pkm')) fs.writeFileSync(list, JSON.stringify([...enabled, 'pkm'], null, 2));
  return dir;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const dir = prepare();
  console.log(`e2e vault ready: ${dir} (Obsidian vault name "${vaultName(dir)}")`);
}
